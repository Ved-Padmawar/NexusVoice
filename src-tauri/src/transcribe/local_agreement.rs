//! Growing-window streaming transcription with `LocalAgreement-2` confirmation.
//!
//! The *active window* — audio after the last trim point — is re-decoded whenever
//! [`MIN_NEW_AUDIO_SECS`] of new audio arrives, so every hypothesis keeps full
//! sentence context. LocalAgreement-2 confirms the prefix
//! two consecutive hypotheses agree on; once the window outgrows
//! [`TRIM_AFTER_SECS`] it is trimmed at a confirmed segment boundary and
//! that text is committed, never to be revised. [`finalize`] decodes only the
//! remaining window, so end-of-recording latency is bounded by the trim
//! threshold, not the recording length. A session that never streamed degrades
//! to a single-pass decode of the whole recording.

use std::sync::{Arc, Mutex};

use crate::inference::transcript::join_segments;
use crate::inference::{TimedSegment, TranscriptionEngine};

/// Minimum new audio before the window is re-decoded (the minimum chunk size).
/// A slow decode just means the next one absorbs more audio — self-adaptive latency.
const MIN_NEW_AUDIO_SECS: f64 = 1.0;
/// Window length that triggers a trim. Kept well under the 15 s reference
/// default: shorter keeps mid-recording decodes cheap on Vulkan while still
/// giving ample context.
const TRIM_AFTER_SECS: f64 = 8.0;
/// Window length past which `trim()` failing to find a segment boundary
/// (continuous speech, one still-growing segment) triggers a word-level
/// cut instead — see `force_trim`.
const FORCE_TRIM_SECS: f64 = 12.0;
/// Audio a trim must leave in the window (decoders need ≥1 s of audio).
const MIN_WINDOW_SECS: f64 = 2.0;
/// Max committed words fed back as the decode prompt.
const PROMPT_TAIL_WORDS: usize = 30;

/// VAD frame size at 16 kHz — earshot requires exactly 256 samples (16 ms).
const VAD_CHUNK_16K: usize = 256;
/// Score ≥ this starts a speech region.
const VAD_SPEECH_ENTER: f32 = 0.5;
/// Frames kept before the first speech frame, ≈32 ms of lead-in.
const VAD_PAD_FRAMES: usize = 2;

/// State of one recording's streaming transcription. Created when recording
/// starts, polled by the stream worker, consumed by [`finalize`].
#[derive(Debug, Default)]
pub struct StreamingSession {
    /// Text committed by window trims. Never revised — the core invariant.
    committed: String,
    /// Native-rate buffer index where the active window starts.
    window_start: usize,
    /// Native-rate buffer length at the last decode; gates decode cadence.
    decoded_len: usize,
    /// Normalized words of the previous hypothesis (`LocalAgreement` input).
    prev_norm: Vec<String>,
    /// Leading words of the latest hypothesis confirmed by two consecutive
    /// hypotheses agreeing. Gates where the window may be trimmed.
    confirmed_words: usize,
    /// Latest hypothesis over the active window.
    segments: Vec<TimedSegment>,
    /// Whether leading silence has been located and skipped (done once).
    lead_trimmed: bool,
}

