use std::path::Path;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::inference::provider::{detect_backend, select_model_size, Backend, ModelSize};
use crate::inference::transcript::{TimedSegment, Word};

pub struct WhisperEngine {
    ctx: WhisperContext,
    #[allow(dead_code)]
    pub backend: Backend,
    #[allow(dead_code)]
    pub model_size: ModelSize,
}

impl WhisperEngine {
    /// Load the appropriate ggml model from `models_dir`.
    /// `override_size` ("large" | "medium") lets the user override auto-selection.
    pub fn new(models_dir: &Path, override_size: Option<&str>) -> Result<Self, String> {
        let backend = detect_backend();
        let model_size = select_model_size(backend, override_size);

        log::info!(
            "backend: {}, model: {}",
            backend.as_str(),
            model_size.display_name()
        );

        let model_path = models_dir.join(model_size.filename());
        if !model_path.exists() {
            return Err(format!("model not found: {}", model_path.display()));
        }

        let mut params = WhisperContextParameters::default();
        // GPU acceleration requires "cuda" or "vulkan" crate features at build time.
        // Without them use_gpu(true) is a no-op — whisper-rs falls back to CPU.
        params.use_gpu(backend.has_gpu());

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or("invalid model path")?,
            params,
        )
        .map_err(|e| format!("failed to load whisper model: {e}"))?;

        let engine = Self {
            ctx,
            backend,
            model_size,
        };

        // Warmup pass — forces model weights into GPU/CPU memory so the first real
        // transcription is instant. Feed 1s of silence and discard the output.
        let silence = vec![0.0f32; 16_000];
        let _ = engine.transcribe_segments(&silence, "", 2);
        log::info!("whisper engine warmed up");

        Ok(engine)
    }

    /// Transcribe 16 kHz mono f32 samples into timed segments.
    /// `beam_size` controls the quality/speed tradeoff: 2=Fast, 5=Balanced, 8=Accurate.
    pub fn transcribe_segments(
        &self,
        samples_16k: &[f32],
        prompt: &str,
        beam_size: i32,
    ) -> Result<Vec<TimedSegment>, String> {
        // whisper.cpp requires at least 1 second of audio at 16 kHz
        const MIN_SAMPLES: usize = 16_000;
        let padded;
        let samples_16k = if samples_16k.len() < MIN_SAMPLES {
            padded = {
                let mut v = samples_16k.to_vec();
                v.resize(MIN_SAMPLES, 0.0);
                v
            };
            padded.as_slice()
        } else {
            samples_16k
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        // clamp(1,4) guarantees the value fits i32 on any platform
        let n_threads = (std::thread::available_parallelism().map_or(4, std::num::NonZero::get) / 2)
            .clamp(1, 4) as i32;

        let beam_size = beam_size.clamp(1, 8);
        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size,
            patience: 1.0,
        });
        params.set_n_threads(n_threads);
        // Large variants are multilingual — let Whisper auto-detect the language.
        // The .en models (Tiny/Base/Small/Medium) only know English so we pin it.
        let lang = match self.model_size {
            ModelSize::Large | ModelSize::LargeFull => None,
            _ => Some("en"),
        };
        params.set_language(lang);
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        // Segments with high token entropy are likely hallucinations (e.g. "Thank you for watching").
        // 2.4 is the community-validated threshold — pairs with the no_speech_probability guard below.
        params.set_entropy_thold(2.4);
        // Temperature fallback (whisper.cpp built-in): a decode that fails the
        // entropy/logprob thresholds is retried at t = 0.2, 0.4, … — this
        // rescues chunks that would otherwise come back garbled or looping.
        // Pinned explicitly so we don't depend on upstream defaults.
        params.set_temperature(0.0);
        params.set_temperature_inc(0.2);
        params.set_logprob_thold(-1.0);
        params.set_suppress_blank(true);
        if !prompt.is_empty() {
            params.set_initial_prompt(prompt);
        }

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("whisper state: {e}"))?;

        state
            .full(params, samples_16k)
            .map_err(|e| format!("whisper full: {e}"))?;

        let n = state.full_n_segments();

        let mut segments: Vec<TimedSegment> = Vec::new();
        for i in 0..n {
            let Some(seg) = state.get_segment(i) else {
                continue;
            };
            // Drop segments whisper flagged as silence/noise
            if seg.no_speech_probability() > 0.6 {
                continue;
            }
            // Music-note segments are sung/noise content — drop whole segment
            if seg.to_str_lossy().is_ok_and(|s| s.contains('♪')) {
                continue;
            }

            let mut words: Vec<Word> = Vec::new();
            for t in 0..seg.n_tokens() {
                let Some(token) = seg.get_token(t) else {
                    continue;
                };
                let Ok(raw) = token.to_str_lossy() else {
                    continue;
                };
                // Special tokens (<|endoftext|>, timestamps) carry no text.
                if raw.starts_with("[_") || raw.starts_with("<|") {
                    continue;
                }
                if raw.trim().is_empty() {
                    continue;
                }

                words.push(Word {
                    text: raw.to_string(),
                });
            }

            // Strip hallucination markers after merging, not before: whisper splits
            // "[BLANK_AUDIO]" across several BPE tokens, so no single token matches
            // the full marker and it survives into the transcript.
            let mut words = merge_subword_tokens(words);
            for w in &mut words {
                w.text = strip_hallucination_tokens(&w.text).trim().to_string();
            }
            words.retain(|w| !w.text.is_empty());
            if words.is_empty() {
                continue;
            }

            segments.push(TimedSegment {
                words,
                // whisper timestamps are centiseconds
                end_ms: seg.end_timestamp() * 10,
            });
        }

        Ok(segments)
    }
}

/// Fold BPE fragments into words: "unbelievable" arrives as " unbe" + "lie" +
/// "vable", and only a leading space marks a new word.
fn merge_subword_tokens(tokens: Vec<Word>) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::with_capacity(tokens.len());
    for tok in tokens {
        let starts_word = tok.text.starts_with(' ') || out.is_empty();
        if starts_word {
            out.push(tok);
        } else if let Some(prev) = out.last_mut() {
            prev.text.push_str(&tok.text);
        }
    }
    for w in &mut out {
        w.text = w.text.trim().to_string();
    }
    out.retain(|w| !w.text.is_empty());
    out
}

/// Remove Whisper's silence/noise hallucination tokens wherever they appear
/// in a segment (case-insensitive). Collapses the doubled space left behind.
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
