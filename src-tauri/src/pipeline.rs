//! UFAL-style streaming transcription (`whisper_streaming`; Macháček et al. 2023,
//! "Turning Whisper into Real-Time Transcription System").
//!
//! The *active window* — audio after the last trim point — is re-decoded whenever
//! [`MIN_NEW_AUDIO_SECS`] of new audio arrives, so every hypothesis keeps full
//! sentence context (the earlier sliding-window attempt lost exactly this and
//! scored ~4x the WER of a single decode). LocalAgreement-2 confirms the prefix
//! two consecutive hypotheses agree on; once the window outgrows
//! [`TRIM_AFTER_SECS`] it is trimmed at a confirmed whisper segment boundary and
//! that text is committed, never to be revised. [`finalize`] decodes only the
//! remaining window, so end-of-recording latency is bounded by the trim
//! threshold, not the recording length. A session that never streamed degrades
//! to a single-pass decode of the whole recording.

use std::sync::{Arc, Mutex};

use crate::inference::transcript::join_segments;
use crate::inference::{TimedSegment, WhisperEngine};

/// Minimum new audio before the window is re-decoded (UFAL's `MinChunkSize`).
/// A slow decode just means the next one absorbs more audio — self-adaptive latency.
const MIN_NEW_AUDIO_SECS: f64 = 1.0;
/// Window length that triggers a trim. UFAL defaults to 15 s; shorter keeps
/// mid-recording decodes cheap on Vulkan while still giving ample context.
const TRIM_AFTER_SECS: f64 = 8.0;
/// Audio a trim must leave in the window (whisper needs ≥1 s to decode).
const MIN_WINDOW_SECS: f64 = 2.0;
/// Beam size for mid-recording decodes; the final decode uses the user's preset.
const STREAM_BEAM: i32 = 2;
/// Max committed words fed back as whisper's `initial_prompt`.
const PROMPT_TAIL_WORDS: usize = 30;

/// VAD frame size at 16 kHz (Silero V5 constraint).
const VAD_CHUNK_16K: usize = 512;
/// Prob ≥ this starts a speech region (Silero default).
const VAD_SPEECH_ENTER: f32 = 0.5;
/// Frames kept before the first speech frame (Silero `speech_pad_ms` ≈ 30ms).
const VAD_PAD_FRAMES: usize = 1;

/// State of one recording's streaming transcription. Created when recording
/// starts, polled by the stream worker, consumed by [`finalize`].
#[derive(Debug, Default)]
pub struct StreamingSession {
    /// Text committed by window trims. Never revised — the UFAL invariant.
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

    /// One streaming step: decode if enough new audio arrived, fold the
    /// hypothesis into the agreement state, trim if the window outgrew the
    /// threshold. Returns the updated preview text when a decode ran.
    ///
    /// `window` is the buffer from [`Self::window_start`] onward, at the mic's
    /// native rate. Runs whisper synchronously — call from a blocking thread.
    pub fn poll(
        &mut self,
        window: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<WhisperEngine>>,
    ) -> Option<String> {
        if native_rate == 0 {
            return None;
        }
        let total_len = self.window_start + window.len();
        let new_samples = total_len.saturating_sub(self.decoded_len.max(self.window_start));
        if to_secs(new_samples, native_rate) < MIN_NEW_AUDIO_SECS {
            return None;
        }
        self.decoded_len = total_len;

        let mut prepared = crate::preprocess::to_16k_denoised(window, native_rate);
        if !self.lead_trimmed {
            // Skip the silent lead-in once speech appears; until then, don't
            // waste decodes hallucinating on silence.
            let skip_16k = lead_speech_offset(&prepared)?;
            self.window_start += from_16k(skip_16k, native_rate);
            prepared.drain(..skip_16k);
            self.lead_trimmed = true;
        }

        let leveled = crate::preprocess::normalize_level(&prepared);
        if leveled.is_empty() {
            return None;
        }

        let prompt = prompt_tail(&self.committed);
        let started = std::time::Instant::now();
        let segments = {
            let guard = engine.lock().ok()?;
            match guard.transcribe_segments(&leveled, &prompt, STREAM_BEAM) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("streaming decode failed: {e}");
                    return None;
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

        if to_secs(total_len - self.window_start, native_rate) > TRIM_AFTER_SECS {
            self.trim(total_len, native_rate);
        }

        Some(self.preview())
    }

    /// Trim the window at the last completed segment boundary inside the
    /// confirmed prefix, committing that text. The last segment never
    /// qualifies — it is still growing.
    fn trim(&mut self, total_len: usize, native_rate: u32) {
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
                break;
            }
            let boundary = self.window_start + from_ms(seg.end_ms, native_rate);
            if to_secs(total_len.saturating_sub(boundary), native_rate) >= MIN_WINDOW_SECS {
                cut = Some((i, cum_words, seg.end_ms));
            }
        }

        let Some((idx, words, end_ms)) = cut else {
            return;
        };
        push_text(&mut self.committed, &join_segments(&self.segments[..=idx]));
        self.window_start += from_ms(end_ms, native_rate);
        self.segments.drain(..=idx);
        self.prev_norm.drain(..words.min(self.prev_norm.len()));
        self.confirmed_words -= words;
    }

    /// Committed text plus the current window hypothesis — what the frontend
    /// shows while recording.
    fn preview(&self) -> String {
        let mut out = self.committed.clone();
        push_text(&mut out, &join_segments(&self.segments));
        out
    }

    /// Decode the remaining window at the user's beam size and return the full
    /// transcript.
    fn finish(
        mut self,
        window: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<WhisperEngine>>,
        beam_size: i32,
    ) -> String {
        if native_rate == 0 || window.is_empty() {
            return self.committed;
        }

        let mut prepared = crate::preprocess::to_16k_denoised(window, native_rate);
        if !self.lead_trimmed {
            // No confident speech at all still gets decoded — whisper's own
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
        let Ok(guard) = engine.lock() else {
            log::error!("WhisperEngine mutex poisoned during finalize");
            return self.committed;
        };
        match guard.transcribe_segments(&leveled, &prompt, beam_size) {
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
    engine: &Arc<Mutex<WhisperEngine>>,
    beam_size: i32,
) -> String {
    let session = session.unwrap_or_default();
    let start = session.window_start().min(buffer.len());
    session.finish(&buffer[start..], native_rate, engine, beam_size)
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

/// The last [`PROMPT_TAIL_WORDS`] words of the committed text, for whisper's
/// `initial_prompt`.
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

/// Case- and punctuation-insensitive form of a word. Whisper often flips
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
/// Only the lead-in is ever trimmed: Silero is a streaming model whose
/// confidence drifts over a long buffer, so using it to mark the *end* of
/// speech silently truncated real dictation. Interior pauses are left alone.
fn lead_speech_offset(samples_16k: &[f32]) -> Option<usize> {
    use voice_activity_detector::{IteratorExt, VoiceActivityDetector};

    let mut vad = VoiceActivityDetector::builder()
        .sample_rate(16_000)
        .chunk_size(VAD_CHUNK_16K)
        .build()
        .ok()?;

    let first = samples_16k
        .iter()
        .copied()
        .predict(&mut vad)
        .position(|(_, prob)| prob >= VAD_SPEECH_ENTER)?;

    let start = first.saturating_sub(VAD_PAD_FRAMES) * VAD_CHUNK_16K;
    (start < samples_16k.len()).then_some(start)
}

#[cfg(test)]
#[path = "../tests/unit/pipeline.rs"]
mod tests;
