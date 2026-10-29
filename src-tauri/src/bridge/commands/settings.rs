//! Preferences shared by the main window and the pill.

use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::bridge::events::PillSettingsChanged;
use crate::settings::{PillSettings, PillSettingsSnapshot, Settings};

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub fn get_pill_settings(settings: State<'_, Settings>) -> PillSettingsSnapshot {
    settings.pill()
}

/// Save the pill's preferences and tell both windows.
#[tauri::command]
#[specta::specta]
pub fn set_pill_settings(
    app: AppHandle,
    settings: State<'_, Settings>,
    pill: PillSettings,
) -> Result<PillSettingsSnapshot, ApiError> {
    let snapshot = settings.set_pill(pill)?;
    let _ = PillSettingsChanged(snapshot).emit(&app);
    Ok(snapshot)
}
