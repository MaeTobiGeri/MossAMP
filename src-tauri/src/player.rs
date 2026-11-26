use std::fs::File;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rodio::cpal::StreamError;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::{PlaybackConfig, Repeat};
use crate::eq::{EqShared, Equalizer};
use crate::library::Track;
use crate::loudness::{self, Request};
use crate::media_keys::MediaKeys;

const TICK: Duration = Duration::from_millis(250);
/// On phones the system volume buttons control loudness, so the app always
/// plays at full level there and has no volume slider of its own.
const FIXED_VOLUME: bool = cfg!(any(target_os = "android", target_os = "ios"));
const RESTART_THRESHOLD: f64 = 3.0;
/// Ticks of "playing but the position does not move" before the output is
/// considered dead and reopened (8 * 250ms = 2s).
const STALL_TICKS: u32 = 8;

pub enum Command {
    PlayQueue(Vec<Track>, usize),
    Enqueue(Track),
    PlayNext(Track),
    Resume,
    Pause,
    Toggle,
    Next,
    Previous,
    Seek(f64),
    Volume(f32),
    Shuffle(bool),
    Repeat(Repeat),
    /// Turns volume leveling on or off.
    Normalize(bool),
    /// A song's loudness was measured.
    Loudness { path: String, loudness: f64, peak: f64 },
}

#[derive(Serialize, Clone)]
pub struct PlayerState {
    track: Option<Track>,
    playing: bool,
    position: f64,
    duration: f64,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    queue_length: usize,
}

impl PlayerState {
    pub fn track(&self) -> Option<&Track> {
        self.track.as_ref()
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn position(&self) -> f64 {
        self.position
    }
}

struct Player {
    app: AppHandle,
    stream: Option<OutputStream>,
    /// Set from the audio backend's error callback when the output device went
    /// away (e.g. Android disconnects the AAudio stream when Bluetooth connects
    /// and the audio route changes).
    stream_lost: Arc<AtomicBool>,
    /// Position where the current sink's source started (sources are seeked
    /// before being appended, so `sink.get_pos()` is relative to this).
    offset: Duration,
    last_pos: Duration,
    stall_ticks: u32,
    eq: Arc<EqShared>,
    normalize: bool,
    analyzer: Sender<Request>,
    sink: Option<Sink>,
    queue: Vec<Track>,
    index: Option<usize>,
    history: Vec<usize>,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    random_state: u64,
}

pub fn spawn(
    app: AppHandle,
    eq: Arc<EqShared>,
    settings: PlaybackConfig,
    analyzer: Sender<Request>,
) -> Sender<Command> {
    let (tx, rx) = mpsc::channel();
    let media_keys_tx = tx.clone();

    thread::spawn(move || {
        let stream_lost = Arc::new(AtomicBool::new(false));
        let stream = open_stream(&stream_lost);
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);

        let mut player = Player {
            app,
            stream,
            stream_lost,
            offset: Duration::ZERO,
            last_pos: Duration::ZERO,
            stall_ticks: 0,
            eq,
            normalize: settings.normalize,
            analyzer,
            sink: None,
            queue: Vec::new(),
            index: None,
            history: Vec::new(),
            volume: if FIXED_VOLUME { 1.0 } else { settings.volume },
            shuffle: settings.shuffle,
            repeat: settings.repeat,
            random_state: seed | 1,
        };
        player.run(rx, MediaKeys::new(media_keys_tx));
    });

    tx
}

/// Opens the current default output device. The error callback flags the
/// stream as lost when the device disappears so the player thread can reopen
/// it on whatever device is the default now.
fn open_stream(lost: &Arc<AtomicBool>) -> Option<OutputStream> {
    lost.store(false, Ordering::SeqCst);
    let flag = lost.clone();
    let on_error = move |err: StreamError| {
        eprintln!("audio stream error: {err}");
        if matches!(err, StreamError::DeviceNotAvailable) {
            flag.store(true, Ordering::SeqCst);
        }
    };

    let builder = match OutputStreamBuilder::from_default_device() {
        Ok(builder) => builder.with_error_callback(on_error),
        Err(err) => {
            eprintln!("no audio output device: {err}");
            return None;
        }
    };
    match builder.open_stream_or_fallback() {
        Ok(mut stream) => {
            stream.log_on_drop(false);
            Some(stream)
        }
        Err(err) => {
            eprintln!("cannot open audio output: {err}");
            None
        }
    }
}

