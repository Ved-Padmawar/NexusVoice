//! Fake speech levels (syllables, phonemes, breaths) for the previews; random
//! noise would make every style look alike.

use crate::audio::waveform::BANDS;
use crate::ui::pill::waveform::{ease, f};

/// A step is never longer than this, so a stalled window does not lurch.
const MAX_DT: f32 = 0.1;

pub struct SpeechLevels {
    smooth: [f32; BANDS],
    time: f32,
    phrase_end: f32,
    breathing: bool,
    phone: f32,
    phone_end: f32,
    vowel: bool,
    rng: u32,
}

impl SpeechLevels {
    pub const fn new() -> Self {
        Self {
            smooth: [0.0; BANDS],
            time: 0.0,
            phrase_end: 2.4,
            breathing: false,
            phone: 0.0,
            phone_end: 0.0,
            vowel: true,
            rng: 0x9e37_79b9,
        }
    }

    /// Uniform in [0, 1) via xorshift.
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        // 24 bits fit f32's mantissa exactly.
        #[allow(clippy::cast_precision_loss)]
        let bits = (self.rng >> 8) as f32;
        bits / 16_777_216.0
    }

    pub fn step(&mut self, dt: f32) -> [f32; BANDS] {
        let dt = dt.clamp(0.0, MAX_DT);
        self.time += dt;
        if self.time > self.phrase_end {
            self.breathing = !self.breathing;
            let length = if self.breathing {
                0.45 + self.random() * 0.35
            } else {
                1.8 + self.random() * 2.2
            };
            self.phrase_end = self.time + length;
        }

        let target = if self.breathing {
            std::array::from_fn(|_| self.random() * 0.03)
        } else {
            self.syllable_target()
        };
        for (i, (smooth, target)) in self.smooth.iter_mut().zip(target).enumerate() {
            let factor = if target > *smooth {
                0.55
            } else {
                0.12 + f(i) * 0.008
            };
            *smooth += (target - *smooth) * ease(factor, dt);
        }
        self.smooth
    }

    /// Vowels load the low bands, consonants the highs, so bands move independently.
    fn syllable_target(&mut self) -> [f32; BANDS] {
        if self.time > self.phone_end {
            self.vowel = self.random() > 0.42;
            self.phone = self.random();
            let length = if self.vowel {
                0.09 + self.random() * 0.13
            } else {
                0.04 + self.random() * 0.06
            };
            self.phone_end = self.time + length;
        }
        let (centre, width, gain) = if self.vowel {
            (0.6 + self.phone * 1.9, 1.5, 0.95)
        } else {
            (4.4 + self.phone * 2.6, 2.1, 0.62)
        };
        let word = 0.6 + 0.4 * (self.time * 1.7 + 0.8).sin();
        std::array::from_fn(|i| {
            let band = (-((f(i) - centre) / width).powi(2)).exp();
            let jitter = 0.78 + self.random() * 0.44;
            (band * gain * word * jitter * 1.5).min(1.0)
        })
    }
}
