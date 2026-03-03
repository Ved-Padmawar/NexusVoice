use crate::inference::errors::InferenceError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ExecutionProvider {
  Cpu,
  Cuda,
  DirectML,
}

pub trait InferenceEngine: Send + Sync {
  #[allow(dead_code)]
  fn execution_provider(&self) -> ExecutionProvider;
  fn run(&self, input: &[f32]) -> Result<Vec<f32>, InferenceError>;
}
