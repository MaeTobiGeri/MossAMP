use std::fs::File;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::{PlaybackConfig, Repeat};
use crate::eq::{EqShared, Equalizer};
use crate::library::Track;
use crate::media_keys::MediaKeys;

const TICK: Duration = Duration::from_millis(250);
const RESTART_THRESHOLD: f64 = 3.0;

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
    eq: Arc<EqShared>,
    sink: Option<Sink>,
    queue: Vec<Track>,
    index: Option<usize>,
    history: Vec<usize>,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    random_state: u64,
}

pub fn spawn(app: AppHandle, eq: Arc<EqShared>, settings: PlaybackConfig) -> Sender<Command> {
    let (tx, rx) = mpsc::channel();
    let media_keys_tx = tx.clone();

    thread::spawn(move || {
        let stream = OutputStreamBuilder::open_default_stream().ok();
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);

        let mut player = Player {
            app,
            stream,
            eq,
            sink: None,
            queue: Vec::new(),
            index: None,
            history: Vec::new(),
            volume: settings.volume,
            shuffle: settings.shuffle,
            repeat: settings.repeat,
            random_state: seed | 1,
        };
        player.run(rx, MediaKeys::new(media_keys_tx));
    });

    tx
}

impl Player {
    fn run(&mut self, rx: Receiver<Command>, mut media_keys: MediaKeys) {
        loop {
            let received = match rx.recv_timeout(TICK) {
                Ok(command) => {
                    self.handle(command);
                    true
                }
                Err(RecvTimeoutError::Timeout) => false,
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
                if let Some(sink) = &self.sink {
                    let _ = sink.try_seek(Duration::from_secs_f64(seconds.max(0.0)));
                }
            }
            Command::Volume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                if let Some(sink) = &self.sink {
                    sink.set_volume(self.volume);
                }
            }
            Command::Shuffle(enabled) => self.shuffle = enabled,
            Command::Repeat(mode) => self.repeat = mode,
        }
    }

    fn start_track(&mut self, index: usize) -> bool {
        self.stop_sink();
        let Some(track) = self.queue.get(index) else {
            return false;
        };
        let Some(stream) = &self.stream else {
            return false;
        };

        let decoded = File::open(&track.path)
            .ok()
            .and_then(|file| Decoder::try_from(file).ok());
        let Some(decoder) = decoded else {
            let _ = self.app.emit("player-error", format!("Cannot play {}", track.title));
            self.index = Some(index);
            return false;
        };

        let sink = Sink::connect_new(stream.mixer());
        sink.set_volume(self.volume);
        sink.append(Equalizer::new(decoder, self.eq.clone()));

        self.sink = Some(sink);
        self.index = Some(index);
        true
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
        let position = self
            .sink
            .as_ref()
            .map(|sink| sink.get_pos().as_secs_f64())
            .unwrap_or(0.0);

        if position > RESTART_THRESHOLD {
            if let Some(sink) = &self.sink {
                let _ = sink.try_seek(Duration::ZERO);
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
            position: self
                .sink
                .as_ref()
                .map(|sink| sink.get_pos().as_secs_f64())
                .unwrap_or(0.0),
            playing: self.is_playing(),
            track,
            volume: self.volume,
            shuffle: self.shuffle,
            repeat: self.repeat,
            queue_length: self.queue.len(),
        }
    }
}
