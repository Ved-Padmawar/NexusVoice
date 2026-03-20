pub mod chunker;
pub mod denoise;
pub mod vad;

use crate::audio::resample;

/// Full preprocessing pipeline: native_rate → DC offset removal → 48k denoise → 16k VAD → peak normalize → speech only.
pub fn preprocess(samples: &[f32], native_rate: u32) -> Vec<f32> {
    // 1. Resample to 48 kHz for nnnoiseless
    let at_48k = resample(samples, native_rate, 48_000);

    // 2. DC offset removal — subtract signal mean before denoising.
    //    Budget USB mics often have a non-zero DC bias that distorts the mel
    //    spectrogram inside Whisper. Mean subtraction eliminates it cheaply.
    let mean = at_48k.iter().copied().sum::<f32>() / at_48k.len().max(1) as f32;
    let at_48k: Vec<f32> = at_48k.iter().map(|s| s - mean).collect();

    // 3. Noise suppression (RNNoise, 480-sample frames at 48 kHz)
    let denoised = denoise::denoise(&at_48k);

    // 4. Resample to 16 kHz for Whisper + VAD
    let at_16k = resample(&denoised, 48_000, 16_000);

    // 5. VAD — keep only speech frames
    let speech = vad::extract_speech(&at_16k);

    // 6. Peak normalization — target –3 dBFS (peak ≈ 0.707).
    //    Whisper's mel spectrogram extraction is sensitive to signal level;
    //    quiet microphones produce poor transcriptions without normalization.
    let peak = speech.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak > 1e-6 {
        let gain = 0.707 / peak;
        speech.iter().map(|s| (s * gain).clamp(-1.0, 1.0)).collect()
    } else {
        speech
    }
}
