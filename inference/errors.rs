use std::fmt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum InferenceError {
    NotImplemented,
    InvalidInput(String),
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented => write!(f, "inference engine not implemented"),
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
        }
    }
}

impl std::error::Error for InferenceError {}
