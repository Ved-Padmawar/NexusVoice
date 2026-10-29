use crate::audio::StreamResampler;

const TARGET_RATE: u32 = 16_000;

/// Converts a recording to 16 kHz as it arrives. No VAD splicing, so sample
/// indices still map linearly back to native-rate time.
pub struct To16k {
    resampler: Option<StreamResampler>,
}

impl To16k {
    pub fn new(native_rate: u32) -> Result<Self, String> {
        if native_rate == 0 {
            return Err("microphone sample rate is unknown".to_string());
        }
        let resampler = if native_rate == TARGET_RATE {
            None
        } else {
            Some(StreamResampler::new(native_rate, TARGET_RATE)?)
        };
        Ok(Self { resampler })
    }

    pub fn push(&mut self, samples: &[f32]) -> Result<Vec<f32>, String> {
        // A glitching device can emit NaN/Inf, which aborts downstream FFTs.
        let sanitized: Vec<f32> = samples
            .iter()
            .map(|s| {
                if s.is_finite() {
                    s.clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            })
            .collect();
        match &mut self.resampler {
            Some(resampler) => resampler.push(&sanitized),
            None => Ok(sanitized),
        }
    }

    pub fn finish(&mut self) -> Result<Vec<f32>, String> {
        match &mut self.resampler {
            Some(resampler) => resampler.finish(),
            None => Ok(Vec::new()),
        }
    }
}

/// Peak normalization to –3 dBFS (peak ≈ 0.707), timeline intact.
///
/// Whisper's mel extraction is level-sensitive and quiet mics transcribe poorly
/// without gain. Silence stays in place, so output indices still map to input
/// time — required for word timestamps to mean anything.
pub fn normalize_level(at_16k: &[f32]) -> Vec<f32> {
    let peak = at_16k.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak > 1e-6 {
        let gain = 0.707 / peak;
        at_16k.iter().map(|s| (s * gain).clamp(-1.0, 1.0)).collect()
    } else {
        at_16k.to_vec()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // asserts compare exact-representable values
#[path = "../../tests/unit/models/preprocess.rs"]
mod tests;
