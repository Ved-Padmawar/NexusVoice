//! Formatter LLM commands: read/write the formatter config and test an
//! endpoint. The formatter is an OpenAI-compatible HTTP call — no local model,
//! no download — so these are thin config + connectivity-check endpoints.

use tauri::State;

use crate::llm::config::FormatConfig;
use crate::state::AppState;

use super::error::ApiError;

#[tauri::command]
pub async fn get_format_config(state: State<'_, AppState>) -> Result<FormatConfig, ApiError> {
    Ok(state.load_format_config())
}

#[tauri::command]
pub async fn set_format_config(
    state: State<'_, AppState>,
    config: FormatConfig,
) -> Result<(), ApiError> {
    state
        .save_format_config(&config)
        .map_err(|e| ApiError::new("io_error", e.to_string()))
}

/// Validate a candidate config (from the provider modal's "Test" button) by
/// sending one trivial formatting request. Does not persist anything.
#[tauri::command]
pub async fn test_format_connection(config: FormatConfig) -> Result<(), ApiError> {
    let active = config.active();
    if config.provider != "anthropic" && active.base_url.trim().is_empty() {
        return Err(ApiError::new("invalid_input", "base URL is required"));
    }
    if active.model.trim().is_empty() {
        return Err(ApiError::new("invalid_input", "model name is required"));
    }
    crate::llm::client::test_connection(&config)
        .await
        .map_err(|e| ApiError::new("connection_failed", e))
}
