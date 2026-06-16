//! Parakeet engine (NVIDIA Parakeet-TDT v3, ONNX via transcribe-rs).
//!
//! One-shot: the whole clip is transcribed on finalize (no streaming). The GPU
//! accelerator is selected per build feature: `DirectML` on Windows standard,
//! CUDA on CUDA builds, CPU otherwise.

use std::path::Path;
use std::sync::Mutex;

use transcribe_rs::accel::{set_ort_accelerator, OrtAccelerator};
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};
use transcribe_rs::onnx::Quantization;

use crate::inference::TranscriptionEngine;

/// Subdirectory under `models_dir` holding the Parakeet ONNX artifacts
/// (`encoder-model*`, `decoder_joint-model*`, `nemo128.onnx`, `vocab.txt`).
pub const MODEL_DIR_NAME: &str = "parakeet-tdt-0.6b-v3-int8";

/// Pick the ORT accelerator matching the compiled-in feature.
///
/// `Auto` deliberately excludes `DirectML` in `transcribe-rs` (it needs special
/// session flags), so `DirectML` must be requested explicitly or Parakeet would
/// silently run on CPU on the Windows standard build.
fn accelerator() -> OrtAccelerator {
    if cfg!(feature = "parakeet-cuda") {
        OrtAccelerator::Cuda
    } else if cfg!(feature = "parakeet-directml") {
        OrtAccelerator::DirectMl
    } else {
        OrtAccelerator::Auto
    }
}

pub struct ParakeetEngine {
    model: Mutex<ParakeetModel>,
}

impl ParakeetEngine {
    pub fn new(models_dir: &Path) -> Result<Self, String> {
        let dir = models_dir.join(MODEL_DIR_NAME);
        if !dir.exists() {
            return Err("model not downloaded yet".to_string());
        }

        // Must be set before the model's sessions are created.
        let accel = accelerator();
        log::info!("Parakeet ORT accelerator: {accel}");
        set_ort_accelerator(accel);

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
