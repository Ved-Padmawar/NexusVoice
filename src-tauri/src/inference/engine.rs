//! Transcription engine: a loaded model plus its decode session.

use std::path::Path;

use transcribe_cpp::{
    Model, ModelOptions, RunExtension, RunOptions, Session, SessionOptions, TimestampKind,
    WhisperRunOptions,
};

use crate::inference::catalog::ModelEntry;
use crate::inference::provider::{detect_backend, select_model};
use crate::inference::transcript::{TimedSegment, Word};

/// Sung/noise content; the whole segment is dropped.
const MUSIC_NOTE: char = '♪';

/// Above this probability a window is treated as non-speech.
const NO_SPEECH_THOLD: f32 = 0.6;
/// Repetition guard against hallucination loops.
const COMPRESSION_RATIO_THOLD: f32 = 2.4;
/// Fallback ladder: a decode failing the thresholds retries at t = 0.2, 0.4, …
const TEMPERATURE: f32 = 0.0;
const TEMPERATURE_INC: f32 = 0.2;
const LOGPROB_THOLD: f32 = -1.0;

pub struct TranscriptionEngine {
    session: Session,
    /// Kept for capability queries after load.
    model: Model,
    /// Finest granularity this model accepts, resolved once at load.
    timestamps: TimestampKind,
    pub entry: &'static ModelEntry,
}

impl TranscriptionEngine {
    /// Load the model selected by `override_id` (or hardware) from `models_dir`.
    pub fn new(models_dir: &Path, override_id: Option<&str>) -> Result<Self, String> {
        let backend = detect_backend();
        let entry = select_model(backend, override_id);

        let model_path = models_dir.join(&entry.filename);
        if !model_path.exists() {
            return Err(format!("model not found: {}", model_path.display()));
        }

        let model = Model::load_with(&model_path, &ModelOptions::default())
            .map_err(|e| format!("failed to load model {}: {e}", entry.display_name))?;

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
            .map_err(|e| format!("failed to create session: {e}"))?;

        // Asking for finer than the model offers is rejected outright.
        let timestamps = match model.capabilities().max_timestamp_kind {
            TimestampKind::None => TimestampKind::None,
            TimestampKind::Segment => TimestampKind::Segment,
            _ => TimestampKind::Word,
        };
        log::info!("timestamp granularity: {timestamps:?}");

        let mut engine = Self {
            session,
            model,
            timestamps,
            entry,
        };

        // Warmup pass so the first real transcription isn't stalled by load.
        let silence = vec![0.0f32; 16_000];
        let _ = engine.transcribe_segments(&silence, "");
        log::info!("engine warmed up");

        Ok(engine)
    }

    /// Whether the loaded model can drive its own streaming session.
    pub fn supports_streaming(&self) -> bool {
        self.model.capabilities().supports_streaming
    }

    /// Borrow the session mutably, for the streaming path.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Run options for a streaming session — no prompt, since streaming models
    /// carry their own context across feeds.
    pub fn stream_run_options(&self) -> RunOptions {
        self.run_options("")
    }

    fn run_options(&self, prompt: &str) -> RunOptions {
        // A non-whisper arch rejects this extension with INVALID_ARG.
        let family = self.entry.is_whisper().then(|| {
            RunExtension::Whisper(WhisperRunOptions {
                initial_prompt: (!prompt.is_empty()).then(|| prompt.to_string()),
                temperature: Some(TEMPERATURE),
                temperature_inc: Some(TEMPERATURE_INC),
                compression_ratio_thold: Some(COMPRESSION_RATIO_THOLD),
                logprob_thold: Some(LOGPROB_THOLD),
                no_speech_thold: Some(NO_SPEECH_THOLD),
                ..Default::default()
            })
        });

        RunOptions {
            // Word timestamps drive the pipeline's mid-segment trim.
            timestamps: self.timestamps,
            // Multilingual models auto-detect; .en models only know English.
            language: (!self.entry.multilingual).then(|| "en".to_string()),
            family,
            ..Default::default()
        }
    }

    /// Transcribe 16 kHz mono f32 samples into timed segments.
    pub fn transcribe_segments(
        &mut self,
        samples_16k: &[f32],
        prompt: &str,
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

        let options = self.run_options(prompt);
        let transcript = self
            .session
            .run(samples, &options)
            .map_err(|e| format!("transcribe failed: {e}"))?;

        Ok(build_segments(&transcript))
    }
}

/// Fold a transcript's word rows into the per-segment shape the pipeline wants.
fn build_segments(transcript: &transcribe_cpp::Transcript) -> Vec<TimedSegment> {
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
            words = split_segment_text(&seg.text, seg.t1_ms);
        }

        words.retain(|w| !w.text.is_empty());
        if words.is_empty() {
            continue;
        }

        segments.push(TimedSegment {
            words,
            end_ms: seg.t1_ms,
        });
    }

    segments
}

/// Split a segment on whitespace when the model emitted no word rows. Every
/// word carries the segment's end time — coarse, but the trim logic still works.
fn split_segment_text(text: &str, end_ms: i64) -> Vec<Word> {
    strip_hallucination_tokens(text)
        .split_whitespace()
        .map(|w| Word {
            text: w.to_string(),
            end_cs: (end_ms >= 0).then_some(end_ms / 10),
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
#[path = "../../tests/unit/inference/engine.rs"]
mod tests;
