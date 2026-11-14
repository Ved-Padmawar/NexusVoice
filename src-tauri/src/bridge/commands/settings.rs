//! Preferences shared by the main window and the pill.

use tauri::State;

use crate::mirror::Committed;
use crate::settings::{PillSetting, Settings};

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub fn set_pill_setting(
    settings: State<'_, Settings>,
    change: PillSetting,
) -> Result<Committed, ApiError> {
    Ok(settings.set_pill(change)?)
}
