use std::path::Path;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::inference::provider::{detect_backend, select_model_size, Backend, ModelSize};

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
        let _ = engine.transcribe(&silence, "", 2);
        log::info!("whisper engine warmed up");

        Ok(engine)
    }

    /// Transcribe 16 kHz mono f32 samples. `prompt` biases recognition.
    /// `beam_size` controls the quality/speed tradeoff: 2=Fast, 5=Balanced, 8=Accurate.
    pub fn transcribe(
        &self,
        samples_16k: &[f32],
        prompt: &str,
        beam_size: i32,
    ) -> Result<String, String> {
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
        let n_threads = (std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(4)
            / 2)
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

        let mut text = String::new();
        for i in 0..n {
            if let Some(seg) = state.get_segment(i) {
                // Drop segments whisper flagged as silence/noise
                if seg.no_speech_probability() > 0.6 {
                    continue;
                }
                if let Ok(s) = seg.to_str_lossy() {
                    // Music-note segments are sung/noise content — drop whole segment
                    if s.contains('♪') {
                        continue;
                    }
                    // Strip hallucination tokens — Whisper emits these on
                    // silence/noise, sometimes embedded inside a real segment
                    // ("[BLANK_AUDIO] so anyway…"), so removal must be
                    // substring-based, not whole-segment matching.
                    let cleaned = strip_hallucination_tokens(&s);
                    if cleaned.trim().is_empty() {
                        continue;
                    }
                    text.push_str(&cleaned);
                }
            }
        }

        Ok(text.trim().to_string())
    }
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
mod tests {
    use super::strip_hallucination_tokens;

    #[test]
    fn strips_embedded_blank_audio_token() {
        assert_eq!(
            strip_hallucination_tokens("[Blank_Audio] so anyway we continue"),
            " so anyway we continue"
        );
    }

    #[test]
    fn strips_token_only_segment_to_empty() {
        assert!(strip_hallucination_tokens(" [BLANK_AUDIO] ")
            .trim()
            .is_empty());
    }

    #[test]
    fn strips_multiple_tokens_and_collapses_spaces() {
        assert_eq!(
            strip_hallucination_tokens("hello [noise] world [SILENCE]"),
            "hello world "
        );
    }

    #[test]
    fn leaves_normal_text_untouched() {
        let s = "the audio was blank but fine";
        assert_eq!(strip_hallucination_tokens(s), s);
    }
}
