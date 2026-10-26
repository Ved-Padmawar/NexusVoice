//! Global hotkey commands.

use tauri::{AppHandle, State};

use crate::hotkeys::{HotkeyKind, Hotkeys};
use crate::settings::{HotkeyBindings, Settings};

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub fn get_hotkeys(settings: State<'_, Settings>) -> HotkeyBindings {
    settings.hotkeys()
}

/// Bind `hotkey` to `kind`, or unbind it with `None`.
#[tauri::command]
#[specta::specta]
pub async fn set_hotkey(
    app: AppHandle,
    hotkeys: State<'_, Hotkeys>,
    settings: State<'_, Settings>,
    kind: HotkeyKind,
    hotkey: Option<String>,
) -> Result<(), ApiError> {
    let hotkey = hotkey.map(normalize_hotkey).transpose()?;
    Ok(hotkeys.set(&app, &settings, kind, hotkey).await?)
}

fn normalize_hotkey(hotkey: String) -> Result<String, ApiError> {
    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err(ApiError::new("hotkey_invalid", "hotkey cannot be empty"));
    }

    let modifiers = ["Ctrl", "Alt", "Shift", "Super", "Win"];
    let parts: Vec<&str> = hotkey.split('+').collect();
    let has_modifier = parts.iter().any(|p| modifiers.contains(p));
    let has_key = parts.iter().any(|p| !modifiers.contains(p));
    if !has_modifier || !has_key {
        return Err(ApiError::new(
            "hotkey_invalid",
            "hotkey must include at least one modifier (Ctrl/Alt/Shift) and one key",
        ));
    }
    Ok(hotkey)
}

#[cfg(test)]
#[path = "../../tests/unit/commands/hotkey.rs"]
mod tests;
