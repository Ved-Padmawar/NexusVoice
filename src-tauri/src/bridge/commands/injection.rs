//! Text injection status and the paste preference. Pasting itself happens in finalize.

use tauri::State;

use super::dto::InjectionStatus;
use super::error::ApiError;
use crate::settings::{PasteMode, Settings};

/// Linux-only; the clipboard platforms have nothing to configure.
#[tauri::command]
#[specta::specta]
pub async fn get_injection_status() -> InjectionStatus {
    InjectionStatus::detect().await
}

#[tauri::command]
#[specta::specta]
pub fn get_paste_mode(settings: State<'_, Settings>) -> PasteMode {
    settings.paste_mode()
}

#[tauri::command]
#[specta::specta]
pub fn set_paste_mode(settings: State<'_, Settings>, mode: PasteMode) -> Result<(), ApiError> {
    Ok(settings.set_paste_mode(mode)?)
}
