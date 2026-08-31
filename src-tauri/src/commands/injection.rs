//! Text injection commands. The platform strategies live in `crate::injection`.

use tauri::AppHandle;

use super::error::ApiError;

#[tauri::command]
pub async fn type_text(app: AppHandle, text: String) -> Result<(), ApiError> {
    crate::injection::type_text(&app, &text)
        .await
        .map_err(|e| ApiError::new("injection_failed", e))
}

/// Linux-only; the clipboard platforms have nothing to configure.
#[tauri::command]
pub async fn get_injection_status() -> Result<super::dto::InjectionStatus, ApiError> {
    Ok(super::dto::InjectionStatus::detect().await)
}
