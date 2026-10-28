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
//!
//! Based on ÚFAL's `whisper_streaming` (<https://github.com/ufal/whisper_streaming>;
//! Macháček et al., 2023).

use std::sync::{Arc, Mutex};

use crate::inference::transcript::{join_words, Word};
use crate::inference::{Pass, TimedSegment, TranscriptionEngine};
use crate::preprocess::To16k;

const MIN_NEW_AUDIO_SECS: f64 = 1.0;
const MAX_NEW_AUDIO_SECS: f64 = 5.0;
/// Share of real time streaming decodes may occupy.
const DUTY_CYCLE: f64 = 0.5;
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
/// Levelled room noise scores ~0.5, levelled quiet speech ~0.8.
const VAD_LEVELED_SPEECH_ENTER: f32 = 0.7;
const VAD_PAD_FRAMES: usize = 2;

pub struct FinalText {
    pub text: String,
    /// `false` when the end of the recording may be missing.
    pub complete: bool,
}

impl FinalText {
    const fn complete(text: String) -> Self {
        Self {
            text,
            complete: true,
        }
    }

    const fn partial(text: String) -> Self {
        Self {
            text,
            complete: false,
        }
    }
}

/// State of one recording's streaming transcription. Created when recording
/// starts, polled by the stream worker, consumed by [`finalize`].
#[derive(Default)]
pub struct StreamingSession {
    /// Words confirmed by LocalAgreement-2, in order. Never revised.
    committed: Vec<Word>,
    committed_norm: Vec<String>,
    /// Native-rate buffer index where the active window starts.
    window_start: usize,
    /// Native-rate samples already converted.
    fed: usize,
    converter: Option<To16k>,
    window_16k: Vec<f32>,
    /// Native-rate buffer length at the last decode; gates decode cadence.
    decoded_len: usize,
    last_decode_secs: f64,
    prev_norm: Vec<String>,
    /// Leading hypothesis words that restate committed text.
    hyp_committed: usize,
    segments: Vec<TimedSegment>,
    lead_trimmed: bool,
}

