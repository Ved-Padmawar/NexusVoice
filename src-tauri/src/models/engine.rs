//! Transcription engine: a loaded model plus its decode session.

use std::path::Path;

use transcribe_cpp::{
    Model, ModelOptions, RunExtension, RunOptions, Session, SessionOptions, TimestampKind,
    WhisperRunOptions,
};

use crate::models::catalog::ModelEntry;
use crate::models::provider::{detect_backend, select_model};
use crate::models::transcript::{TimedSegment, Word};

/// Sung/noise content; the whole segment is dropped.
const MUSIC_NOTE: char = '♪';

/// Above this probability a window is treated as non-speech.
const NO_SPEECH_THOLD: f32 = 0.6;
/// Repetition guard against hallucination loops.
const COMPRESSION_RATIO_THOLD: f32 = 2.4;
/// Fallback ladder: a decode failing the thresholds retries at t = 0.2, 0.4, …
const TEMPERATURE: f32 = 0.0;
const TEMPERATURE_INC: f32 = 0.2;
/// Disables the ladder for provisional passes.
const TEMPERATURE_INC_OFF: f32 = 0.0;
const LOGPROB_THOLD: f32 = -1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Streaming,
    Final,
}

pub struct TranscriptionEngine {
    session: Session,
    /// Kept for device queries after load.
    model: Model,
    /// Finest granularity this model accepts, resolved once at load.
    timestamps: TimestampKind,
    /// Whether the model drives its own streaming session, read once at load.
    supports_streaming: bool,
    /// Codes the model accepts; empty means it advertises none.
    languages: Vec<String>,
    pub entry: &'static ModelEntry,
}

/// The user-facing message for a model that would not load; the engine's own
/// wording stays in the log.
fn load_failure(entry: &ModelEntry, cause: &dyn std::fmt::Display) -> String {
    log::error!("failed to load {}: {cause}", entry.display_name);
    format!(
        "Couldn't load {}. The file may be damaged - try downloading it again.",
        entry.display_name
    )
}

impl TranscriptionEngine {
    /// Load the model selected by `override_id` (or hardware) from `models_dir`.
    pub fn new(models_dir: &Path, override_id: Option<&str>) -> Result<Self, String> {
        let backend = detect_backend();
        let entry = select_model(backend, override_id);

        let model_path = models_dir.join(&entry.filename);
        if !model_path.exists() {
            log::error!("model file missing: {}", model_path.display());
            return Err(format!("{} isn't downloaded.", entry.display_name));
        }

        let model = Model::load_with(&model_path, &ModelOptions::default())
            .map_err(|e| load_failure(entry, &e))?;

        // The bound backend may differ from the request (e.g. CPU fallback).
        log::info!(
            "loaded {} (arch {}, backend {})",
            entry.display_name,
            model.arch(),
            model.backend()
        );

        // Half the cores, leaving headroom for capture and the UI thread.
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let n_threads = (std::thread::available_parallelism().map_or(4, std::num::NonZero::get) / 2)
            .clamp(1, 4) as i32;

        let session = model
            .session_with(&SessionOptions {
                n_threads,
                ..Default::default()
            })
            .map_err(|e| load_failure(entry, &e))?;

        // Asking for finer than the model offers is rejected outright.
        let caps = model.capabilities().map_err(|e| load_failure(entry, &e))?;
        let timestamps = match caps.max_timestamp_kind {
            TimestampKind::None => TimestampKind::None,
            TimestampKind::Segment => TimestampKind::Segment,
            _ => TimestampKind::Word,
        };
        log::info!("timestamp granularity: {timestamps:?}");

        let mut engine = Self {
            session,
            model,
            timestamps,
            supports_streaming: caps.supports_streaming,
            languages: caps.languages,
            entry,
        };

        // Warmup pass so the first real transcription isn't stalled by load.
        let silence = vec![0.0f32; 16_000];
        let _ = engine.transcribe_segments(&silence, "", Pass::Final, None);
        log::info!("engine warmed up");

        Ok(engine)
    }

    /// Codes this model advertises. Empty means it accepts any hint.
    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// The transcript's known language: the decode hint, else the model's only
    /// one. `None` when it auto-detects among several.
    pub fn output_language(&self, language: Option<&str>) -> Option<String> {
        self.hint(language)
            .or_else(|| match self.languages.as_slice() {
                [only] => Some(only.clone()),
                _ => None,
            })
    }

    /// What a decode in the saved `language` is pinned to; `None` auto-detects.
    /// English-only models know nothing else, whatever the setting says.
    fn hint(&self, language: Option<&str>) -> Option<String> {
        let wanted = if self.entry.multilingual {
            crate::models::language::resolve(language)
        } else {
            Some("en")
        };
        resolve_language(&self.languages, wanted)
    }

    /// Backend the model bound to: "cuda", "vulkan" or "cpu".
    pub fn backend(&self) -> String {
        // `Model::backend` names the device ("CUDA0"); the kind is what we show.
        self.model
            .device()
            .map_or_else(|_| "cpu".to_string(), |device| device.kind)
    }

    /// Whether the loaded model can drive its own streaming session.
    pub fn supports_streaming(&self) -> bool {
        self.supports_streaming
    }

    /// Borrow the session mutably, for the streaming path.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// A finished stream reports truncation only here, not through its status.
    pub fn was_truncated(&self) -> bool {
        self.session.was_truncated()
    }

    /// Run options for a streaming session — no prompt, since streaming models
    /// carry their own context across feeds.
    pub fn stream_run_options(&self, language: Option<&str>) -> RunOptions {
        self.run_options("", Pass::Final, language)
    }

