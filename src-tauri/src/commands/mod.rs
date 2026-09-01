// Tauri requires State<'_, T> by value in sync command signatures — this is correct usage.
#![allow(clippy::needless_pass_by_value)]

//! Thin `#[tauri::command]` endpoints, grouped by domain. Business logic lives
//! in service modules; shared response/error types in `dto` and `error`.

mod data;
pub(crate) mod dto;
mod error;
mod hotkey;
mod injection;
mod llm;
mod models;
mod transcription;
mod window;

// Globs are required: `collect_commands!` needs the helper items that
// `#[tauri::command]` and `#[specta::specta]` generate alongside each fn.
#[allow(unused_imports)] // public surface — commands return this type
pub use error::ApiError;

pub use data::*;
pub use hotkey::*;
pub use injection::*;
pub use llm::*;
pub use models::*;
pub use transcription::*;
pub use window::*;
