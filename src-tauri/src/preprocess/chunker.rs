/// VAD-aware audio chunker for long recordings.
///
/// Strategy (mirrors Whisper's own long-form algorithm):
///   1. Classify every 32ms frame with Silero VAD.
///   2. Find silence gaps (runs of non-speech frames ≥ MIN_SILENCE_FRAMES).
///   3. Split at the silence gap closest to the TARGET_CHUNK_SAMPLES boundary.
///   4. Add OVERLAP_SAMPLES of audio from the *end* of the previous chunk to
///      the *start* of the next chunk.
///   5. After all chunks are transcribed, stitch by finding the longest common
///      word-sequence in the overlapping region and merging there.
///
/// If the entire recording fits within MAX_CHUNK_SAMPLES the buffer is returned
/// as a single chunk (no-op fast path).
use voice_activity_detector::{IteratorExt, VoiceActivityDetector};

// All sample counts are at 16 kHz (post-preprocess rate).
const SAMPLE_RATE: usize = 16_000;
/// Target chunk length before we look for a split point (~25 s).
const TARGET_CHUNK_SAMPLES: usize = 25 * SAMPLE_RATE;
/// Hard ceiling — never exceed Whisper's 30 s context window.
const MAX_CHUNK_SAMPLES: usize = 30 * SAMPLE_RATE;
/// Overlap added to each side of a boundary (~0.5 s).
const OVERLAP_SAMPLES: usize = SAMPLE_RATE / 2;
/// VAD frame size (Silero V5 at 16 kHz).
const VAD_CHUNK: usize = 512;
/// Speech threshold for silence-gap detection (split point finder only).
/// Deliberately higher than vad.rs (0.35) — here we need *clear* silence gaps
/// to split at, not speech extraction. A higher threshold avoids splitting at
/// brief inter-word pauses that vad.rs would correctly keep as speech.
const VAD_THRESHOLD: f32 = 0.5;
/// Minimum run of silent frames to be considered a valid split point (~160 ms).
const MIN_SILENCE_FRAMES: usize = 5;

/// A single chunk ready for Whisper inference.
#[derive(Debug)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    /// True when this chunk carries overlap audio prepended from the previous chunk.
    #[allow(dead_code)]
    pub has_leading_overlap: bool,
}

/// Split `samples` (16 kHz mono f32) into Whisper-sized chunks.
/// Returns a single-element vec when the recording is short enough.
pub fn chunk_audio(samples: &[f32]) -> Vec<AudioChunk> {
    if samples.len() <= MAX_CHUNK_SAMPLES {
        return vec![AudioChunk {
            samples: samples.to_vec(),
            has_leading_overlap: false,
        }];
    }

    let silence_map = build_silence_map(samples);
    let mut chunks = Vec::new();
    let mut pos: usize = 0;

    while pos < samples.len() {
        let remaining = samples.len() - pos;
        if remaining <= MAX_CHUNK_SAMPLES {
            // Last chunk — take everything left
            let chunk_samples = samples[pos..].to_vec();
            chunks.push(AudioChunk {
                samples: chunk_samples,
                has_leading_overlap: pos > 0,
            });
            break;
        }

        // Find best split: silence gap nearest TARGET from current position
        let target_abs = pos + TARGET_CHUNK_SAMPLES;
        let split = find_split_point(&silence_map, pos, target_abs, samples.len());

        // Build this chunk: [pos .. split]
        let chunk_end = split.min(samples.len());
        let chunk_samples = samples[pos..chunk_end].to_vec();
        chunks.push(AudioChunk {
            samples: chunk_samples,
            has_leading_overlap: pos > 0,
        });

        // Next chunk starts OVERLAP_SAMPLES before the split so stitching can
        // find a common word boundary.
        pos = split.saturating_sub(OVERLAP_SAMPLES);
    }

    chunks
}

