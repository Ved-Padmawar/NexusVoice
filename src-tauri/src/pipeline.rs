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
/// ~1.5 s spans several words, so the stitcher can still find a common run
/// when Whisper transcribes the boundary word differently in the two chunks
/// (0.4 s was ~one word and caused boundary duplications/drops).
const OVERLAP_SECS: f64 = 1.5;
/// Max words from already-transcribed chunks fed back as Whisper's
/// `initial_prompt` for the next chunk — gives the decoder real continuity
/// (casing, sentence state, vocabulary) within the current session.
const PROMPT_TAIL_WORDS: usize = 30;
/// VAD frame size at 16 kHz (Silero V5 constraint).
const VAD_CHUNK_16K: usize = 512;
/// Enter-speech threshold: prob ≥ this starts a speech region (Silero default 0.5).
const VAD_SPEECH_ENTER: f32 = 0.5;
/// Exit-speech threshold: once in speech, prob must drop below this to return to
/// silence (Silero's `neg_threshold` = enter − 0.15). The gap is hysteresis — a
/// single frame flickering between the two thresholds no longer fragments a run.
const VAD_SPEECH_EXIT: f32 = 0.35;
/// Minimum silent frames to treat a gap as a real pause (~100ms, Silero default).
/// One 512-sample frame at 16 kHz is 32ms, so 3 frames ≈ 96ms.
const MIN_SILENCE_FRAMES: usize = 3;
/// Frames of padding kept between a split point and the adjacent speech, so the
/// cut doesn't clip a word's tail/onset (Silero `speech_pad_ms` ≈ 30ms ≈ 1 frame).
const VAD_PAD_FRAMES: usize = 1;

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

    /// Whisper `initial_prompt` for the next chunk: the tail of what this
    /// session has already transcribed. Empty for the first chunk — priming
    /// with anything else (old transcripts, dictionary terms) biases the
    /// decoder and invites hallucination loops.
    fn context_prompt(&self) -> String {
        let words: Vec<&str> = self
            .completed_texts
            .iter()
            .flat_map(|t| t.split_whitespace())
            .collect();
        let start = words.len().saturating_sub(PROMPT_TAIL_WORDS);
        words[start..].join(" ")
    }

    /// First buffer index the poller needs to snapshot: the committed cursor
    /// minus the stitch overlap. Lets the poller copy only the uncommitted
    /// tail instead of cloning the whole recording.
    pub fn snapshot_start(&self, native_rate: u32) -> usize {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let overlap_native = (OVERLAP_SECS * f64::from(native_rate)) as usize;
        self.committed_cursor.saturating_sub(overlap_native)
    }

    /// Called periodically while recording. Checks if enough new audio has
    /// accumulated and a VAD silence boundary is nearby. If so, preprocesses
    /// and transcribes the chunk synchronously (this runs on a blocking thread).
    ///
    /// `tail` is the buffer slice starting at absolute index `tail_start`
    /// (from `snapshot_start`) — the poller no longer copies the full buffer.
    ///
    /// Returns `true` if a chunk was committed, `false` if nothing was done.
    pub fn try_commit_chunk(
        &mut self,
        tail: &[f32],
        tail_start: usize,
        native_rate: u32,
        engine: &Arc<Mutex<WhisperEngine>>,
        beam_size: i32,
    ) -> bool {
        if native_rate == 0 || tail_start > self.committed_cursor {
            return false;
        }

        let buffer_len = tail_start + tail.len();
        let new_samples = buffer_len.saturating_sub(self.committed_cursor);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let min_new = (MIN_NEW_SECS * f64::from(native_rate)) as usize;

        if new_samples < min_new {
            return false;
        }

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let chunk_samples_native = (CHUNK_SECS * f64::from(native_rate)) as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let overlap_native = (OVERLAP_SECS * f64::from(native_rate)) as usize;

        // Cursor and ideal chunk end relative to the tail slice
        let rel_cursor = self.committed_cursor - tail_start;
        let ideal_end_rel = (rel_cursor + chunk_samples_native).min(tail.len());
        let chunk_start_rel = rel_cursor.saturating_sub(overlap_native);

        // Resample + denoise once to 16 kHz, *without* VAD splicing — indices
        // in this buffer map linearly back to native-rate time, so the split
        // found here translates exactly to a cursor position.
        let candidate = &tail[chunk_start_rel..ideal_end_rel];
        let denoised_16k = crate::preprocess::to_16k_denoised(candidate, native_rate);

        if denoised_16k.is_empty() {
            return false;
        }

        // Find a VAD silence boundary near the end of the denoised audio
        let split_16k = find_vad_split(&denoised_16k);

        // Map the 16 kHz split back to native-rate samples (relative to the
        // candidate start). Exact because the audio above is not spliced.
        let ratio = f64::from(native_rate) / 16_000.0;
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let split_native = (split_16k as f64 * ratio) as usize;

        let chunk_end_abs = (tail_start + chunk_start_rel + split_native).min(buffer_len);

        if chunk_end_abs <= self.committed_cursor {
            return false;
        }

        // Whisper input: speech-splice + normalize only the audio up to the split.
        let inference_input = crate::preprocess::splice_normalize(
            &denoised_16k[..split_16k.min(denoised_16k.len())],
        );
        if inference_input.is_empty() {
            // Nothing speech-like before the split (e.g. pure silence) — still
            // advance the cursor so silence doesn't get re-scanned every tick.
            self.committed_cursor = chunk_end_abs;
            return false;
        }

        let prompt = self.context_prompt();
        let text = if let Ok(guard) = engine.lock() {
            match guard.transcribe(&inference_input, &prompt, beam_size) {
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

        log::debug!("streaming chunk committed: {} chars", text.len());

        if !text.is_empty() {
            self.completed_texts.push(text);
        }

        self.committed_cursor = chunk_end_abs;
        true
    }

    /// Called once after the capture thread stops. Preprocesses + transcribes
    /// the remaining tail audio, stitches all chunks, and returns the final text.
    pub fn finalize(
        mut self,
        buffer: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<WhisperEngine>>,
        beam_size: i32,
    ) -> String {
        if native_rate == 0 || buffer.is_empty() {
            return stitch_transcripts(&self.completed_texts);
        }

        let tail_start = self.snapshot_start(native_rate);

        if tail_start < buffer.len() {
            let tail = &buffer[tail_start..];
            let resampled = crate::preprocess::preprocess(tail, native_rate);

            if !resampled.is_empty() {
                let prompt = self.context_prompt();
                let text = engine.lock().map_or_else(
                    |_| {
                        log::error!("WhisperEngine mutex poisoned during finalize");
                        String::new()
                    },
                    |guard| {
                        guard
                            .transcribe(&resampled, &prompt, beam_size)
                            .unwrap_or_default()
                    },
                );
                if !text.is_empty() {
                    self.completed_texts.push(text);
                }
            }
        }

        stitch_transcripts(&self.completed_texts)
    }
}

/// Find the best VAD silence boundary near the end of `samples` (16 kHz).
///
/// Runs a two-threshold hysteresis state machine over the Silero probabilities
/// (enter speech at `VAD_SPEECH_ENTER`, exit only below `VAD_SPEECH_EXIT`) so a
/// single flickering frame can't fragment a silence run. Among the silence
/// regions in the last 4 s, picks the longest one ≥ `MIN_SILENCE_FRAMES` and
/// splits at its centre, padded by `VAD_PAD_FRAMES` away from the surrounding
/// speech so the cut doesn't clip a word. Falls back to `samples.len()`.
fn find_vad_split(samples: &[f32]) -> usize {
    use voice_activity_detector::{IteratorExt, VoiceActivityDetector};

    let Ok(mut vad) = VoiceActivityDetector::builder()
        .sample_rate(16_000)
        .chunk_size(VAD_CHUNK_16K)
        .build()
    else {
        return samples.len();
    };

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

    // Search only the last 4 s worth of frames — we want a split near the end.
    let search_frames = (4 * 16_000 / VAD_CHUNK_16K).min(n);
    let search_start = n.saturating_sub(search_frames);

    // Hysteresis state machine: collect silence regions as [start, end) frame
    // ranges. `in_speech` only flips to silence below the exit threshold and
    // back to speech above the enter threshold — the band between them is dead
    // zone, which is what stops a borderline frame from splitting a pause.
    let mut in_speech = true; // assume we begin mid-utterance
    let mut silence_start: Option<usize> = None;
    let mut best: Option<(usize, usize)> = None; // (run_len, mid_frame)

    // Record a silence region [start, end) if it's long enough and the longest seen.
    let mut consider = |start: usize, end: usize| {
        let run_len = end - start;
        if run_len >= MIN_SILENCE_FRAMES && best.is_none_or(|(b, _)| run_len > b) {
            best = Some((run_len, start + run_len / 2));
        }
    };

    for (i, &p) in predictions.iter().enumerate().skip(search_start) {
        if in_speech {
            if p < VAD_SPEECH_EXIT {
                in_speech = false;
                silence_start = Some(i);
            }
        } else if p >= VAD_SPEECH_ENTER {
            in_speech = true;
            if let Some(start) = silence_start.take() {
                consider(start, i);
            }
        }
    }
    // Trailing silence that runs to the end of the buffer.
    if let Some(start) = silence_start {
        consider(start, n);
    }

    let Some((run_len, mid_frame)) = best else {
        return samples.len();
    };

    // Split at the silence centre, then nudge back toward the preceding speech
    // by the padding so the *next* utterance keeps its onset (Silero pads speech
    // on both sides; here we bias the cut to preserve the leading word). Clamped
    // to stay within the silence region, never into the prior speech.
    let pad = VAD_PAD_FRAMES.min(run_len / 2);
    let split_frame = mid_frame.saturating_sub(pad);

    (split_frame * VAD_CHUNK_16K).min(samples.len())
}