impl StreamingSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn fed(&self) -> usize {
        self.fed
    }

    /// Whether [`Self::poll`] would decode a buffer of `total_len` samples.
    pub fn would_decode(&self, total_len: usize, native_rate: u32) -> bool {
        if native_rate == 0 {
            return false;
        }
        let new_samples = total_len.saturating_sub(self.decoded_len.max(self.window_start));
        to_secs(new_samples, native_rate) >= self.decode_gate_secs()
    }

    fn decode_gate_secs(&self) -> f64 {
        (self.last_decode_secs / DUTY_CYCLE).clamp(MIN_NEW_AUDIO_SECS, MAX_NEW_AUDIO_SECS)
    }

    fn ingest(&mut self, new_audio: &[f32], native_rate: u32) -> Result<(), String> {
        let converter = match self.converter.as_mut() {
            Some(converter) => converter,
            None => self.converter.insert(To16k::new(native_rate)?),
        };
        self.window_16k.extend(converter.push(new_audio)?);
        self.fed += new_audio.len();
        Ok(())
    }

    /// A failed conversion leaves the filter state unknown: refeed from the window start.
    fn restart_conversion(&mut self) {
        self.converter = None;
        self.window_16k.clear();
        self.fed = self.window_start;
    }

    fn advance_window(&mut self, new_start: usize, native_rate: u32) {
        let dropped = (new_start - self.window_start) * 16_000 / native_rate as usize;
        self.window_16k.drain(..dropped.min(self.window_16k.len()));
        self.window_start = new_start;
    }

    /// One streaming step over `new_audio`, the native-rate buffer from
    /// [`Self::fed`] on. Decodes synchronously — call from a blocking thread.
    pub fn poll(
        &mut self,
        new_audio: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<TranscriptionEngine>>,
    ) {
        if let Err(e) = self.ingest(new_audio, native_rate) {
            log::warn!("streaming pass skipped: {e}");
            self.restart_conversion();
            return;
        }
        let total_len = self.fed;
        if !self.lead_trimmed {
            // `decoded_len` stays put so the next poll re-checks the grown buffer.
            let Some(skip_16k) = lead_speech_offset(&self.window_16k) else {
                return;
            };
            self.window_start += from_16k(skip_16k, native_rate);
            self.window_16k.drain(..skip_16k);
            self.lead_trimmed = true;
        }
        self.decoded_len = total_len;

        let leveled = crate::preprocess::normalize_level(&self.window_16k);
        if leveled.is_empty() {
            return;
        }

        let prompt = self.prompt();
        let started = std::time::Instant::now();
        let segments = {
            let Ok(mut guard) = engine.lock() else {
                return;
            };
            match guard.transcribe_segments(&leveled, &prompt, Pass::Streaming) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("streaming decode failed: {e}");
                    return;
                }
            }
        };
        self.last_decode_secs = started.elapsed().as_secs_f64();
        log::debug!(
            "streaming decode: {:.1}s window in {}ms, {} segments",
            to_secs(total_len - self.window_start, native_rate),
            started.elapsed().as_millis(),
            segments.len(),
        );

        self.segments = segments;
        self.absorb();

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
            .skip(self.hyp_committed)
            .cloned()
            .collect();
        let tentative = if pending.is_empty() {
            String::new()
        } else {
            format!(" {}", join_words(&pending).trim())
        };
        (self.text(), tentative)
    }

    /// Align away what the hypothesis restates, then commit what two hypotheses
    /// now agree on.
    fn absorb(&mut self) {
        let hyp: Vec<Word> = self
            .segments
            .iter()
            .flat_map(|s| &s.words)
            .cloned()
            .collect();
        let norm: Vec<String> = hyp.iter().map(|w| normalize_word(&w.text)).collect();

        let overlap = trailing_overlap(&self.committed_norm, &norm);
        let fresh_norm = &norm[overlap..];
        let agreed = common_prefix_len(&self.prev_norm, fresh_norm);

        for (word, n) in hyp[overlap..overlap + agreed]
            .iter()
            .zip(&fresh_norm[..agreed])
        {
            self.committed.push(word.clone());
            self.committed_norm.push(n.clone());
        }
        self.prev_norm = fresh_norm[agreed..].to_vec();
        self.hyp_committed = overlap + agreed;
    }

    fn text(&self) -> String {
        join_words(&self.committed)
    }

    #[cfg(test)]
    fn set_committed(&mut self, words: &[&str]) {
        self.committed = words
            .iter()
            .map(|t| Word {
                text: (*t).to_string(),
                end_cs: None,
            })
            .collect();
        self.committed_norm = words.iter().map(|t| normalize_word(t)).collect();
    }

    /// Committed text whose audio has scrolled out of the window. Prompting with
    /// text the window still covers makes the model skip re-transcribing it.
    fn prompt(&self) -> String {
        let outside = self.committed.len() - self.hyp_committed.min(self.committed.len());
        let start = outside.saturating_sub(PROMPT_TAIL_WORDS);
        join_words(&self.committed[start..outside])
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
            if cum_words > self.hyp_committed {
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
        self.advance_window(
            self.window_start + from_ms(end_ms, native_rate),
            native_rate,
        );
        self.segments.drain(..=idx);
        self.hyp_committed -= words;
        true
    }

    /// Fallback when [`Self::trim`] finds no segment boundary: cuts inside the
    /// first segment at a committed word boundary.
    fn force_trim(&mut self, total_len: usize, native_rate: u32) {
        let Some(first) = self.segments.first() else {
            return;
        };
        let take_words = self.hyp_committed.min(first.words.len());
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
        self.advance_window(boundary, native_rate);
        self.segments[0].words.drain(..words);
        if self.segments[0].words.is_empty() {
            self.segments.remove(0);
        }
        self.hyp_committed -= words;
    }

    /// Decode the remaining window and append whatever follows the committed
    /// text. This pass re-covers committed audio, which is aligned away. `Err`
    /// means the audio could not be prepared, so the tail was never decoded.
    fn finish(
        mut self,
        new_audio: &[f32],
        native_rate: u32,
        engine: &Arc<Mutex<TranscriptionEngine>>,
    ) -> Result<FinalText, String> {
        if native_rate == 0 {
            return Ok(FinalText::complete(self.text()));
        }

        self.ingest(new_audio, native_rate)?;
        if let Some(converter) = self.converter.as_mut() {
            self.window_16k.extend(converter.finish()?);
        }
        if !self.lead_trimmed {
            // Decoding silence makes Whisper invent text ("Thanks for watching!").
            let Some(skip_16k) = final_speech_offset(&self.window_16k) else {
                return Ok(FinalText::complete(self.text()));
            };
            self.window_16k.drain(..skip_16k);
        }
        let leveled = crate::preprocess::normalize_level(&self.window_16k);
        if leveled.is_empty() {
            return Ok(FinalText::complete(self.text()));
        }

        let prompt = self.prompt();
        let Ok(mut guard) = engine.lock() else {
            log::error!("TranscriptionEngine mutex poisoned during finalize");
            return Ok(FinalText::partial(self.text()));
        };
        match guard.transcribe_segments(&leveled, &prompt, Pass::Final) {
            Ok(segments) => {
                let truncated = guard.was_truncated();
                let hyp: Vec<Word> = segments.iter().flat_map(|s| &s.words).cloned().collect();
                let norm: Vec<String> = hyp.iter().map(|w| normalize_word(&w.text)).collect();
                let overlap = trailing_overlap(&self.committed_norm, &norm);
                self.committed.extend_from_slice(&hyp[overlap..]);
                Ok(FinalText {
                    text: self.text(),
                    complete: !truncated,
                })
            }
            Err(e) => {
                log::warn!("final decode failed: {e}");
                Ok(FinalText::partial(self.text()))
            }
        }
    }
}

