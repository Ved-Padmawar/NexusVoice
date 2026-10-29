// Tauri requires State<'_, T> by value in sync command signatures — this is correct usage.
#![allow(clippy::needless_pass_by_value)]

//! Thin `#[tauri::command]` endpoints, grouped by domain. Business logic lives
//! in the owners they call; shared response/error types in `dto` and `error`.

mod cuda;
mod data;
pub(crate) mod dto;
mod error;
mod hotkey;
mod injection;
mod llm;
mod models;
mod settings;
mod transcription;
mod window;

// Globs are required: `collect_commands!` needs the helper items that
// `#[tauri::command]` and `#[specta::specta]` generate alongside each fn.
pub use cuda::*;
pub use data::*;
pub use hotkey::*;
pub use injection::*;
pub use llm::*;
pub use models::*;
pub use settings::*;
pub use transcription::*;
pub use window::*;