    fn run_options(&self, prompt: &str, pass: Pass, language: Option<&str>) -> RunOptions {
        // A non-whisper arch rejects this extension with INVALID_ARG.
        let family = self.entry.is_whisper().then(|| {
            RunExtension::Whisper(WhisperRunOptions {
                initial_prompt: (!prompt.is_empty()).then(|| prompt.to_string()),
                temperature: Some(TEMPERATURE),
                temperature_inc: Some(match pass {
                    Pass::Streaming => TEMPERATURE_INC_OFF,
                    Pass::Final => TEMPERATURE_INC,
                }),
                compression_ratio_thold: Some(COMPRESSION_RATIO_THOLD),
                logprob_thold: Some(LOGPROB_THOLD),
                no_speech_thold: Some(NO_SPEECH_THOLD),
                ..Default::default()
            })
        });

        RunOptions {
            // Decides how finely the pipeline can trim.
            timestamps: self.timestamps,
            language: self.hint(language),
            family,
            ..Default::default()
        }
    }

    /// Transcribe 16 kHz mono f32 samples in the saved `language`.
    pub fn transcribe_segments(
        &mut self,
        samples_16k: &[f32],
        prompt: &str,
        pass: Pass,
        language: Option<&str>,
    ) -> Result<Vec<TimedSegment>, String> {
        // Decoders require at least a second of audio at 16 kHz.
        const MIN_SAMPLES: usize = 16_000;
        let padded;
        let samples = if samples_16k.len() < MIN_SAMPLES {
            padded = {
                let mut v = samples_16k.to_vec();
                v.resize(MIN_SAMPLES, 0.0);
                v
            };
            padded.as_slice()
        } else {
            samples_16k
        };

        let options = self.run_options(prompt, pass, language);
        match self.session.run(samples, &options) {
            Ok(transcript) => Ok(build_segments(&transcript)),
            // Stopped early (repetition loop or length budget): keep the text before it.
            Err(e) => {
                if let Some(partial) = e.partial() {
                    log::warn!("decode stopped early, keeping the partial: {e}");
                    Ok(build_segments(partial))
                } else {
                    log::error!("transcribe failed: {e}");
                    Err("Transcription failed. Please try again.".to_string())
                }
            }
        }
    }
}

/// Keep `language` only if the model advertises it — an unadvertised code is
/// rejected outright, failing every decode. A bare code matches the model's
/// locale for it (`en` → `en-GB`), since models advertise BCP-47; an
/// unsupported one falls back to the model's English, else auto-detect.
fn resolve_language(advertised: &[String], language: Option<&str>) -> Option<String> {
    use crate::models::language::{primary_of, DEFAULT};

    let code = language?;
    if advertised.is_empty() || advertised.iter().any(|l| l == code) {
        return Some(code.to_string());
    }
    if let Some(locale) = advertised.iter().find(|l| primary_of(l) == code) {
        return Some(locale.clone());
    }
    advertised
        .iter()
        .find(|l| primary_of(l) == DEFAULT)
        .cloned()
}

/// Fold a transcript's word rows into the per-segment shape the pipeline wants.
fn build_segments(transcript: &transcribe_cpp::Transcript) -> Vec<TimedSegment> {
    // A model without timestamps reports every segment as ending at 0 ms.
    let timed = transcript.timestamp_kind != TimestampKind::None;
    let mut segments: Vec<TimedSegment> = Vec::with_capacity(transcript.segments.len());

    for (idx, seg) in transcript.segments.iter().enumerate() {
        if seg.text.contains(MUSIC_NOTE) {
            continue;
        }

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let seg_index = idx as i32;
        let mut words: Vec<Word> = transcript
            .words
            .iter()
            .filter(|w| w.seg_index == seg_index)
            .map(|w| Word {
                text: strip_hallucination_tokens(&w.text).trim().to_string(),
                // Milliseconds here; the pipeline works in centiseconds.
                end_cs: (w.t1_ms >= 0).then_some(w.t1_ms / 10),
            })
            .collect();

        // A model that emitted no word rows still yields usable text.
        if words.is_empty() {
            words = split_segment_text(&seg.text);
        }

        words.retain(|w| !w.text.is_empty());
        if words.is_empty() {
            continue;
        }

        segments.push(TimedSegment {
            words,
            end_ms: timed.then_some(seg.t1_ms),
        });
    }

    segments
}

/// Split a segment on whitespace when the model emitted no word rows. The words
/// carry no end time: trimming at the segment's end would drop later words.
fn split_segment_text(text: &str) -> Vec<Word> {
    strip_hallucination_tokens(text)
        .split_whitespace()
        .map(|w| Word {
            text: w.to_string(),
            end_cs: None,
        })
        .collect()
}

/// Remove silence/noise hallucination tokens wherever they appear (case-insensitive).
/// Collapses the doubled space left behind.
fn strip_hallucination_tokens(segment: &str) -> String {
    const TOKENS: [&str; 5] = [
        "[blank_audio]",
        "[silence]",
        "[noise]",
        "[music]",
        "(music)",
    ];

    let mut out = segment.to_string();
    for token in TOKENS {
        loop {
            let lower = out.to_ascii_lowercase();
            let Some(pos) = lower.find(token) else { break };
            out.replace_range(pos..pos + token.len(), "");
        }
    }
    // Token removal can leave "word  word" — collapse runs of spaces.
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    out
}

#[cfg(test)]
#[path = "../../tests/unit/models/engine.rs"]
mod tests;
