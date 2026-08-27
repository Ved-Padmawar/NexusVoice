use crate::audio::resample;

/// Resample to 16 kHz. No VAD splicing, so sample indices still map linearly
/// back to native-rate time.
pub fn to_16k(samples: &[f32], native_rate: u32) -> Vec<f32> {
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

    if native_rate == 16_000 {
        sanitized
    } else {
        resample(&sanitized, native_rate, 16_000)
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
