//! Audio preprocessing pipeline for Whisper transcription quality.
//!
//! Steps applied before passing audio to Whisper:
//!   1. Resample to 48 kHz (required by nnnoiseless / RNNoise)
//!   2. Noise suppression via nnnoiseless (RNNoise neural model)
//!   3. Resample from 48 kHz to 16 kHz (required by Whisper + VAD)
//!   4. VAD — Silero VAD V5 classifies 512-sample (32ms) frames; only
//!      confirmed speech frames + padding are kept, silence is discarded
//!      (replaces the old RMS-threshold trim heuristic)

use nnnoiseless::DenoiseState;
use voice_activity_detector::{IteratorExt, VoiceActivityDetector};

// Silero VAD parameters at 16 kHz
const VAD_SAMPLE_RATE: i64 = 16_000;
// 512 samples = 32 ms at 16 kHz — only chunk size supported by Silero V5 at 16k
const VAD_CHUNK: usize = 512;
// Speech probability threshold — frames above this are kept (0.5 is standard)
const VAD_THRESHOLD: f32 = 0.5;
// Number of 32ms frames to pad before/after each speech segment (2 × 32ms = 64ms)
const VAD_PAD_FRAMES: usize = 2;

/// Full preprocessing pipeline: native_rate → 48k denoise → 16k VAD → speech only.
pub fn preprocess(samples: &[f32], native_rate: u32) -> Vec<f32> {
    // 1. Resample to 48 kHz for nnnoiseless
    let at_48k = resample(samples, native_rate, 48_000);

    // 2. Noise suppression (RNNoise, 480-sample frames at 48 kHz)
    let denoised = denoise(&at_48k);

    // 3. Resample to 16 kHz for Whisper + VAD
    let at_16k = resample(&denoised, 48_000, 16_000);

    // 4. VAD — keep only speech frames
    extract_speech(&at_16k)
}

/// Apply Silero VAD V5: classify each 32ms frame, collect speech segments with
/// padding, and concatenate into a single buffer ready for Whisper.
/// Falls back to the full buffer if VAD init fails (should never happen).
fn extract_speech(samples: &[f32]) -> Vec<f32> {
    let mut vad = match VoiceActivityDetector::builder()
        .sample_rate(VAD_SAMPLE_RATE)
        .chunk_size(VAD_CHUNK)
        .build()
    {
        Ok(v) => v,
        Err(_) => return samples.to_vec(), // graceful fallback
    };

    // Collect (chunk, probability) pairs
    let predictions: Vec<(Vec<f32>, f32)> = samples
        .iter()
        .copied()
        .predict(&mut vad)
        .map(|(chunk, prob)| (chunk.to_vec(), prob))
        .collect();

    let n = predictions.len();
    if n == 0 {
        return Vec::new();
    }

    // Build a boolean mask: true = speech frame
    let speech_mask: Vec<bool> = predictions.iter().map(|(_, p)| *p >= VAD_THRESHOLD).collect();

    // Expand mask with padding: any frame within VAD_PAD_FRAMES of a speech frame is kept
    let padded_mask: Vec<bool> = (0..n)
        .map(|i| {
            let lo = i.saturating_sub(VAD_PAD_FRAMES);
            let hi = (i + VAD_PAD_FRAMES + 1).min(n);
            speech_mask[lo..hi].iter().any(|&s| s)
        })
        .collect();

    // Check if any speech was detected — if not, return the full buffer so
    // Whisper can produce an empty/silence result rather than us silently
    // dropping everything (prevents missed short utterances near threshold)
    let any_speech = speech_mask.iter().any(|&s| s);
    if !any_speech {
        // No speech detected: return trimmed-by-rms fallback so Whisper gets
        // a short buffer and emits empty string cleanly
        return trim_silence_rms(samples);
    }

    // Concatenate kept frames
    let mut out = Vec::with_capacity(samples.len());
    for (i, (chunk, _)) in predictions.into_iter().enumerate() {
        if padded_mask[i] {
            out.extend_from_slice(&chunk);
        }
    }
    out
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
fn denoise(samples: &[f32]) -> Vec<f32> {
    const FRAME: usize = DenoiseState::FRAME_SIZE;
    let mut state = DenoiseState::new();
    let mut out = Vec::with_capacity(samples.len());

    // nnnoiseless expects f32 in the range of i16 (-32768..32768)
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
        out.extend(frame_out[..len].iter().map(|&s| s / 32768.0));
    }
    out
}

/// Simple RMS silence trim — used only as a fallback when VAD detects no speech.
fn trim_silence_rms(samples: &[f32]) -> Vec<f32> {
    const WINDOW: usize = 160; // 10ms at 16kHz
    if samples.is_empty() {
        return samples.to_vec();
    }
    let windows: Vec<f32> = samples
        .chunks(WINDOW)
        .map(|w| (w.iter().map(|&s| s * s).sum::<f32>() / w.len() as f32).sqrt())
        .collect();
    let first = windows.iter().position(|&r| r >= 0.01).unwrap_or(0);
    let last = windows.iter().rposition(|&r| r >= 0.01).unwrap_or(windows.len() - 1);
    let start = first.saturating_sub(1) * WINDOW;
    let end = ((last + 2) * WINDOW).min(samples.len());
    samples[start..end].to_vec()
}
