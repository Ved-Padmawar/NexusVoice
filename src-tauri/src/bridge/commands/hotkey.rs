//! Global hotkey commands.

use tauri::{AppHandle, State};

use crate::platform::hotkeys::{HotkeyKind, Hotkeys};
use crate::settings::{HotkeyBindings, Settings};

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub fn get_hotkeys(settings: State<'_, Settings>) -> HotkeyBindings {
    settings.hotkeys()
}

/// What is bound with the OS right now. A saved shortcut another application
/// holds is saved but missing here.
#[tauri::command]
#[specta::specta]
pub async fn get_bound_hotkeys(hotkeys: State<'_, Hotkeys>) -> Result<HotkeyBindings, ApiError> {
    Ok(hotkeys.bound().await)
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

/// Release the shortcuts while a new one is being captured. Not saved; they
/// come back on `resume_hotkeys`, a `set_hotkey`, or after a timeout.
#[tauri::command]
#[specta::specta]
pub async fn suspend_hotkeys(app: AppHandle, hotkeys: State<'_, Hotkeys>) -> Result<(), ApiError> {
    Ok(hotkeys.suspend(&app).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn resume_hotkeys(
    app: AppHandle,
    settings: State<'_, Settings>,
    hotkeys: State<'_, Hotkeys>,
) -> Result<(), ApiError> {
    hotkeys.resume(&app, &settings).await;
    Ok(())
}

fn normalize_hotkey(hotkey: String) -> Result<String, ApiError> {
    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err(ApiError::new(
            "hotkey_invalid",
            "A shortcut cannot be empty.",
        ));
    }

    let modifiers = ["Ctrl", "Alt", "Shift", "Super", "Win"];
    let parts: Vec<&str> = hotkey.split('+').collect();
    let has_modifier = parts.iter().any(|p| modifiers.contains(p));
    let has_key = parts.iter().any(|p| !modifiers.contains(p));
    if !has_modifier || !has_key {
        return Err(ApiError::new(
            "hotkey_invalid",
            "A shortcut needs at least one modifier (Ctrl, Alt or Shift) and one key.",
        ));
    }
    Ok(hotkey)
}

#[cfg(test)]
#[path = "../../../tests/unit/bridge/commands/hotkey.rs"]
mod tests;
