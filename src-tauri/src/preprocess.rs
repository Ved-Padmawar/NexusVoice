//! Audio preprocessing pipeline for Whisper transcription quality.
//!
//! Steps applied before passing audio to Whisper:
//!   1. Resample to 48 kHz (required by nnnoiseless / RNNoise)
//!   2. Noise suppression via nnnoiseless (RNNoise neural model)
//!   3. Resample from 48 kHz to 16 kHz (required by Whisper)
//!   4. Silence trim — strip leading/trailing frames below RMS threshold

use nnnoiseless::DenoiseState;

/// Full preprocessing pipeline: native_rate → 48k denoise → 16k trim.
pub fn preprocess(samples: &[f32], native_rate: u32) -> Vec<f32> {
    // 1. Resample to 48 kHz for nnnoiseless
    let at_48k = resample(samples, native_rate, 48_000);

    // 2. Noise suppression (RNNoise, 480-sample frames at 48 kHz)
    let denoised = denoise(&at_48k);

    // 3. Resample to 16 kHz for Whisper
    let at_16k = resample(&denoised, 48_000, 16_000);

    // 4. Trim leading/trailing silence
    trim_silence(&at_16k, 0.01)
}

/// Linear resampler — sufficient quality for speech.
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = ((samples.len() as f64) * ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src as usize;
        let frac = src - idx as f64;
        let a = *samples.get(idx).unwrap_or(&0.0);
        let b = *samples.get(idx + 1).unwrap_or(&a);
        out.push(a + (b - a) * frac as f32);
    }
    out
}

/// Apply RNNoise frame-by-frame. Expects samples at 48 kHz.
/// Converts i16-range floats (-32768..32768) as expected by nnnoiseless.
fn denoise(samples: &[f32]) -> Vec<f32> {
    const FRAME: usize = DenoiseState::FRAME_SIZE;
    let mut state = DenoiseState::new();
    let mut out = Vec::with_capacity(samples.len());

    // nnnoiseless expects f32 in the range of i16 (-32768..32768)
    // Our samples are in -1.0..1.0, so scale up then back down
    let scaled: Vec<f32> = samples.iter().map(|&s| s * 32768.0).collect();

    let mut frame_in = [0.0f32; FRAME];
    let mut frame_out = [0.0f32; FRAME];

    for chunk in scaled.chunks(FRAME) {
        let len = chunk.len();
        frame_in[..len].copy_from_slice(chunk);
        if len < FRAME {
            frame_in[len..].fill(0.0);
        }
        state.process_frame(&mut frame_out, &frame_in);
        // Only push the valid samples (not the zero-padded tail)
        out.extend(frame_out[..len].iter().map(|&s| s / 32768.0));
    }
    out
}

/// Remove leading and trailing silence below `rms_threshold`.
/// Operates on 10ms windows at 16 kHz (160 samples).
fn trim_silence(samples: &[f32], rms_threshold: f32) -> Vec<f32> {
    const WINDOW: usize = 160; // 10ms at 16kHz
    if samples.is_empty() {
        return samples.to_vec();
    }

    let windows: Vec<f32> = samples
        .chunks(WINDOW)
        .map(|w| {
            let rms = (w.iter().map(|&s| s * s).sum::<f32>() / w.len() as f32).sqrt();
            rms
        })
        .collect();

    let first = windows.iter().position(|&r| r >= rms_threshold).unwrap_or(0);
    let last = windows.iter().rposition(|&r| r >= rms_threshold).unwrap_or(windows.len() - 1);

    // Add 1-window padding on each side to avoid clipping
    let start = first.saturating_sub(1) * WINDOW;
    let end = ((last + 2) * WINDOW).min(samples.len());

    samples[start..end].to_vec()
}
