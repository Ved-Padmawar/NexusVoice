//! Text injection status. Pasting itself happens in finalize.

use super::dto::InjectionStatus;

/// Linux-only; the clipboard platforms have nothing to configure.
#[tauri::command]
#[specta::specta]
pub async fn get_injection_status() -> InjectionStatus {
    InjectionStatus::detect().await
}
