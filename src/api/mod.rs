//! The operations the UI performs, grouped by domain, failing with [`ApiError`].

mod cuda;
mod data;
pub mod dto;
mod error;
mod hotkey;
mod injection;
mod llm;
mod models;
mod transcription;
mod window;

pub use error::ApiError;

pub use cuda::*;
pub use data::*;
pub use hotkey::*;
pub use injection::*;
pub use llm::*;
pub use models::*;
pub use transcription::*;
pub use window::*;
