pub mod denoise;
pub mod vad;

use crate::audio::resample;

/// Full preprocessing pipeline: `native_rate` → DC offset removal → 48k denoise → 16k VAD → peak normalize → speech only.
pub fn preprocess(samples: &[f32], native_rate: u32) -> Vec<f32> {
    splice_normalize(&to_16k_denoised(samples, native_rate))
}

/// Stage 1: resample → DC removal → denoise → 16 kHz. No VAD splicing, so
/// sample indices still map linearly back to native-rate time — required by
/// the streaming pipeline's split/cursor math.
pub fn to_16k_denoised(samples: &[f32], native_rate: u32) -> Vec<f32> {
    // 1. Resample to 48 kHz for nnnoiseless
    let at_48k = resample(samples, native_rate, 48_000);

    // 2. DC offset removal — subtract signal mean before denoising.
    //    Budget USB mics often have a non-zero DC bias that distorts acoustic
    //    features. Mean subtraction eliminates it cheaply.
    #[allow(clippy::cast_precision_loss)] // buffer len fits f32 at audio sizes
    let mean = at_48k.iter().copied().sum::<f32>() / at_48k.len().max(1) as f32;
    let at_48k: Vec<f32> = at_48k.iter().map(|s| s - mean).collect();

    // 3. Noise suppression (RNNoise, 480-sample frames at 48 kHz)
    let denoised = denoise::denoise(&at_48k);

    // 4. Resample to 16 kHz for parakeet.cpp + VAD
    resample(&denoised, 48_000, 16_000)
}

/// Stage 2: VAD speech splicing + peak normalization — produces the buffer
/// fed to parakeet.cpp. Splicing removes/shortens silence, so indices in the
/// output no longer correspond to input time.
pub fn splice_normalize(at_16k: &[f32]) -> Vec<f32> {
    // 5. VAD — keep only speech frames
    let speech = vad::extract_speech(at_16k);

    // 6. Peak normalization — target –3 dBFS (peak ≈ 0.707).
    //    Stable input level improves acoustic-model behavior on quiet microphones.
    let peak = speech.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak > 1e-6 {
        let gain = 0.707 / peak;
        speech.iter().map(|s| (s * gain).clamp(-1.0, 1.0)).collect()
    } else {
        speech
    }
}
