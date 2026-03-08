use super::engine::InferenceEngine;
use super::errors::InferenceError;

type InferenceHandler = Box<dyn Fn(&[f32]) -> Result<Vec<f32>, InferenceError> + Send + Sync>;

#[allow(dead_code)]
pub struct MockInferenceEngine {
    handler: InferenceHandler,
}

#[allow(dead_code)]
impl MockInferenceEngine {
    pub fn new(
        handler: impl Fn(&[f32]) -> Result<Vec<f32>, InferenceError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            handler: Box::new(handler),
        }
    }
}

impl InferenceEngine for MockInferenceEngine {
    fn run(&self, input: &[f32]) -> Result<Vec<f32>, InferenceError> {
        (self.handler)(input)
    }
}