/// Stitch a list of transcribed chunk texts into a single string.
///
/// For adjacent chunk pairs where the later chunk had leading overlap,
/// we find the longest common word subsequence in the tail/head of the
/// pair and merge at that boundary, eliminating duplicated words.
pub fn stitch_transcripts(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    if parts.len() == 1 {
        return parts[0].trim().to_string();
    }

    let mut result = parts[0].trim().to_string();

    for next in &parts[1..] {
        let next = next.trim();
        if next.is_empty() {
            continue;
        }
        result = merge_pair(&result, next);
    }

    result
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a per-frame silence boolean map using Silero VAD.
/// Index i → true means frame i is silent (non-speech).
fn build_silence_map(samples: &[f32]) -> Vec<bool> {
    let mut vad = match VoiceActivityDetector::builder()
        .sample_rate(SAMPLE_RATE as i64)
        .chunk_size(VAD_CHUNK)
        .build()
    {
        Ok(v) => v,
        // VAD unavailable — treat everything as speech (no good split points)
        Err(_) => {
            return vec![false; samples.len() / VAD_CHUNK + 1];
        }
    };

    samples
        .iter()
        .copied()
        .predict(&mut vad)
        .map(|(_, prob)| prob < VAD_THRESHOLD)
        .collect()
}

/// Find the sample index of the best split point near `target_abs`.
/// Preference: a silence run of ≥ MIN_SILENCE_FRAMES centred as close to
/// target as possible. Falls back to `target_abs` if no silence is found.
fn find_split_point(
    silence_map: &[bool],
    chunk_start: usize,
    target_abs: usize,
    total_samples: usize,
) -> usize {
    // Search window: ±5 s around target
    let search_half = 5 * SAMPLE_RATE;
    let search_start_sample = target_abs.saturating_sub(search_half).max(chunk_start);
    let search_end_sample = (target_abs + search_half).min(total_samples);

    // Convert sample positions to frame indices
    let frame_start = search_start_sample / VAD_CHUNK;
    let frame_end = (search_end_sample / VAD_CHUNK).min(silence_map.len());
    let target_frame = target_abs / VAD_CHUNK;

    let mut best_split: Option<usize> = None;
    let mut best_dist = usize::MAX;

    let mut i = frame_start;
    while i < frame_end {
        if silence_map[i] {
            // Measure run length
            let run_start = i;
            while i < frame_end && silence_map[i] {
                i += 1;
            }
            let run_len = i - run_start;
            if run_len >= MIN_SILENCE_FRAMES {
                // Use the centre of the silence run as the split point
                let mid_frame = run_start + run_len / 2;
                let dist = mid_frame.abs_diff(target_frame);
                if dist < best_dist {
                    best_dist = dist;
                    best_split = Some(mid_frame * VAD_CHUNK);
                }
            }
        } else {
            i += 1;
        }
    }

    best_split.unwrap_or(target_abs).min(total_samples)
}

/// Merge two adjacent transcript strings by removing duplicated words at the
/// boundary introduced by the overlap window.
///
/// Algorithm: look at the last N words of `prev` and the first N words of
/// `next` (N = STITCH_WINDOW). Find the longest suffix of `prev_words` that
/// matches a prefix of `next_words`, then concatenate without the duplicate.
fn merge_pair(prev: &str, next: &str) -> String {
    const STITCH_WINDOW: usize = 12; // words to examine at each boundary

    let prev_words: Vec<&str> = prev.split_whitespace().collect();
    let next_words: Vec<&str> = next.split_whitespace().collect();

    let pw = prev_words.len().min(STITCH_WINDOW);
    let nw = next_words.len().min(STITCH_WINDOW);

    let prev_tail = &prev_words[prev_words.len() - pw..];
    let next_head = &next_words[..nw];

    // Find longest overlap: suffix of prev_tail == prefix of next_head
    let mut best_len = 0usize;
    for len in 1..=pw.min(nw) {
        if prev_tail[pw - len..] == next_head[..len] {
            best_len = len;
        }
    }

    if best_len > 0 {
        // Drop the overlapping words from next and join
        let next_remainder = next_words[best_len..].join(" ");
        if next_remainder.is_empty() {
            prev.to_string()
        } else {
            format!("{prev} {next_remainder}")
        }
    } else {
        // No overlap found — simple join with a space
        format!("{prev} {next}")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_audio_is_single_chunk() {
        let samples = vec![0f32; SAMPLE_RATE * 10]; // 10 s
        let chunks = chunk_audio(&samples);
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].has_leading_overlap);
    }

    #[test]
    fn long_audio_produces_multiple_chunks() {
        // 70 s of audio — must split into ≥ 3 chunks
        let samples = vec![0f32; SAMPLE_RATE * 70];
        let chunks = chunk_audio(&samples);
        assert!(chunks.len() >= 3, "expected ≥3 chunks, got {}", chunks.len());
        for c in &chunks {
            assert!(
                c.samples.len() <= MAX_CHUNK_SAMPLES + OVERLAP_SAMPLES,
                "chunk too large: {}",
                c.samples.len()
            );
        }
    }

    #[test]
    fn stitch_removes_duplicate_words() {
        let a = "hello world this is a test".to_string();
        let b = "this is a test and more words".to_string();
        let result = stitch_transcripts(&[a, b]);
        // "this is a test" should not appear twice
        let count = result.matches("this is a test").count();
        assert_eq!(count, 1, "duplicate overlap not removed: {result}");
        assert!(result.contains("and more words"), "tail missing: {result}");
    }

    #[test]
    fn stitch_no_overlap_joins_with_space() {
        let a = "hello world".to_string();
        let b = "goodbye world".to_string();
        let result = stitch_transcripts(&[a, b]);
        assert_eq!(result, "hello world goodbye world");
    }

    #[test]
    fn stitch_single_part_returns_as_is() {
        let result = stitch_transcripts(&["only one".to_string()]);
        assert_eq!(result, "only one");
    }
}
