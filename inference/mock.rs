#![allow(dead_code)]

use super::engine::{ExecutionProvider, InferenceEngine};
use super::errors::InferenceError;

type InferenceHandler = Box<dyn Fn(&[f32]) -> Result<Vec<f32>, InferenceError> + Send + Sync>;

pub struct MockInferenceEngine {
    #[allow(dead_code)]
    provider: ExecutionProvider,
    handler: InferenceHandler,
}

impl MockInferenceEngine {
    pub fn new(
        provider: ExecutionProvider,
        handler: impl Fn(&[f32]) -> Result<Vec<f32>, InferenceError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            provider,
            handler: Box::new(handler),
        }
    }
}

impl InferenceEngine for MockInferenceEngine {
    fn execution_provider(&self) -> ExecutionProvider {
        self.provider
    }

    fn run(&self, input: &[f32]) -> Result<Vec<f32>, InferenceError> {
        (self.handler)(input)
    }
}
