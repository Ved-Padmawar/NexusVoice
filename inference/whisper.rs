use std::path::PathBuf;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::engine::{ExecutionProvider, InferenceEngine};
use super::errors::InferenceError;

/// whisper-rs based transcription engine.
/// `run()` is NOT used by the streaming pipeline — transcription is done via
/// `transcribe()` which takes the full audio buffer and returns the decoded text.
pub struct WhisperEngine {
    context: WhisperContext,
    #[allow(dead_code)]
    provider: ExecutionProvider,
}

impl WhisperEngine {
    /// Load a ggml model file from `model_path`.
    pub fn new(model_path: impl Into<PathBuf>, provider: ExecutionProvider) -> Result<Self, String> {
        let path = model_path.into();
        let path_str = path
            .to_str()
            .ok_or_else(|| "model path is not valid UTF-8".to_string())?;

        let ctx_params = WhisperContextParameters::default();
        let context = WhisperContext::new_with_params(path_str, ctx_params)
            .map_err(|e| format!("failed to load whisper model: {e}"))?;

        Ok(Self { context, provider })
    }

    /// Run full transcription on `samples` (f32 mono, 16 kHz) and return the decoded text.
    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        let mut state = self
            .context
            .create_state()
            .map_err(|e| format!("failed to create whisper state: {e}"))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(num_threads());
        params.set_translate(false);
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        // Suppress hallucinations on silence/noise
        params.set_no_speech_thold(0.6);

        state
            .full(params, samples)
            .map_err(|e| format!("whisper inference failed: {e}"))?;

        let n = state.full_n_segments();

        let mut text = String::new();
        for i in 0..n {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(s) = segment.to_str() {
                    text.push_str(s);
                }
            }
        }

        Ok(text.trim().to_string())
    }
}

/// The `InferenceEngine` trait implementation is a no-op chunk runner —
/// transcription uses `transcribe()` directly on the full buffer.
impl InferenceEngine for WhisperEngine {
    fn execution_provider(&self) -> ExecutionProvider {
        self.provider
    }

    fn run(&self, _input: &[f32]) -> Result<Vec<f32>, InferenceError> {
        Err(InferenceError::NotImplemented)
    }
}

fn num_threads() -> i32 {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    (cpus as i32).min(8)
}
