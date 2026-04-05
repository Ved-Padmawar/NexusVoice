//! Streaming transcription pipeline.
//!
//! While the user holds the hotkey, audio accumulates in the shared buffer.
//! A background poller calls `try_commit_chunk` every ~2 s. When ≥ `CHUNK_SECS`
//! of new audio has accumulated *and* a VAD silence boundary is found near the
//! target split point, that chunk is preprocessed and transcribed immediately.
//!
//! On hotkey release, `finalize` preprocesses + transcribes only the tail
//! (audio since the last committed chunk), then stitches everything together.
//! Because most of the work was done mid-recording, finalize returns quickly.

use std::sync::{Arc, Mutex};

use crate::inference::WhisperEngine;
use crate::preprocess::stitcher::stitch_transcripts;

// Native-rate samples — all thresholds are in raw samples at whatever rate the
// mic reports. We convert to seconds using the captured sample rate.

/// Target chunk size in seconds. We look for a VAD silence boundary near here.
const CHUNK_SECS: f64 = 8.0;
/// Minimum seconds of new audio before we bother checking for a split.
const MIN_NEW_SECS: f64 = 6.0;
/// Overlap in seconds added to the start of each chunk from the end of the
/// previous one — gives the stitcher word-level context to deduplicate.
const OVERLAP_SECS: f64 = 0.4;
/// VAD frame size at 16 kHz (Silero V5 constraint).
const VAD_CHUNK_16K: usize = 512;
/// VAD silence threshold for finding a split boundary (higher = only clear gaps).
const VAD_SPLIT_THRESHOLD: f32 = 0.45;
/// Minimum run of silent frames to accept as a split point (~5 × 32ms = 160ms).
const MIN_SILENCE_FRAMES: usize = 5;

pub struct StreamingPipeline {
    /// How many raw (native-rate) samples we have already committed to inference.
    committed_cursor: usize,
    /// Transcribed text from each committed chunk, in order.
    completed_texts: Vec<String>,
}

impl StreamingPipeline {
    pub const fn new() -> Self {
        Self {
            committed_cursor: 0,
            completed_texts: Vec::new(),
        }
    }

    /// Called periodically while recording. Checks if enough new audio has
    /// accumulated and a VAD silence boundary is nearby. If so, preprocesses
    /// and transcribes the chunk synchronously (this runs on a blocking thread).
    ///
    /// Returns `true` if a chunk was committed, `false` if nothing was done.
    pub fn try_commit_chunk(
        &mut self,
        buffer: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<WhisperEngine>>,
        prompt: &str,
        beam_size: i32,
    ) -> bool {
        if native_rate == 0 {
            return false;
        }

        let new_samples = buffer.len().saturating_sub(self.committed_cursor);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let min_new = (MIN_NEW_SECS * f64::from(native_rate)) as usize;

        if new_samples < min_new {
            return false;
        }

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let chunk_samples_native = (CHUNK_SECS * f64::from(native_rate)) as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let overlap_native = (OVERLAP_SECS * f64::from(native_rate)) as usize;

        // Ideal end of chunk in the raw buffer
        let ideal_end = (self.committed_cursor + chunk_samples_native).min(buffer.len());

        // Preprocess the candidate slice to 16 kHz so we can run VAD on it
        let candidate = &buffer[self.committed_cursor..ideal_end];
        let preprocessed = crate::preprocess::preprocess(candidate, native_rate);

        // Find a VAD silence boundary near the end of the preprocessed audio
        let split_16k = find_vad_split(&preprocessed);

        // Map the 16 kHz split back to native-rate samples
        let ratio = f64::from(native_rate) / 16_000.0;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
        let split_native = (split_16k as f64 * ratio) as usize;

        // The chunk to transcribe is from the start to the split (native rate)
        let chunk_end_native = (self.committed_cursor + split_native).min(buffer.len());

        if chunk_end_native <= self.committed_cursor {
            return false;
        }

        // Take with overlap prepended from previous committed position
        let chunk_start_native = self.committed_cursor.saturating_sub(overlap_native);
        let raw_chunk = &buffer[chunk_start_native..chunk_end_native];
        let has_overlap = chunk_start_native < self.committed_cursor;

        let resampled = crate::preprocess::preprocess(raw_chunk, native_rate);

        if resampled.is_empty() {
            return false;
        }

        let text = if let Ok(guard) = engine.lock() {
            match guard.transcribe(&resampled, prompt, beam_size) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("streaming chunk inference failed: {e}");
                    return false;
                }
            }
        } else {
            log::error!("WhisperEngine mutex poisoned during streaming chunk");
            return false;
        };

        log::debug!(
            "streaming chunk committed: {} chars (overlap={})",
            text.len(),
            has_overlap
        );

        if !text.is_empty() {
            self.completed_texts.push(text);
        }

        self.committed_cursor = chunk_end_native;
        true
    }

    /// Called once after the capture thread stops. Preprocesses + transcribes
    /// the remaining tail audio, stitches all chunks, and returns the final text.
    pub fn finalize(
        mut self,
        buffer: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<WhisperEngine>>,
        prompt: &str,
        beam_size: i32,
    ) -> String {
        if native_rate == 0 || buffer.is_empty() {
            return stitch_transcripts(&self.completed_texts);
        }

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let overlap_native = (OVERLAP_SECS * f64::from(native_rate)) as usize;
        let tail_start = self.committed_cursor.saturating_sub(overlap_native);

        if tail_start < buffer.len() {
            let tail = &buffer[tail_start..];
            let resampled = crate::preprocess::preprocess(tail, native_rate);

            if !resampled.is_empty() {
                let text = engine.lock().map_or_else(|_| {
                    log::error!("WhisperEngine mutex poisoned during finalize");
                    String::new()
                }, |guard| guard.transcribe(&resampled, prompt, beam_size).unwrap_or_default());
                if !text.is_empty() {
                    self.completed_texts.push(text);
                }
            }
        }

        stitch_transcripts(&self.completed_texts)
    }
}

/// Find the best VAD silence boundary near the end of `samples` (16 kHz).
/// Scans the last 4 seconds for the deepest silence run and returns the
/// sample index of its midpoint. Falls back to `samples.len()` if none found.
fn find_vad_split(samples: &[f32]) -> usize {
    use voice_activity_detector::{IteratorExt, VoiceActivityDetector};

    let Ok(mut vad) = VoiceActivityDetector::builder()
        .sample_rate(16_000)
        .chunk_size(VAD_CHUNK_16K)
        .build() else { return samples.len() };

    let predictions: Vec<f32> = samples
        .iter()
        .copied()
        .predict(&mut vad)
        .map(|(_, prob)| prob)
        .collect();

    let n = predictions.len();
    if n == 0 {
        return samples.len();
    }

    // Search only the last 4 s worth of frames — we want a split near the end
    let search_frames = (4 * 16_000 / VAD_CHUNK_16K).min(n);
    let search_start = n.saturating_sub(search_frames);

    let mut best_split: Option<usize> = None;
    let mut best_run = 0usize;

    let mut i = search_start;
    while i < n {
        if predictions[i] < VAD_SPLIT_THRESHOLD {
            let run_start = i;
            while i < n && predictions[i] < VAD_SPLIT_THRESHOLD {
                i += 1;
            }
            let run_len = i - run_start;
            if run_len >= MIN_SILENCE_FRAMES && run_len > best_run {
                best_run = run_len;
                best_split = Some((run_start + run_len / 2) * VAD_CHUNK_16K);
            }
        } else {
            i += 1;
        }
    }

    best_split.unwrap_or(samples.len()).min(samples.len())
}
