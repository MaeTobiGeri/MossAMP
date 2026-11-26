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
/// Level above which the safety limiter starts to round off peaks.
const LIMIT_THRESHOLD: f32 = 0.9;

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
    /// False for a 0 dB band: it is skipped entirely, keeping flat bands
    /// bit-exact.
    active: bool,
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
            active: gain_db.abs() > 0.01,
            ..Biquad::default()
        }
    }

    /// Takes over another filter's coefficients but keeps this filter's
    /// history, so changing a gain while playing does not click.
    fn retune(&mut self, other: &Biquad) {
        self.b0 = other.b0;
        self.b1 = other.b1;
        self.b2 = other.b2;
        self.a1 = other.a1;
        self.a2 = other.a2;
        if other.active && !self.active {
            self.reset();
        }
        self.active = other.active;
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Magnitude of the filter's response at `frequency`.
    fn magnitude(&self, frequency: f32, sample_rate: f32) -> f32 {
        let w = 2.0 * PI * frequency / sample_rate;
        let (c1, s1) = (w.cos(), w.sin());
        let (c2, s2) = ((2.0 * w).cos(), (2.0 * w).sin());
        let num_re = self.b0 + self.b1 * c1 + self.b2 * c2;
        let num_im = -(self.b1 * s1 + self.b2 * s2);
        let den_re = 1.0 + self.a1 * c1 + self.a2 * c2;
        let den_im = -(self.a1 * s1 + self.a2 * s2);
        ((num_re * num_re + num_im * num_im) / (den_re * den_re + den_im * den_im)).sqrt()
    }

    fn process(&mut self, input: f32) -> f32 {
        if !self.active {
            return input;
        }
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
    /// Gain applied before the filters so boosted bands do not push the
    /// signal past full scale (which would clip and distort).
    preamp: f32,
    was_enabled: bool,
}

/// Highest gain (as a linear factor) the combined bands apply anywhere in the
/// audible range. Neighbouring bands overlap, so this can exceed the largest
/// single slider value.
fn peak_gain(bands: &[Biquad], sample_rate: f32) -> f32 {
    const POINTS: usize = 256;
    let low: f32 = 20.0;
    let high = 20_000f32.min(sample_rate * 0.45);
    let grid = (0..POINTS)
        .map(|i| low * (high / low).powf(i as f32 / (POINTS - 1) as f32))
        .chain(BAND_FREQUENCIES.into_iter().filter(|f| *f < high));

    grid.map(|f| bands.iter().map(|b| b.magnitude(f, sample_rate)).product::<f32>())
        .fold(1.0, f32::max)
}

/// Soft limiter: transparent below the threshold, then rounds peaks off
/// smoothly towards full scale instead of hard clipping them.
fn soft_limit(sample: f32) -> f32 {
    let level = sample.abs();
    if level <= LIMIT_THRESHOLD {
        return sample;
    }
    let headroom = 1.0 - LIMIT_THRESHOLD;
    let limited = LIMIT_THRESHOLD + headroom * ((level - LIMIT_THRESHOLD) / headroom).tanh();
    limited.copysign(sample)
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
            preamp: 1.0,
            was_enabled: false,
        }
    }

    fn rebuild_filters(&mut self) {
        let gains = *self.shared.gains.lock().unwrap();
        let sample_rate = self.source.sample_rate() as f32;

        // Every band always gets a filter (a 0 dB band is an exact
        // pass-through), so the filter list keeps its shape and each filter's
        // history survives gain changes.
        let bands: Vec<Biquad> = gains
            .iter()
            .zip(BAND_FREQUENCIES)
            .map(|(gain, frequency)| {
                let gain = if frequency < sample_rate / 2.0 { *gain } else { 0.0 };
                Biquad::peaking(frequency, gain, sample_rate)
            })
            .collect();

        self.preamp = 1.0 / peak_gain(&bands, sample_rate);

        for channel in &mut self.filters {
            if channel.len() == bands.len() {
                for (filter, band) in channel.iter_mut().zip(&bands) {
                    filter.retune(band);
                }
            } else {
                *channel = bands.clone();
            }
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

        let enabled = self.shared.enabled.load(Ordering::Relaxed);
        if enabled && !self.was_enabled {
            // History from before the EQ was switched off is stale.
            for filter in self.filters.iter_mut().flatten() {
                filter.reset();
            }
        }
        self.was_enabled = enabled;

        if enabled {
            sample *= self.preamp;
            for filter in &mut self.filters[channel] {
                sample = filter.process(sample);
            }
            sample = soft_limit(sample);
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

    fn output(frequency: f32, eq: &Arc<EqShared>) -> Vec<f32> {
        let decoder = Decoder::try_from(Cursor::new(sine_wav(frequency, 1))).unwrap();
        Equalizer::new(decoder, eq.clone()).skip(10000).collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn gains_with(band: usize, gain: f32) -> [f32; EQ_BANDS] {
        let mut gains = [0.0; EQ_BANDS];
        gains[band] = gain;
        gains
    }

    #[test]
    fn boosting_a_band_raises_it_relative_to_the_rest() {
        let eq = EqShared::new(true, gains_with(5, 6.0));
        let boosted = rms(&output(1000.0, &eq));
        let untouched = rms(&output(125.0, &eq));

        let ratio = boosted / untouched;
        assert!((1.8..2.2).contains(&ratio), "ratio was {ratio}");
    }

    #[test]
    fn boosting_never_makes_the_signal_louder_than_the_original() {
        // The preamp gives back exactly the headroom the boost needs, so a
        // boosted frequency comes out at (about) its original level.
        let flat = rms(&output(1000.0, &EqShared::new(false, [0.0; EQ_BANDS])));
        let boosted = rms(&output(1000.0, &EqShared::new(true, gains_with(5, 6.0))));

        let ratio = boosted / flat;
        assert!((0.95..1.05).contains(&ratio), "ratio was {ratio}");
    }

    #[test]
    fn flat_eq_is_transparent() {
        let off = output(440.0, &EqShared::new(false, [0.0; EQ_BANDS]));
        let flat = output(440.0, &EqShared::new(true, [0.0; EQ_BANDS]));
        let max_diff = off.iter().zip(&flat).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(max_diff < 1e-4, "max difference was {max_diff}");
    }

    #[test]
    fn overlapping_boosts_are_compensated() {
        // "Bass boost": adjacent bands stack to more than the largest slider.
        let gains = [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let rate = 44100.0;
        let bands: Vec<Biquad> = gains
            .iter()
            .zip(BAND_FREQUENCIES)
            .map(|(g, f)| Biquad::peaking(f, *g, rate))
            .collect();
        let peak_db = 20.0 * peak_gain(&bands, rate).log10();
        assert!(peak_db > 6.0 && peak_db < 12.0, "peak was {peak_db} dB");
    }

    #[test]
    fn limiter_is_smooth_and_bounded() {
        assert_eq!(soft_limit(0.5), 0.5);
        assert_eq!(soft_limit(-0.9), -0.9);
        assert!(soft_limit(5.0) <= 1.0 && soft_limit(5.0) > 0.99);
        assert!(soft_limit(-5.0) >= -1.0);
        let just_over = soft_limit(0.91);
        assert!((just_over - 0.91).abs() < 0.001);
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