impl StreamingSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the active window starts, as a native-rate buffer index. The
    /// stream worker snapshots the buffer from here.
    pub const fn window_start(&self) -> usize {
        self.window_start
    }

    /// Whether a `poll` over a buffer of `total_len` samples would decode.
    /// Lets the stream worker skip copying the window on polls that would bail
    /// on the same gate anyway.
    pub fn would_decode(&self, total_len: usize, native_rate: u32) -> bool {
        if native_rate == 0 {
            return false;
        }
        let new_samples = total_len.saturating_sub(self.decoded_len.max(self.window_start));
        to_secs(new_samples, native_rate) >= MIN_NEW_AUDIO_SECS
    }

    /// One streaming step: decode if enough new audio arrived, fold the
    /// hypothesis into the agreement state, trim if the window outgrew the
    /// threshold. Work accumulates in the session; finalize reads it out.
    ///
    /// `window` is the buffer from [`Self::window_start`] onward, at the mic's
    /// native rate. Decodes synchronously — call from a blocking thread.
    pub fn poll(
        &mut self,
        window: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<TranscriptionEngine>>,
    ) {
        if native_rate == 0 {
            return;
        }
        let total_len = self.window_start + window.len();
        let new_samples = total_len.saturating_sub(self.decoded_len.max(self.window_start));
        if to_secs(new_samples, native_rate) < MIN_NEW_AUDIO_SECS {
            return;
        }

        let mut prepared = crate::preprocess::to_16k_denoised(window, native_rate);
        if !self.lead_trimmed {
            // Skip the silent lead-in once speech appears. `decoded_len` stays
            // put so the next poll re-checks the grown buffer.
            let Some(skip_16k) = lead_speech_offset(&prepared) else {
                return;
            };
            self.window_start += from_16k(skip_16k, native_rate);
            prepared.drain(..skip_16k);
            self.lead_trimmed = true;
        }
        self.decoded_len = total_len;

        let leveled = crate::preprocess::normalize_level(&prepared);
        if leveled.is_empty() {
            return;
        }

        let prompt = prompt_tail(&self.committed);
        let started = std::time::Instant::now();
        let segments = {
            let Ok(mut guard) = engine.lock() else {
                return;
            };
            match guard.transcribe_segments(&leveled, &prompt) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("streaming decode failed: {e}");
                    return;
                }
            }
        };
        log::debug!(
            "streaming decode: {:.1}s window in {}ms, {} segments",
            to_secs(total_len - self.window_start, native_rate),
            started.elapsed().as_millis(),
            segments.len(),
        );

        // LocalAgreement-2: confirm the prefix two consecutive hypotheses share.
        let new_norm = normalized_words(&segments);
        self.confirmed_words = common_prefix_len(&self.prev_norm, &new_norm);
        self.prev_norm = new_norm;
        self.segments = segments;

        let window_secs = to_secs(total_len - self.window_start, native_rate);
        if window_secs > TRIM_AFTER_SECS {
            let trimmed = self.trim(total_len, native_rate);
            if !trimmed && window_secs > FORCE_TRIM_SECS {
                self.force_trim(total_len, native_rate);
            }
        }
    }

    /// Trim the window at the last completed segment boundary inside the
    /// confirmed prefix, committing that text. The last segment never
    /// qualifies — it is still growing. Returns whether a trim happened.
    fn trim(&mut self, total_len: usize, native_rate: u32) -> bool {
        let mut cum_words = 0;
        let mut cut: Option<(usize, usize, i64)> = None; // (segment idx, words, end_ms)
        for (i, seg) in self
            .segments
            .iter()
            .enumerate()
            .take(self.segments.len().saturating_sub(1))
        {
            cum_words += seg.words.len();
            if cum_words > self.confirmed_words {
                continue;
            }
            let boundary = self.window_start + from_ms(seg.end_ms, native_rate);
            if to_secs(total_len.saturating_sub(boundary), native_rate) >= MIN_WINDOW_SECS {
                cut = Some((i, cum_words, seg.end_ms));
            }
        }

        let Some((idx, words, end_ms)) = cut else {
            return false;
        };
        push_text(&mut self.committed, &join_segments(&self.segments[..=idx]));
        self.window_start += from_ms(end_ms, native_rate);
        self.segments.drain(..=idx);
        self.prev_norm.drain(..words.min(self.prev_norm.len()));
        self.confirmed_words -= words;
        true
    }

    /// Fallback when `trim()` finds no segment boundary: cuts inside the
    /// first (still-growing) segment at a word boundary using DTW end
    /// timestamps. No-op without timestamps or without `MIN_WINDOW_SECS` left.
    fn force_trim(&mut self, total_len: usize, native_rate: u32) {
        let Some(first) = self.segments.first() else {
            return;
        };
        let take_words = self.confirmed_words.min(first.words.len());
        let Some(cut_word_idx) = (0..take_words)
            .rev()
            .find(|&i| first.words[i].end_cs.is_some())
        else {
            return;
        };
        let end_cs = first.words[cut_word_idx].end_cs.expect("checked above");
        let boundary = self.window_start + from_ms(end_cs * 10, native_rate);
        if to_secs(total_len.saturating_sub(boundary), native_rate) < MIN_WINDOW_SECS {
            return;
        }

        let words = cut_word_idx + 1;
        push_text(
            &mut self.committed,
            &crate::inference::transcript::join_words(&first.words[..words]),
        );
        self.window_start = boundary;
        self.segments[0].words.drain(..words);
        if self.segments[0].words.is_empty() {
            self.segments.remove(0);
        }
        self.prev_norm.drain(..words.min(self.prev_norm.len()));
        self.confirmed_words = self.confirmed_words.saturating_sub(words);
    }

    /// Decode the remaining window and return the full transcript.
    fn finish(
        mut self,
        window: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<TranscriptionEngine>>,
    ) -> String {
        if native_rate == 0 || window.is_empty() {
            return self.committed;
        }

        let mut prepared = crate::preprocess::to_16k_denoised(window, native_rate);
        if !self.lead_trimmed {
            // No confident speech at all still gets decoded — the model's own
            // no-speech filters have the final say.
            if let Some(skip_16k) = lead_speech_offset(&prepared) {
                prepared.drain(..skip_16k);
            }
        }
        let leveled = crate::preprocess::normalize_level(&prepared);
        if leveled.is_empty() {
            return self.committed;
        }

        let prompt = prompt_tail(&self.committed);
        let Ok(mut guard) = engine.lock() else {
            log::error!("TranscriptionEngine mutex poisoned during finalize");
            return self.committed;
        };
        match guard.transcribe_segments(&leveled, &prompt) {
            Ok(segments) => {
                push_text(&mut self.committed, &join_segments(&segments));
                self.committed
            }
            Err(e) => {
                log::warn!("final decode failed: {e}");
                self.committed
            }
        }
    }
}

