//! Real-time 8-band spectrum meter for the pill waveform.
//!
//! Driven by the same cpal stream that feeds transcription — one audio source on
//! every OS, so the pill no longer opens its own (Linux-fragile) getUserMedia
//! stream. The cpal callback only copies samples into a ring buffer; the FFT runs
//! off the audio thread when the emitter task asks for `levels()`.

use std::sync::Mutex;

use rustfft::{num_complex::Complex, Fft, FftPlanner};

pub const BANDS: usize = 8;

const FFT_SIZE: usize = 1024;

/// Band edges in Hz, log-spaced across the speech range. 9 edges → 8 bands.
const BAND_EDGES: [f32; BANDS + 1] = [
    80.0, 170.0, 310.0, 530.0, 850.0, 1300.0, 2000.0, 3100.0, 5000.0,
];

/// dB window mapped to bar height 0–1. Below floor reads as silence; ceil is a
/// full bar. Tuned to measured mic levels (speech ≈ -60 dB, idle ≈ -90 dB).
const DB_FLOOR: f32 = -82.0;
const DB_CEIL: f32 = -52.0;
/// Per-band gain (dB) compensating for voice energy concentrating in low bands,
/// so higher bars still move. Low → high.
const BAND_TILT_DB: [f32; BANDS] = [0.0, 0.0, 4.0, 8.0, 12.0, 16.0, 20.0, 24.0];
/// Decay smoothing for falling bars (instant rise). 0 = none, →1 = slow.
const DECAY: f32 = 0.6;

pub struct WaveformMeter {
    /// Ring buffer + write position + rate. Locked only by the audio callback
    /// (`push`) and briefly by `levels` to snapshot — never held across the FFT.
    inner: Mutex<State>,
    /// Smoothed bar levels, owned by `levels` (emitter task only).
    smoothed: Mutex<[f32; BANDS]>,
    fft: std::sync::Arc<dyn Fft<f32>>,
    window: [f32; FFT_SIZE],
}

struct State {
    ring: [f32; FFT_SIZE],
    pos: usize,
    sample_rate: f32,
}

impl WaveformMeter {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
        let window = std::array::from_fn(|i| {
            #[allow(clippy::cast_precision_loss)]
            let phase = std::f32::consts::TAU * i as f32 / (FFT_SIZE as f32 - 1.0);
            0.5 - 0.5 * phase.cos()
        });
        Self {
            inner: Mutex::new(State::new(sample_rate)),
            smoothed: Mutex::new([0.0; BANDS]),
            fft,
            window,
        }
    }

    /// Re-arm for a new device sample rate and clear state (called on capture start).
    pub fn reset(&self, sample_rate: u32) {
        if let Ok(mut s) = self.inner.lock() {
            *s = State::new(sample_rate);
        }
        if let Ok(mut levels) = self.smoothed.lock() {
            *levels = [0.0; BANDS];
        }
    }

    /// Append mono samples into the ring buffer (cheap; called from cpal callback).
    pub fn push(&self, mono: &[f32]) {
        let Ok(mut s) = self.inner.lock() else { return };
        for &sample in mono {
            let pos = s.pos;
            s.ring[pos] = sample;
            s.pos = (pos + 1) % FFT_SIZE;
        }
    }

    /// Compute current bar levels (0.0–1.0). Snapshots the ring under the lock,
    /// then releases it before running the FFT — so the audio callback never
    /// blocks on the FFT.
    pub fn levels(&self) -> [f32; BANDS] {
        let mut buf = [Complex { re: 0.0, im: 0.0 }; FFT_SIZE];
        let sample_rate = {
            let Ok(s) = self.inner.lock() else {
                return [0.0; BANDS];
            };
            for (i, slot) in buf.iter_mut().enumerate() {
                slot.re = s.ring[(s.pos + i) % FFT_SIZE] * self.window[i];
            }
            s.sample_rate
        };

        self.fft.process(&mut buf);

        #[allow(clippy::cast_precision_loss)]
        let bin_hz = sample_rate / FFT_SIZE as f32;
        let nyquist_bin = FFT_SIZE / 2;

        let Ok(mut levels) = self.smoothed.lock() else {
            return [0.0; BANDS];
        };
        for b in 0..BANDS {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let lo = ((BAND_EDGES[b] / bin_hz) as usize).max(1);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let hi = ((BAND_EDGES[b + 1] / bin_hz) as usize).min(nyquist_bin).max(lo + 1);

            let mut sum = 0.0_f32;
            for bin in &buf[lo..hi] {
                sum += bin.norm_sqr();
            }
            #[allow(clippy::cast_precision_loss)]
            let rms = (sum / (hi - lo) as f32).sqrt() / FFT_SIZE as f32;

            let db = 20.0 * (rms.max(1e-9)).log10() + BAND_TILT_DB[b];
            let target = ((db - DB_FLOOR) / (DB_CEIL - DB_FLOOR)).clamp(0.0, 1.0);

            levels[b] = if target >= levels[b] {
                target
            } else {
                levels[b] * DECAY + target * (1.0 - DECAY)
            };
        }

        *levels
    }
}

impl State {
    fn new(sample_rate: u32) -> Self {
        #[allow(clippy::cast_precision_loss)]
        let sr = sample_rate as f32;
        Self {
            ring: [0.0; FFT_SIZE],
            pos: 0,
            sample_rate: sr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WaveformMeter, BANDS, FFT_SIZE};
    use std::f32::consts::TAU;

    const SR: u32 = 48_000;

    #[allow(clippy::cast_precision_loss)]
    fn levels_for_tone(meter: &WaveformMeter, freq: f32, amp: f32) -> [f32; BANDS] {
        let sr = SR as f32;
        let samples: Vec<f32> = (0..FFT_SIZE * 4)
            .map(|i| amp * (TAU * freq * i as f32 / sr).sin())
            .collect();
        for block in samples.chunks(256) {
            meter.push(block);
        }
        meter.levels()
    }

    #[test]
    fn silence_reads_flat() {
        let meter = WaveformMeter::new(SR);
        meter.push(&vec![0.0; FFT_SIZE]);
        assert!(meter.levels().iter().all(|&l| l == 0.0));
    }

    #[test]
    fn low_amplitude_stays_near_floor() {
        let meter = WaveformMeter::new(SR);
        let levels = levels_for_tone(&meter, 1000.0, 0.0002);
        assert!(levels.iter().all(|&l| l < 0.05), "got {levels:?}");
    }

    #[test]
    fn tone_peaks_in_its_own_band() {
        let meter = WaveformMeter::new(SR);
        // 1000 Hz falls in band 4 (850–1300 Hz).
        let levels = levels_for_tone(&meter, 1000.0, 0.1);
        let loudest = levels
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(loudest, 4, "got {levels:?}");
    }

    #[test]
    fn separated_tones_drive_separate_bands() {
        let meter = WaveformMeter::new(SR);
        // A low tone (200 Hz, band 1) should not pin a high band (3000 Hz, band 7).
        let low = levels_for_tone(&meter, 200.0, 0.1);
        assert!(low[1] > low[7], "low tone leaked to high band: {low:?}");
    }
}
