//! The operations the UI performs, grouped by domain. Each is a thin call over
//! the app state and the service modules, returning [`ApiError`] on failure.
//! Async ones run on the tokio runtime; the UI reaches them through
//! `ui::runtime`.

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
