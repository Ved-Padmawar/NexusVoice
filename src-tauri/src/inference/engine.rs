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

        log::info!("backend: {}, model: {}", backend.as_str(), model_size.display_name());

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

        let mut engine = Self { ctx, backend, model_size };

        // Warmup pass — forces model weights into GPU/CPU memory so the first real
        // transcription is instant. Feed 1s of silence and discard the output.
        let silence = vec![0.0f32; 16_000];
        let _ = engine.transcribe(&silence, "", 2);
        log::info!("whisper engine warmed up");

        Ok(engine)
    }

    /// Transcribe 16 kHz mono f32 samples. `prompt` biases recognition.
    /// `beam_size` controls the quality/speed tradeoff: 2=Fast, 5=Balanced, 8=Accurate.
    pub fn transcribe(&mut self, samples_16k: &[f32], prompt: &str, beam_size: i32) -> Result<String, String> {
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
            .unwrap_or(4) / 2).clamp(1, 4) as i32;

        let beam_size = beam_size.clamp(1, 8);
        let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size, patience: 1.0 });
        params.set_n_threads(n_threads);
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        // Segments with high token entropy are likely hallucinations (e.g. "Thank you for watching").
        // 2.4 is the community-validated threshold — pairs with the no_speech_probability guard below.
        params.set_entropy_thold(2.4);
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
                    // Drop hallucination tokens — Whisper emits these on silence/noise segments
                    let trimmed = s.trim();
                    if trimmed.eq_ignore_ascii_case("[blank_audio]")
                        || trimmed.eq_ignore_ascii_case("[silence]")
                        || trimmed.eq_ignore_ascii_case("[noise]")
                        || trimmed.eq_ignore_ascii_case("[music]")
                        || trimmed.eq_ignore_ascii_case("(music)")
                        || trimmed.contains('♪')
                    {
                        continue;
                    }
                    text.push_str(&s);
                }
            }
        }

        Ok(text.trim().to_string())
    }
}
