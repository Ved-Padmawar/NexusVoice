//! Parakeet engine (NVIDIA Parakeet-TDT v3, ONNX via transcribe-rs).
//!
//! One-shot: the whole clip is transcribed on finalize (no streaming). The GPU
//! accelerator (`DirectML` / CUDA) is chosen at build time via `transcribe-rs`
//! `ort` features; `Auto` falls back to CPU when no GPU provider is compiled in.

use std::path::Path;
use std::sync::Mutex;

use transcribe_rs::accel::{set_ort_accelerator, OrtAccelerator};
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};
use transcribe_rs::onnx::Quantization;

use crate::inference::TranscriptionEngine;

/// Subdirectory under `models_dir` holding the Parakeet ONNX artifacts
/// (`encoder-model*`, `decoder_joint-model*`, `nemo128.onnx`, `vocab.txt`).
pub const MODEL_DIR_NAME: &str = "parakeet-tdt-0.6b-v3-int8";

pub struct ParakeetEngine {
    model: Mutex<ParakeetModel>,
}

impl ParakeetEngine {
    pub fn new(models_dir: &Path) -> Result<Self, String> {
        let dir = models_dir.join(MODEL_DIR_NAME);
        if !dir.exists() {
            return Err("model not downloaded yet".to_string());
        }

        // Auto picks the best compiled-in ORT provider (DirectML/CUDA) and falls
        // back to CPU. Must be set before the model's sessions are created.
        set_ort_accelerator(OrtAccelerator::Auto);

        let model = ParakeetModel::load(&dir, &Quantization::Int8)
            .map_err(|e| format!("failed to load Parakeet model: {e}"))?;

        Ok(Self {
            model: Mutex::new(model),
        })
    }
}

impl TranscriptionEngine for ParakeetEngine {
    fn transcribe(
        &self,
        samples_16k: &[f32],
        _prompt: &str,
        _beam_size: i32,
    ) -> Result<String, String> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| "Parakeet model mutex poisoned".to_string())?;

        let result = model
            .transcribe_with(samples_16k, &ParakeetParams::default())
            .map_err(|e| format!("Parakeet transcription failed: {e}"))?;

        Ok(result.text.trim().to_string())
    }

    fn supports_streaming(&self) -> bool {
        false
    }
}