/// Finalize a recording: decode the audio the session hasn't committed yet and
/// return the transcript. `None` (recording never streamed) decodes the whole
/// buffer in one pass. `Err` means the audio could not be prepared.
pub fn finalize(
    session: Option<StreamingSession>,
    buffer: &[f32],
    native_rate: u32,
    engine: &Arc<Mutex<TranscriptionEngine>>,
) -> Result<FinalText, String> {
    let session = session.unwrap_or_default();
    let start = session.fed().min(buffer.len());
    session.finish(&buffer[start..], native_rate, engine)
}

/// How many leading words of `hypothesis` restate the tail of `committed`.
fn trailing_overlap(committed: &[String], hypothesis: &[String]) -> usize {
    let max = committed.len().min(hypothesis.len());
    (1..=max)
        .rev()
        .find(|&k| committed[committed.len() - k..] == hypothesis[..k])
        .unwrap_or(0)
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
    speech_offset(samples_16k, VAD_SPEECH_ENTER)
}

/// Quiet speech can score under the threshold raw; levelling lifts room noise
/// too, so the levelled second look is held to a stricter one.
fn final_speech_offset(samples_16k: &[f32]) -> Option<usize> {
    lead_speech_offset(samples_16k).or_else(|| {
        speech_offset(
            &crate::preprocess::normalize_level(samples_16k),
            VAD_LEVELED_SPEECH_ENTER,
        )
    })
}

fn speech_offset(samples_16k: &[f32], enter: f32) -> Option<usize> {
    let mut vad = earshot::Detector::default_boxed();

    // A trailing partial frame is ignored; at 16 ms it is below our padding.
    let first = samples_16k
        .as_chunks::<VAD_CHUNK_16K>()
        .0
        .iter()
        .position(|frame| vad.predict_f32(frame) >= enter)?;

    let start = first.saturating_sub(VAD_PAD_FRAMES) * VAD_CHUNK_16K;
    (start < samples_16k.len()).then_some(start)
}

#[cfg(test)]
#[path = "../../tests/unit/transcribe/local_agreement.rs"]
mod tests;
