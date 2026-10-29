//! Smart-formatting commands: the provider settings, and a connection test.

use tauri::State;

use crate::settings::formatter::{FormatterUpdate, FormatterView};
use crate::settings::Settings;

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub fn get_format_config(settings: State<'_, Settings>) -> FormatterView {
    settings.formatter()
}

#[tauri::command]
#[specta::specta]
pub fn set_format_config(
    settings: State<'_, Settings>,
    config: FormatterUpdate,
) -> Result<(), ApiError> {
    settings
        .set_formatter(config)
        .map_err(|e| ApiError::new("settings_error", e))
}

/// Send one trivial request with a draft from the provider modal. Saves
/// nothing.
#[tauri::command]
#[specta::specta]
pub async fn test_format_connection(
    settings: State<'_, Settings>,
    config: FormatterUpdate,
) -> Result<(), ApiError> {
    let config = settings.format_config_for(&config);
    let active = config.active();
    if config.provider != "anthropic" && active.base_url.trim().is_empty() {
        return Err(ApiError::new("invalid_input", "base URL is required"));
    }
    if active.model.trim().is_empty() {
        return Err(ApiError::new("invalid_input", "model name is required"));
    }
    crate::text::llm::client::test_connection(&config)
        .await
        .map_err(|e| ApiError::new("connection_failed", e))
}
