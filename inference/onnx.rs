use std::path::PathBuf;

use super::engine::InferenceEngine;
use super::errors::InferenceError;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ONNXRuntimeEngine {
    model_path: PathBuf,
}

#[allow(dead_code)]
impl ONNXRuntimeEngine {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
        }
    }
}

impl InferenceEngine for ONNXRuntimeEngine {
    fn run(&self, _input: &[f32]) -> Result<Vec<f32>, InferenceError> {
        Err(InferenceError::NotImplemented)
    }
}
