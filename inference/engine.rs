use super::errors::InferenceError;

pub trait InferenceEngine: Send + Sync {
    fn run(&self, input: &[f32]) -> Result<Vec<f32>, InferenceError>;
}
