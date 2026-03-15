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

        Ok(Self { ctx, backend, model_size })
    }

    /// Transcribe 16 kHz mono f32 samples. `prompt` biases recognition.
    pub fn transcribe(&mut self, samples_16k: &[f32], prompt: &str) -> Result<String, String> {
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
        let n_threads = (std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4) / 2)
            .max(1) as i32;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });
        params.set_n_threads(n_threads);
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
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
                    text.push_str(&s);
                }
            }
        }

        Ok(text.trim().to_string())
    }
}
