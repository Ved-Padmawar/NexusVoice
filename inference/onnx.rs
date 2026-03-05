use std::path::PathBuf;

use super::engine::{ExecutionProvider, InferenceEngine};
use super::errors::InferenceError;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ONNXRuntimeEngine {
    model_path: PathBuf,
    provider: ExecutionProvider,
}

#[allow(dead_code)]
impl ONNXRuntimeEngine {
    pub fn new(model_path: impl Into<PathBuf>, provider: ExecutionProvider) -> Self {
        Self {
            model_path: model_path.into(),
            provider,
        }
    }

    pub fn model_path(&self) -> &PathBuf {
        &self.model_path
    }
}

impl InferenceEngine for ONNXRuntimeEngine {
    fn execution_provider(&self) -> ExecutionProvider {
        self.provider
    }

    fn run(&self, _input: &[f32]) -> Result<Vec<f32>, InferenceError> {
        Err(InferenceError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_is_injected() {
        let engine = ONNXRuntimeEngine::new("model.onnx", ExecutionProvider::Cuda);
        assert_eq!(engine.execution_provider(), ExecutionProvider::Cuda);
    }

    #[test]
    fn run_returns_not_implemented() {
        let engine = ONNXRuntimeEngine::new("model.onnx", ExecutionProvider::Cpu);
        let err = engine.run(&[0.0]).expect_err("should fail");

        match err {
            InferenceError::NotImplemented => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
