use voice_activity_detector::{IteratorExt, VoiceActivityDetector};

// Silero VAD parameters at 16 kHz
const VAD_SAMPLE_RATE: i64 = 16_000;
// 512 samples = 32 ms at 16 kHz — only chunk size supported by Silero V5 at 16k
const VAD_CHUNK: usize = 512;
// Speech probability threshold for speech extraction.
// Deliberately lower than chunker.rs (0.5) — here we want to keep quiet syllables
// and soft consonants that would otherwise be clipped at a higher threshold.
// chunker.rs uses a higher threshold (0.5) for a different purpose: finding clear
// silence gaps as chunk split points, not filtering speech frames.
const VAD_THRESHOLD: f32 = 0.35;
// Padding frames before/after each speech segment (12 × 32ms = 384ms).
// Bridges natural inter-word pauses without cutting speech.
const VAD_PAD_FRAMES: usize = 12;
// Minimum consecutive non-speech frames before ending a segment (~15 × 32ms = 480ms).
// Prevents short pauses between words from splitting the audio.
const MIN_SILENCE_FRAMES: usize = 15;

/// Apply Silero VAD V5: classify each 32ms frame, collect speech segments with
/// padding, and concatenate into a single buffer ready for Whisper.
/// Falls back to the full buffer if VAD init fails.
pub fn extract_speech(samples: &[f32]) -> Vec<f32> {
    let Ok(mut vad) = VoiceActivityDetector::builder()
        .sample_rate(VAD_SAMPLE_RATE)
        .chunk_size(VAD_CHUNK)
        .build() else { return samples.to_vec() };

    let predictions: Vec<(Vec<f32>, f32)> = samples
        .iter()
        .copied()
        .predict(&mut vad)
        .map(|(chunk, prob)| (chunk.clone(), prob))
        .collect();

    let n = predictions.len();
    if n == 0 {
        return Vec::new();
    }

    let speech_mask: Vec<bool> = predictions
        .iter()
        .map(|(_, p)| *p >= VAD_THRESHOLD)
        .collect();

    let any_speech = speech_mask.iter().any(|&s| s);
    if !any_speech {
        return trim_silence_rms(samples);
    }

    // Apply minimum silence duration: only mark a frame as silent if it belongs
    // to a run of ≥ MIN_SILENCE_FRAMES consecutive non-speech frames.
    // Short pauses (< MIN_SILENCE_FRAMES) are treated as speech to avoid cutting words.
    let gated_mask: Vec<bool> = {
        let mut mask = speech_mask.clone();
        let mut i = 0;
        while i < n {
            if speech_mask[i] {
                i += 1;
            } else {
                // Measure silence run length
                let run_start = i;
                while i < n && !speech_mask[i] {
                    i += 1;
                }
                let run_len = i - run_start;
                if run_len < MIN_SILENCE_FRAMES {
                    // Short gap — treat as speech to bridge the pause
                    mask[run_start..i].fill(true);
                }
            }
        }
        mask
    };

    // Apply padding around remaining speech segments
    let padded_mask: Vec<bool> = (0..n)
        .map(|i| {
            let lo = i.saturating_sub(VAD_PAD_FRAMES);
            let hi = (i + VAD_PAD_FRAMES + 1).min(n);
            gated_mask[lo..hi].iter().any(|&s| s)
        })
        .collect();

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
        .map(|w| {
            #[allow(clippy::cast_precision_loss)] // window ≤ 160 samples, fits f32 exactly
            (w.iter().map(|&s| s * s).sum::<f32>() / w.len() as f32).sqrt()
        })
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
