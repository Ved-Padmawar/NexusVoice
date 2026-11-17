//! Smart-formatting commands: the provider settings, and a connection test.

use tauri::{AppHandle, Manager, State};

use crate::mirror::Committed;
use crate::settings::formatter::FormatterDraft;
use crate::settings::Settings;
use crate::text::llm::config::check_base_url;

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub fn set_format_enabled(
    settings: State<'_, Settings>,
    enabled: bool,
) -> Result<Committed, ApiError> {
    Ok(settings.set_formatter_enabled(enabled)?)
}

/// Save the provider modal's endpoint as the active one, and turn formatting on.
#[tauri::command]
#[specta::specta]
pub async fn save_format_endpoint(
    app: AppHandle,
    draft: FormatterDraft,
) -> Result<Committed, ApiError> {
    draft
        .validate()
        .map_err(|message| ApiError::new("invalid_input", message))?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Settings>().save_formatter_endpoint(&draft)
    })
    .await
    .map_err(|e| ApiError::new("settings_error", e.to_string()))?
    .map_err(|e| ApiError::new("settings_error", e))
}

/// Send one trivial request with a draft from the provider modal. Saves
/// nothing.
#[tauri::command]
#[specta::specta]
pub async fn test_format_connection(app: AppHandle, draft: FormatterDraft) -> Result<(), ApiError> {
    let config = tauri::async_runtime::spawn_blocking(move || {
        app.state::<Settings>().format_config_for(&draft)
    })
    .await
    .map_err(|e| ApiError::new("settings_error", e.to_string()))?;
    let active = config.active();
    if config.provider != "anthropic" {
        check_base_url(&active.base_url)
            .map_err(|message| ApiError::new("invalid_input", message))?;
    }
    if active.model.trim().is_empty() {
        return Err(ApiError::new("invalid_input", "model name is required"));
    }
    crate::text::llm::client::test_connection(&config)
        .await
        .map_err(|e| ApiError::new("connection_failed", e))
}