/// Finalize a recording: decode the audio the session hasn't committed yet and
/// return the complete transcript. `None` (recording never streamed) decodes
/// the whole buffer in one pass, exactly like the previous pipeline.
pub fn finalize(
    session: Option<StreamingSession>,
    buffer: &[f32],
    native_rate: u32,
    engine: &Arc<Mutex<TranscriptionEngine>>,
) -> String {
    let session = session.unwrap_or_default();
    let start = session.window_start().min(buffer.len());
    session.finish(&buffer[start..], native_rate, engine)
}

/// Append `text` to `out` with a separating space.
fn push_text(out: &mut String, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(text);
}

/// The last [`PROMPT_TAIL_WORDS`] words of the committed text, used as the
/// next decode's prompt.
fn prompt_tail(committed: &str) -> String {
    let words: Vec<&str> = committed.split_whitespace().collect();
    let start = words.len().saturating_sub(PROMPT_TAIL_WORDS);
    words[start..].join(" ")
}

/// Flatten a hypothesis into normalized words for agreement comparison.
fn normalized_words(segments: &[TimedSegment]) -> Vec<String> {
    segments
        .iter()
        .flat_map(|s| &s.words)
        .map(|w| normalize_word(&w.text))
        .collect()
}

/// Case- and punctuation-insensitive form of a word. Models often flip
/// "Okay," ↔ "okay" between decodes; agreement shouldn't reset over that.
fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Length of the longest common prefix of two word sequences.
fn common_prefix_len(a: &[String], b: &[String]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// Seconds represented by `samples` at `native_rate`.
#[allow(clippy::cast_precision_loss)] // audio-sized buffers fit f64 exactly enough
fn to_secs(samples: usize, native_rate: u32) -> f64 {
    samples as f64 / f64::from(native_rate)
}

/// Native-rate samples equivalent to `samples_16k` at 16 kHz.
fn from_16k(samples_16k: usize, native_rate: u32) -> usize {
    samples_16k * native_rate as usize / 16_000
}

/// Native-rate samples equivalent to `ms` milliseconds.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // clamped non-negative; ms fits usize
fn from_ms(ms: i64, native_rate: u32) -> usize {
    (ms.max(0) as usize) * native_rate as usize / 1000
}

/// First sample of confident speech in a 16 kHz buffer, padded back by
/// [`VAD_PAD_FRAMES`]. `None` when no speech has been detected yet.
///
/// Only the lead-in is ever trimmed — marking the *end* of speech this way
/// silently truncates real dictation. Interior pauses are left alone.
fn lead_speech_offset(samples_16k: &[f32]) -> Option<usize> {
    let mut vad = earshot::Detector::default_boxed();

    // A trailing partial frame is ignored; at 16 ms it is below our padding.
    let first = samples_16k
        .as_chunks::<VAD_CHUNK_16K>()
        .0
        .iter()
        .position(|frame| vad.predict_f32(frame) >= VAD_SPEECH_ENTER)?;

    let start = first.saturating_sub(VAD_PAD_FRAMES) * VAD_CHUNK_16K;
    (start < samples_16k.len()).then_some(start)
}

#[cfg(test)]
#[path = "../../tests/unit/transcribe/local_agreement.rs"]
mod tests;
