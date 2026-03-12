pub mod denoise;
pub mod vad;

use crate::audio::resample;

/// Full preprocessing pipeline: native_rate → 48k denoise → 16k VAD → speech only.
pub fn preprocess(samples: &[f32], native_rate: u32) -> Vec<f32> {
    // 1. Resample to 48 kHz for nnnoiseless
    let at_48k = resample(samples, native_rate, 48_000);

    // 2. Noise suppression (RNNoise, 480-sample frames at 48 kHz)
    let denoised = denoise::denoise(&at_48k);

    // 3. Resample to 16 kHz for Whisper + VAD
    let at_16k = resample(&denoised, 48_000, 16_000);

    // 4. VAD — keep only speech frames
    vad::extract_speech(&at_16k)
}
