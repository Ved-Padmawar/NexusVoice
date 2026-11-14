//! Preferences shared by the main window and the pill.

use tauri::State;

use crate::settings::{PillSetting, PillSettings, Settings};

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub fn get_pill_settings(settings: State<'_, Settings>) -> PillSettings {
    settings.pill()
}

#[tauri::command]
#[specta::specta]
pub fn set_pill_setting(
    settings: State<'_, Settings>,
    change: PillSetting,
) -> Result<(), ApiError> {
    Ok(settings.set_pill(change)?)
}
