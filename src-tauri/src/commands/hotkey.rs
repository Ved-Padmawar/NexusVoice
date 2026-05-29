//! Global hotkey commands: register, unregister, and query the active hotkey.

use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

use super::error::ApiError;

#[tauri::command]
pub async fn register_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<bool, ApiError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err(ApiError::new("hotkey_invalid", "hotkey cannot be empty"));
    }
    let parts: Vec<&str> = hotkey.split('+').collect();
    let modifiers = ["Ctrl", "Alt", "Shift", "Super", "Win"];
    let has_modifier = parts.iter().any(|p| modifiers.contains(p));
    let has_key = parts.iter().any(|p| !modifiers.contains(p));
    if !has_modifier || !has_key {
        return Err(ApiError::new(
            "hotkey_invalid",
            "hotkey must include at least one modifier (Ctrl/Alt/Shift) and one key",
        ));
    }

    app.global_shortcut()
        .unregister_all()
        .map_err(|e| ApiError::new("hotkey_unregister_failed", e.to_string()))?;

    let app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey.as_str(), move |_app, _shortcut, event| {
            use tauri_plugin_global_shortcut::ShortcutState;
            if event.state == ShortcutState::Pressed {
                let _ = app_clone.emit("hotkey-pressed", ());
            } else {
                let _ = app_clone.emit("hotkey-released", ());
            }
        })
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("already registered") || msg.contains("already in use") {
                ApiError::new(
                    "hotkey_already_in_use",
                    "this hotkey is already in use by another application",
                )
            } else if msg.contains("permission") || msg.contains("access") {
                ApiError::new(
                    "hotkey_permission_denied",
                    "OS denied hotkey registration — try a different combination",
                )
            } else {
                ApiError::new("hotkey_register_failed", msg)
            }
        })?;

    let _ = state.save_hotkey(&hotkey);
    *state.current_hotkey.lock().await = Some(hotkey);

    Ok(true)
}

#[tauri::command]
pub async fn unregister_hotkey(app: AppHandle, state: State<'_, AppState>) -> Result<(), ApiError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    app.global_shortcut()
        .unregister_all()
        .map_err(|e| ApiError::new("hotkey_unregister_failed", e.to_string()))?;

    state.delete_hotkey();
    *state.current_hotkey.lock().await = None;

    Ok(())
}

#[tauri::command]
pub async fn get_registered_hotkeys(state: State<'_, AppState>) -> Result<Vec<String>, ApiError> {
    let hotkey = state.current_hotkey.lock().await.clone();
    if let Some(h) = hotkey {
        return Ok(vec![h]);
    }
    Ok(state.load_hotkey().map(|h| vec![h]).unwrap_or_default())
}
