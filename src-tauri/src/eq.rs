use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};

use crate::config::EQ_BANDS;

pub const BAND_FREQUENCIES: [f32; EQ_BANDS] =
    [32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

const BAND_Q: f32 = 1.2;

pub struct EqShared {
    enabled: AtomicBool,
    version: AtomicU32,
    gains: Mutex<[f32; EQ_BANDS]>,
}

impl EqShared {
    pub fn new(enabled: bool, gains: [f32; EQ_BANDS]) -> Arc<EqShared> {
        Arc::new(EqShared {
            enabled: AtomicBool::new(enabled),
            version: AtomicU32::new(1),
            gains: Mutex::new(gains),
        })
    }

    pub fn set(&self, enabled: bool, gains: [f32; EQ_BANDS]) {
        *self.gains.lock().unwrap() = gains;
        self.enabled.store(enabled, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn peaking(frequency: f32, gain_db: f32, sample_rate: f32) -> Biquad {
        let amplitude = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * frequency / sample_rate;
        let alpha = w0.sin() / (2.0 * BAND_Q);
        let cos_w0 = w0.cos();

        let a0 = 1.0 + alpha / amplitude;
        Biquad {
            b0: (1.0 + alpha * amplitude) / a0,
            b1: (-2.0 * cos_w0) / a0,
            b2: (1.0 - alpha * amplitude) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha / amplitude) / a0,
            ..Biquad::default()
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

pub struct Equalizer<S: Source> {
    source: S,
    shared: Arc<EqShared>,
    seen_version: u32,
    channels: usize,
    next_channel: usize,
    filters: Vec<Vec<Biquad>>,
}

impl<S: Source> Equalizer<S> {
    pub fn new(source: S, shared: Arc<EqShared>) -> Self {
        let channels = source.channels() as usize;
        Equalizer {
            source,
            shared,
            seen_version: 0,
            channels,
            next_channel: 0,
            filters: vec![Vec::new(); channels],
        }
    }

    fn rebuild_filters(&mut self) {
        let gains = *self.shared.gains.lock().unwrap();
        let sample_rate = self.source.sample_rate() as f32;

        let bands: Vec<Biquad> = gains
            .iter()
            .zip(BAND_FREQUENCIES)
            .filter(|(gain, frequency)| gain.abs() > 0.01 && *frequency < sample_rate / 2.0)
            .map(|(gain, frequency)| Biquad::peaking(frequency, *gain, sample_rate))
            .collect();

        for channel in &mut self.filters {
            *channel = bands.clone();
        }
    }
}

impl<S: Source> Iterator for Equalizer<S> {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        let mut sample = self.source.next()?;

        let version = self.shared.version.load(Ordering::Acquire);
        if version != self.seen_version {
            self.seen_version = version;
            self.rebuild_filters();
        }

        let channel = self.next_channel;
        self.next_channel = (channel + 1) % self.channels;

        if self.shared.enabled.load(Ordering::Relaxed) {
            for filter in &mut self.filters[channel] {
                sample = filter.process(sample);
            }
            sample = sample.clamp(-1.0, 1.0);
        }

        Some(sample)
    }
}

impl<S: Source> Source for Equalizer<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.source.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.source.try_seek(position)?;
        self.next_channel = 0;
        self.seen_version = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use rodio::Decoder;

    use super::*;

    fn sine_wav(frequency: f32, seconds: u32) -> Vec<u8> {
        let rate = 44100u32;
        let samples: Vec<i16> = (0..rate * seconds)
            .map(|i| ((2.0 * PI * frequency * i as f32 / rate as f32).sin() * 8000.0) as i16)
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
        for sample in samples {
            wav.extend(sample.to_le_bytes());
        }
        wav
    }

    fn rms(eq: &Arc<EqShared>) -> f32 {
        let decoder = Decoder::try_from(Cursor::new(sine_wav(1000.0, 1))).unwrap();
        let samples: Vec<f32> = Equalizer::new(decoder, eq.clone()).skip(10000).collect();
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn boosting_a_band_raises_its_level() {
        let flat = rms(&EqShared::new(false, [0.0; EQ_BANDS]));

        let mut gains = [0.0; EQ_BANDS];
        gains[5] = 6.0;
        let boosted = rms(&EqShared::new(true, gains));

        let ratio = boosted / flat;
        assert!((1.8..2.2).contains(&ratio), "ratio was {ratio}");
    }

    #[test]
    fn seeking_skips_ahead() {
        let decoder = Decoder::try_from(Cursor::new(sine_wav(440.0, 4))).unwrap();
        let mut source = Equalizer::new(decoder, EqShared::new(true, [0.0; EQ_BANDS]));

        source.try_seek(Duration::from_secs(3)).unwrap();
        let remaining = source.count();

        assert!(remaining < 44100 * 2, "remaining was {remaining}");
    }
}
