// Tauri requires State<'_, T> by value in sync command signatures — this is correct usage.
#![allow(clippy::needless_pass_by_value)]

//! Command layer: thin `#[tauri::command]` endpoints grouped by domain.
//!
//! Each submodule exposes the commands for one domain. Business logic lives in
//! service modules (e.g. `crate::transcription`) — these files only validate
//! input, call services, and shape responses. Shared error/response types live
//! in `error` and `dto`.

mod auth;
mod data;
pub(crate) mod dto;
mod error;
mod hotkey;
mod injection;
mod llm;
mod models;
mod transcription;
mod window;

// Re-export every command and the shared error type so existing `commands::*`
// paths in main.rs keep working unchanged. Glob re-exports are required so that
// the helper items `#[tauri::command]` generates alongside each fn (consumed by
// `generate_handler!`) are also brought into this module's namespace.
#[allow(unused_imports)] // public surface — commands return this type
pub use error::ApiError;

pub use auth::*;
pub use data::*;
pub use hotkey::*;
pub use injection::*;
pub use llm::*;
pub use models::*;
pub use transcription::*;
pub use window::*;
