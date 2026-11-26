//! Volume leveling: measures each song's loudness (EBU R128) in the
//! background and works out the gain that brings it to a common level.

use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use ebur128::{EbuR128, Mode};
use rodio::{Decoder, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::player::Command;
use crate::AppState;

/// Level every song is brought to, in LUFS (the level Apple Music uses).
pub const TARGET_LUFS: f64 = -16.0;
/// Stored for songs that cannot be measured (silence, decode errors), so they
/// are not analysed again. They play without adjustment.
pub const UNMEASURABLE: f64 = -1000.0;
/// Most a quiet song is ever boosted, in dB.
const MAX_BOOST_DB: f64 = 12.0;
/// Assumed for a song that has not been measured yet: typical for modern
/// masters, so an unmeasured song is not much louder than the leveled ones.
const PROVISIONAL_LUFS: f64 = -10.0;

pub enum Request {
    /// Look for unmeasured songs (after a scan or when leveling is turned on).
    Wake,
    /// Measure this song next; it is about to play.
    Priority(String),
}

#[derive(Serialize, Clone, Copy, Default)]
pub struct Progress {
    pub analyzed: i64,
    pub total: i64,
}

/// Linear gain for a song with the given measurement.
pub fn gain(loudness: Option<f64>, peak: Option<f64>) -> f32 {
    let loudness = loudness.unwrap_or(PROVISIONAL_LUFS);
    if !loudness.is_finite() || loudness <= UNMEASURABLE {
        return 1.0;
    }
    let db = (TARGET_LUFS - loudness).min(MAX_BOOST_DB);
    let mut gain = 10f64.powf(db / 20.0);
    // A boost may only use the headroom the song actually has, so leveling
    // never makes a song clip.
    if let Some(peak) = peak.filter(|p| *p > 0.0) {
        gain = gain.min((1.0 / peak).max(1.0));
    }
    gain as f32
}

/// Integrated loudness (LUFS) and sample peak (linear) of a file.
pub fn measure(path: &Path) -> Option<(f64, f64)> {
    let decoder = Decoder::try_from(File::open(path).ok()?).ok()?;
    let channels = decoder.channels() as u32;
    let rate = decoder.sample_rate();
    let mut meter = EbuR128::new(channels, rate, Mode::I | Mode::SAMPLE_PEAK).ok()?;

    let chunk_len = 4096 * channels as usize;
    let mut chunk = Vec::with_capacity(chunk_len);
    for sample in decoder {
        chunk.push(sample);
        if chunk.len() == chunk_len {
            meter.add_frames_f32(&chunk).ok()?;
            chunk.clear();
        }
    }
    // Only whole frames can be added.
    chunk.truncate(chunk.len() - chunk.len() % channels as usize);
    if !chunk.is_empty() {
        meter.add_frames_f32(&chunk).ok()?;
    }

    let loudness = meter.loudness_global().ok()?;
    let peak = (0..channels)
        .filter_map(|c| meter.sample_peak(c).ok())
        .fold(0.0, f64::max);
    loudness.is_finite().then_some((loudness, peak))
}

pub fn spawn(app: AppHandle, requests: Receiver<Request>, player: Sender<Command>) {
    thread::Builder::new()
        .name("loudness".into())
        .spawn(move || run(app, requests, player))
        .expect("failed to start loudness analyser");
}

fn enabled(state: &AppState) -> bool {
    state.config.lock().unwrap().playback.normalize
}

fn run(app: AppHandle, requests: Receiver<Request>, player: Sender<Command>) {
    let mut priority: VecDeque<String> = VecDeque::new();

    // Sleeps until there is something to do; returns when the app closes.
    while let Ok(request) = requests.recv() {
        if let Request::Priority(path) = request {
            priority.push_back(path);
        }

        let Some(state) = app.try_state::<AppState>() else { continue };

        loop {
            while let Ok(request) = requests.try_recv() {
                if let Request::Priority(path) = request {
                    priority.push_back(path);
                }
            }
            if !enabled(&state) {
                priority.clear();
                break;
            }

            let next = match priority.pop_front() {
                Some(path) => Some(path),
                None => crate::library::next_unmeasured(&state.db.lock().unwrap()).ok().flatten(),
            };
            let Some(path) = next else { break };

            let stored = crate::library::loudness_of(&state.db.lock().unwrap(), &path).ok().flatten();
            let (loudness, peak) = match stored {
                Some(measured) => measured,
                None => {
                    let measured = measure(Path::new(&path)).unwrap_or((UNMEASURABLE, 0.0));
                    let saved = crate::library::store_loudness(&state.db.lock().unwrap(), &path, measured);
                    if saved.is_err() {
                        // The song is gone from the library; skip it.
                        continue;
                    }
                    if let Ok(progress) = crate::library::loudness_progress(&state.db.lock().unwrap()) {
                        let _ = app.emit("loudness-progress", progress);
                    }
                    measured
                }
            };
            let _ = player.send(Command::Loudness { path, loudness, peak });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(gain: f32) -> f64 {
        20.0 * (gain as f64).log10()
    }

    #[test]
    fn loud_songs_are_turned_down_to_the_target() {
        assert!((db(gain(Some(-8.0), Some(1.0))) - -8.0).abs() < 0.01);
    }

    #[test]
    fn quiet_songs_are_boosted_only_as_far_as_their_peaks_allow() {
        // -22 LUFS wants +6 dB, but a peak of 0.7 leaves only ~3.1 dB.
        let boost = db(gain(Some(-22.0), Some(0.7)));
        assert!((boost - 3.1).abs() < 0.05, "boost was {boost}");
        // With enough headroom the full boost is applied.
        assert!((db(gain(Some(-22.0), Some(0.3))) - 6.0).abs() < 0.01);
    }

    #[test]
    fn unmeasurable_songs_play_unchanged() {
        assert_eq!(gain(Some(UNMEASURABLE), Some(0.0)), 1.0);
    }

    #[test]
    fn unmeasured_songs_get_a_typical_reduction() {
        assert!((db(gain(None, None)) - -6.0).abs() < 0.01);
    }

    #[test]
    fn measures_a_tone() {
        // A full-scale 1 kHz sine measures about -3 LUFS per channel; at
        // half amplitude (-6 dB) about -9 LUFS for a mono file.
        let rate = 44100u32;
        let samples: Vec<i16> = (0..rate * 3)
            .map(|i| ((2.0 * std::f32::consts::PI * 1000.0 * i as f32 / rate as f32).sin() * 16384.0) as i16)
            .collect();
        let data_len = (samples.len() * 2) as u32;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + data_len).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(rate.to_le_bytes());
        wav.extend((rate * 2).to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(data_len.to_le_bytes());
        for s in samples {
            wav.extend(s.to_le_bytes());
        }
        let path = std::env::temp_dir().join(format!("mossamp-loudness-{}.wav", std::process::id()));
        std::fs::write(&path, wav).unwrap();
        let measured = measure(&path);
        let _ = std::fs::remove_file(&path);

        let (loudness, peak) = measured.unwrap();
        assert!((loudness - -9.0).abs() < 0.5, "loudness was {loudness}");
        assert!((peak - 0.5).abs() < 0.01, "peak was {peak}");
    }
}