impl Player {
    fn run(&mut self, rx: Receiver<Command>, mut media_keys: MediaKeys) {
        loop {
            let received = match rx.recv_timeout(TICK) {
                Ok(command) => {
                    self.check_output();
                    self.handle(command);
                    true
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.check_output();
                    false
                }
                Err(RecvTimeoutError::Disconnected) => return,
            };

            if self.track_finished() {
                self.advance_after_finish();
            }

            if received || self.is_playing() {
                let state = self.state();
                media_keys.update(&state);
                let _ = self.app.emit("player-state", &state);
            }
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::PlayQueue(tracks, start) => {
                self.queue = tracks;
                self.history.clear();
                self.start_track(start);
            }
            Command::Enqueue(track) => self.queue.push(track),
            Command::PlayNext(track) => {
                let position = self.index.map(|i| i + 1).unwrap_or(self.queue.len());
                self.queue.insert(position, track);
            }
            Command::Resume => {
                if let Some(sink) = &self.sink {
                    sink.play();
                } else if let Some(index) = self.index {
                    self.start_track(index);
                }
            }
            Command::Pause => {
                if let Some(sink) = &self.sink {
                    sink.pause();
                }
            }
            Command::Toggle => {
                if let Some(sink) = &self.sink {
                    if sink.is_paused() {
                        sink.play();
                    } else {
                        sink.pause();
                    }
                } else if let Some(index) = self.index {
                    self.start_track(index);
                }
            }
            Command::Next => self.skip_forward(),
            Command::Previous => self.skip_back(),
            Command::Seek(seconds) => {
                // Rebuild the sink from a pre-seeked decoder instead of using
                // `Sink::try_seek`, which blocks until the audio thread picks the
                // request up and therefore hangs forever on a dead output.
                if let (Some(sink), Some(index)) = (&self.sink, self.index) {
                    let paused = sink.is_paused();
                    self.load_track(index, Duration::from_secs_f64(seconds.max(0.0)), paused);
                }
            }
            Command::Volume(_) if FIXED_VOLUME => {}
            Command::Volume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                if let Some(sink) = &self.sink {
                    sink.set_volume(self.volume);
                }
            }
            Command::Shuffle(enabled) => self.shuffle = enabled,
            Command::Repeat(mode) => self.repeat = mode,
            Command::Normalize(enabled) => {
                self.normalize = enabled;
                self.apply_track_gain();
            }
            Command::Loudness { path, loudness, peak } => {
                // Every copy in the queue learns the result, so the song
                // starts at the right level the next time too.
                for track in self.queue.iter_mut().filter(|t| t.path == path) {
                    track.loudness = Some(loudness);
                    track.peak = Some(peak);
                }
                if self.current_track().is_some_and(|t| t.path == path) {
                    self.apply_track_gain();
                }
            }
        }
    }

    fn start_track(&mut self, index: usize) -> bool {
        self.load_track(index, Duration::ZERO, false)
    }

    fn load_track(&mut self, index: usize, start: Duration, paused: bool) -> bool {
        self.stop_sink();
        let Some(track) = self.queue.get(index) else {
            return false;
        };
        if self.stream.is_none() {
            // Startup or a previous reopen may have failed (e.g. while the
            // phone was switching to Bluetooth); try again now.
            self.stream = open_stream(&self.stream_lost);
        }
        let Some(stream) = &self.stream else {
            let _ = self.app.emit("player-error", "No audio output device available");
            self.index = Some(index);
            return false;
        };

        let decoded = File::open(&track.path)
            .ok()
            .and_then(|file| Decoder::try_from(file).ok());
        let Some(mut decoder) = decoded else {
            let _ = self.app.emit("player-error", format!("Cannot play {}", track.title));
            self.index = Some(index);
            return false;
        };

        let mut offset = Duration::ZERO;
        if start > Duration::ZERO && decoder.try_seek(start).is_ok() {
            offset = start;
        }

        // Set before the sink exists so the song starts at its own level.
        self.index = Some(index);
        self.apply_track_gain();

        let sink = Sink::connect_new(stream.mixer());
        sink.set_volume(self.volume);
        if paused {
            sink.pause();
        }
        sink.append(Equalizer::new(decoder, self.eq.clone()));

        self.sink = Some(sink);
        self.index = Some(index);
        self.offset = offset;
        self.last_pos = offset;
        self.stall_ticks = 0;
        true
    }

    fn current_track(&self) -> Option<&Track> {
        self.index.and_then(|i| self.queue.get(i))
    }

    /// Volume-leveling gain for the current song; asks the analyser to
    /// measure it first if that has not happened yet.
    fn apply_track_gain(&self) {
        let gain = match self.current_track() {
            Some(track) if self.normalize => {
                if track.loudness.is_none() {
                    let _ = self.analyzer.send(Request::Priority(track.path.clone()));
                }
                loudness::gain(track.loudness, track.peak)
            }
            _ => 1.0,
        };
        self.eq.set_track_gain(gain);
    }

    fn position(&self) -> Duration {
        self.sink
            .as_ref()
            .map(|sink| self.offset + sink.get_pos())
            .unwrap_or(Duration::ZERO)
    }

    /// Detects a dead audio output and reopens it on the current default
    /// device, resuming the current track where it was.
    fn check_output(&mut self) {
        let lost = self.stream_lost.load(Ordering::SeqCst);

        let stalled = if self.is_playing() {
            let pos = self.position();
            if pos == self.last_pos {
                self.stall_ticks += 1;
            } else {
                self.stall_ticks = 0;
                self.last_pos = pos;
            }
            self.stall_ticks >= STALL_TICKS
        } else {
            self.stall_ticks = 0;
            false
        };

        if lost || stalled {
            self.reopen_output();
        }
    }

    fn reopen_output(&mut self) {
        let resume = self
            .sink
            .as_ref()
            .map(|sink| sink.is_paused())
            .zip(self.index);
        let position = self.position();

        self.stop_sink();
        // Drop the old stream before opening a new one so the backend releases it.
        self.stream = None;
        self.stream = open_stream(&self.stream_lost);
        self.stall_ticks = 0;

        if let Some((paused, index)) = resume {
            self.load_track(index, position, paused);
        }
    }

    fn stop_sink(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }

    fn is_playing(&self) -> bool {
        self.sink
            .as_ref()
            .map(|sink| !sink.is_paused() && !sink.empty())
            .unwrap_or(false)
    }

    fn track_finished(&self) -> bool {
        self.sink.as_ref().map(|sink| sink.empty()).unwrap_or(false)
    }

    fn advance_after_finish(&mut self) {
        let Some(current) = self.index else { return };

        if self.repeat == Repeat::One {
            self.start_track(current);
            return;
        }

        let last = current + 1 >= self.queue.len();
        if last && !self.shuffle && self.repeat == Repeat::Off {
            self.stop_sink();
            return;
        }
        self.skip_forward();
    }

    fn skip_forward(&mut self) {
        let Some(current) = self.index else { return };
        let count = self.queue.len();
        if count == 0 {
            return;
        }

        for _ in 0..count {
            let candidate = self.pick_next(self.index.unwrap_or(current), count);
            self.history.push(self.index.unwrap_or(current));
            if self.start_track(candidate) {
                return;
            }
        }
    }

    fn pick_next(&mut self, current: usize, count: usize) -> usize {
        if self.shuffle && count > 1 {
            loop {
                let candidate = (self.next_random() % count as u64) as usize;
                if candidate != current {
                    return candidate;
                }
            }
        }
        (current + 1) % count
    }

    fn skip_back(&mut self) {
        let position = self.position().as_secs_f64();

        if position > RESTART_THRESHOLD {
            if let (Some(sink), Some(index)) = (&self.sink, self.index) {
                let paused = sink.is_paused();
                self.load_track(index, Duration::ZERO, paused);
            }
            return;
        }

        let count = self.queue.len();
        let Some(current) = self.index else { return };
        let target = self
            .history
            .pop()
            .unwrap_or_else(|| (current + count - 1) % count.max(1));
        self.start_track(target);
    }

    fn next_random(&mut self) -> u64 {
        let mut x = self.random_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.random_state = x;
        x
    }

    fn state(&self) -> PlayerState {
        let track = self.index.and_then(|i| self.queue.get(i)).cloned();
        PlayerState {
            duration: track.as_ref().map(|t| t.duration).unwrap_or(0.0),
            position: self.position().as_secs_f64(),
            playing: self.is_playing(),
            track,
            volume: self.volume,
            shuffle: self.shuffle,
            repeat: self.repeat,
            queue_length: self.queue.len(),
        }
    }
}
