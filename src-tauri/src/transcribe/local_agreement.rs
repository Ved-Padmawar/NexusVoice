//! Growing-window streaming transcription with `LocalAgreement-2` confirmation.
//!
//! The active window is re-decoded whenever [`MIN_NEW_AUDIO_SECS`] of new audio
//! arrives; re-decoding the same audio is what lets a later pass revise an
//! earlier guess.
//!
//! Committing and trimming are separate. Words are committed once two
//! consecutive hypotheses agree on them and are never revised; trimming only
//! drops audio already behind the committed text, so a trim that finds no cut
//! point costs compute, never text.

use std::sync::{Arc, Mutex};

use crate::inference::transcript::{join_words, Word};
use crate::inference::{TimedSegment, TranscriptionEngine};

/// A slow decode just means the next absorbs more audio — self-adaptive latency.
const MIN_NEW_AUDIO_SECS: f64 = 1.0;
/// Well under the 15 s reference default: keeps mid-recording decodes cheap on Vulkan.
const TRIM_AFTER_SECS: f64 = 8.0;
/// Window length past which a segment-boundary trim gives way to a word-level cut.
const FORCE_TRIM_SECS: f64 = 12.0;
/// Audio a trim must leave behind — decoders need ≥1 s.
const MIN_WINDOW_SECS: f64 = 2.0;
const PROMPT_TAIL_WORDS: usize = 30;

/// earshot requires exactly 256 samples (16 ms) per frame.
const VAD_CHUNK_16K: usize = 256;
const VAD_SPEECH_ENTER: f32 = 0.5;
const VAD_PAD_FRAMES: usize = 2;

/// State of one recording's streaming transcription. Created when recording
/// starts, polled by the stream worker, consumed by [`finalize`].
#[derive(Debug, Default)]
pub struct StreamingSession {
    /// Text confirmed by LocalAgreement-2. Never revised.
    committed: String,
    /// Native-rate buffer index where the active window starts.
    window_start: usize,
    /// Native-rate buffer length at the last decode; gates decode cadence.
    decoded_len: usize,
    /// Normalized words of the previous hypothesis, past `committed_words`.
    prev_norm: Vec<String>,
    /// Leading words of `segments` already committed. They stay in `segments`
    /// so a trim can still find their timestamps.
    committed_words: usize,
    segments: Vec<TimedSegment>,
    lead_trimmed: bool,
}

impl StreamingSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Native-rate buffer index the stream worker snapshots the window from.
    pub const fn window_start(&self) -> usize {
        self.window_start
    }

    /// Whether a `poll` over a buffer of `total_len` samples would decode. Lets
    /// the worker skip copying the window on polls that would bail anyway.
    pub fn would_decode(&self, total_len: usize, native_rate: u32) -> bool {
        if native_rate == 0 {
            return false;
        }
        let new_samples = total_len.saturating_sub(self.decoded_len.max(self.window_start));
        to_secs(new_samples, native_rate) >= MIN_NEW_AUDIO_SECS
    }

    /// One streaming step: decode if enough new audio arrived, commit whatever
    /// two consecutive hypotheses now agree on, then trim the window if it
    /// outgrew the threshold.
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

        let mut prepared = crate::preprocess::to_16k(window, native_rate);
        if !self.lead_trimmed {
            // `decoded_len` stays put so the next poll re-checks the grown buffer.
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

        self.segments = segments;
        self.commit_agreed();

        let window_secs = to_secs(total_len - self.window_start, native_rate);
        if window_secs > TRIM_AFTER_SECS
            && !self.trim(total_len, native_rate)
            && window_secs > FORCE_TRIM_SECS
        {
            self.force_trim(total_len, native_rate);
        }
    }

    /// Transcript so far, split into LocalAgreement-confirmed text and the
    /// hypothesis tail still open to revision.
    pub fn partial(&self) -> (String, String) {
        let pending: Vec<Word> = self
            .segments
            .iter()
            .flat_map(|s| &s.words)
            .skip(self.committed_words)
            .cloned()
            .collect();
        let tentative = if pending.is_empty() {
            String::new()
        } else {
            format!(" {}", join_words(&pending).trim())
        };
        (self.committed.clone(), tentative)
    }

    /// Commit the prefix this hypothesis shares with the previous one. Both are
    /// compared past `committed_words`, so the committed prefix only ever grows.
    fn commit_agreed(&mut self) {
        let fresh: Vec<Word> = self
            .segments
            .iter()
            .flat_map(|s| &s.words)
            .skip(self.committed_words)
            .cloned()
            .collect();
        let norm: Vec<String> = fresh.iter().map(|w| normalize_word(&w.text)).collect();

        let agreed = common_prefix_len(&self.prev_norm, &norm);
        if agreed > 0 {
            push_text(&mut self.committed, &join_words(&fresh[..agreed]));
            self.committed_words += agreed;
        }
        self.prev_norm = norm[agreed..].to_vec();
    }

    /// Drop the audio behind the last completed segment boundary inside the
    /// committed prefix. Purely a buffer cut — the text is already committed.
    /// The last segment never qualifies: it is still growing.
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
            if cum_words > self.committed_words {
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
        self.window_start += from_ms(end_ms, native_rate);
        self.segments.drain(..=idx);
        self.committed_words -= words;
        true
    }

    /// Fallback when [`Self::trim`] finds no segment boundary: cuts inside the
    /// first segment at a committed word boundary using DTW end timestamps.
    fn force_trim(&mut self, total_len: usize, native_rate: u32) {
        let Some(first) = self.segments.first() else {
            return;
        };
        let take_words = self.committed_words.min(first.words.len());
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
        self.window_start = boundary;
        self.segments[0].words.drain(..words);
        if self.segments[0].words.is_empty() {
            self.segments.remove(0);
        }
        self.committed_words -= words;
    }

    /// Decode the remaining window and append whatever follows the committed
    /// prefix — this pass re-covers committed audio too, so its head is dropped.
    fn finish(
        mut self,
        window: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<TranscriptionEngine>>,
    ) -> String {
        if native_rate == 0 || window.is_empty() {
            return self.committed;
        }

        let mut prepared = crate::preprocess::to_16k(window, native_rate);
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
                let tail: Vec<Word> = segments
                    .iter()
                    .flat_map(|s| &s.words)
                    .skip(self.committed_words)
                    .cloned()
                    .collect();
                push_text(&mut self.committed, &join_words(&tail));
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
/// the whole buffer in one pass.
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

/// The last [`PROMPT_TAIL_WORDS`] words of the committed text, fed back as the
/// next decode's prompt.
fn prompt_tail(committed: &str) -> String {
    let words: Vec<&str> = committed.split_whitespace().collect();
    let start = words.len().saturating_sub(PROMPT_TAIL_WORDS);
    words[start..].join(" ")
}

/// Models flip "Okay," ↔ "okay" between decodes; agreement shouldn't reset over that.
fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

fn common_prefix_len(a: &[String], b: &[String]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

#[allow(clippy::cast_precision_loss)] // audio-sized buffers fit f64 exactly enough
fn to_secs(samples: usize, native_rate: u32) -> f64 {
    samples as f64 / f64::from(native_rate)
}

fn from_16k(samples_16k: usize, native_rate: u32) -> usize {
    samples_16k * native_rate as usize / 16_000
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // clamped non-negative; ms fits usize
fn from_ms(ms: i64, native_rate: u32) -> usize {
    (ms.max(0) as usize) * native_rate as usize / 1000
}

/// First sample of confident speech in a 16 kHz buffer, padded back by
/// [`VAD_PAD_FRAMES`].
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
