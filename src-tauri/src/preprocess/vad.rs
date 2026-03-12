use voice_activity_detector::{IteratorExt, VoiceActivityDetector};

// Silero VAD parameters at 16 kHz
const VAD_SAMPLE_RATE: i64 = 16_000;
// 512 samples = 32 ms at 16 kHz — only chunk size supported by Silero V5 at 16k
const VAD_CHUNK: usize = 512;
// Speech probability threshold — frames above this are kept
const VAD_THRESHOLD: f32 = 0.5;
// Number of 32ms frames to pad before/after each speech segment (2 × 32ms = 64ms)
const VAD_PAD_FRAMES: usize = 2;

/// Apply Silero VAD V5: classify each 32ms frame, collect speech segments with
/// padding, and concatenate into a single buffer ready for Whisper.
/// Falls back to the full buffer if VAD init fails.
pub fn extract_speech(samples: &[f32]) -> Vec<f32> {
    let mut vad = match VoiceActivityDetector::builder()
        .sample_rate(VAD_SAMPLE_RATE)
        .chunk_size(VAD_CHUNK)
        .build()
    {
        Ok(v) => v,
        Err(_) => return samples.to_vec(),
    };

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

    let speech_mask: Vec<bool> = predictions
        .iter()
        .map(|(_, p)| *p >= VAD_THRESHOLD)
        .collect();

    let padded_mask: Vec<bool> = (0..n)
        .map(|i| {
            let lo = i.saturating_sub(VAD_PAD_FRAMES);
            let hi = (i + VAD_PAD_FRAMES + 1).min(n);
            speech_mask[lo..hi].iter().any(|&s| s)
        })
        .collect();

    let any_speech = speech_mask.iter().any(|&s| s);
    if !any_speech {
        return trim_silence_rms(samples);
    }

    let mut out = Vec::with_capacity(samples.len());
    for (i, (chunk, _)) in predictions.into_iter().enumerate() {
        if padded_mask[i] {
            out.extend_from_slice(&chunk);
        }
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
    let last = windows
        .iter()
        .rposition(|&r| r >= 0.01)
        .unwrap_or(windows.len() - 1);
    let start = first.saturating_sub(1) * WINDOW;
    let end = ((last + 2) * WINDOW).min(samples.len());
    samples[start..end].to_vec()
}
