//! Global hotkey commands: register, unregister, and query active hotkeys.

use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;

use super::error::ApiError;

#[derive(serde::Serialize)]
pub struct RegisteredHotkeys {
    pub ptt: Vec<String>,
    pub dictation: Vec<String>,
    pub dictation_commit: Vec<String>,
}

#[tauri::command]
pub async fn register_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<bool, ApiError> {
    let hotkey = normalize_hotkey(hotkey)?;
    let dictation_hotkey = state.current_dictation_hotkey.lock().await.clone();
    let dictation_commit_hotkey = state.current_dictation_commit_hotkey.lock().await.clone();
    ensure_no_conflict(
        &hotkey,
        [
            dictation_hotkey.as_deref(),
            dictation_commit_hotkey.as_deref(),
        ],
    )?;

    if let Err(e) = register_shortcuts(
        &app,
        Some(hotkey.clone()),
        dictation_hotkey.clone(),
        dictation_commit_hotkey.clone(),
    ) {
        // Restore the previously-working set so a rejected change never drops live hotkeys.
        let prev_ptt = state.current_hotkey.lock().await.clone();
        let _ = register_shortcuts(&app, prev_ptt, dictation_hotkey, dictation_commit_hotkey);
        return Err(e);
    }
    let _ = state.save_hotkey(&hotkey);
    *state.current_hotkey.lock().await = Some(hotkey);

    Ok(true)
}

#[tauri::command]
pub async fn unregister_hotkey(app: AppHandle, state: State<'_, AppState>) -> Result<(), ApiError> {
    let prev_ptt = state.current_hotkey.lock().await.clone();
    let dictation_hotkey = state.current_dictation_hotkey.lock().await.clone();
    let dictation_commit_hotkey = state.current_dictation_commit_hotkey.lock().await.clone();
    if let Err(e) = register_shortcuts(
        &app,
        None,
        dictation_hotkey.clone(),
        dictation_commit_hotkey.clone(),
    ) {
        let _ = register_shortcuts(&app, prev_ptt, dictation_hotkey, dictation_commit_hotkey);
        return Err(e);
    }

    state.delete_hotkey();
    *state.current_hotkey.lock().await = None;

    Ok(())
}

#[tauri::command]
pub async fn register_dictation_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<bool, ApiError> {
    let hotkey = normalize_hotkey(hotkey)?;
    let ptt_hotkey = state.current_hotkey.lock().await.clone();
    let dictation_commit_hotkey = state.current_dictation_commit_hotkey.lock().await.clone();
    ensure_no_conflict(
        &hotkey,
        [ptt_hotkey.as_deref(), dictation_commit_hotkey.as_deref()],
    )?;

    if let Err(e) = register_shortcuts(
        &app,
        ptt_hotkey.clone(),
        Some(hotkey.clone()),
        dictation_commit_hotkey.clone(),
    ) {
        let prev_dictation = state.current_dictation_hotkey.lock().await.clone();
        let _ = register_shortcuts(&app, ptt_hotkey, prev_dictation, dictation_commit_hotkey);
        return Err(e);
    }
    let _ = state.save_dictation_hotkey(&hotkey);
    *state.current_dictation_hotkey.lock().await = Some(hotkey);

    Ok(true)
}

#[tauri::command]
pub async fn unregister_dictation_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    let ptt_hotkey = state.current_hotkey.lock().await.clone();
    let prev_dictation = state.current_dictation_hotkey.lock().await.clone();
    let dictation_commit_hotkey = state.current_dictation_commit_hotkey.lock().await.clone();
    if let Err(e) = register_shortcuts(&app, ptt_hotkey.clone(), None, dictation_commit_hotkey.clone())
    {
        let _ = register_shortcuts(&app, ptt_hotkey, prev_dictation, dictation_commit_hotkey);
        return Err(e);
    }

    state.delete_dictation_hotkey();
    *state.current_dictation_hotkey.lock().await = None;

    Ok(())
}

#[tauri::command]
pub async fn register_dictation_commit_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<bool, ApiError> {
    let hotkey = normalize_hotkey(hotkey)?;
    let ptt_hotkey = state.current_hotkey.lock().await.clone();
    let dictation_hotkey = state.current_dictation_hotkey.lock().await.clone();
    ensure_no_conflict(
        &hotkey,
        [ptt_hotkey.as_deref(), dictation_hotkey.as_deref()],
    )?;

    if let Err(e) =
        register_shortcuts(&app, ptt_hotkey.clone(), dictation_hotkey.clone(), Some(hotkey.clone()))
    {
        let prev_commit = state.current_dictation_commit_hotkey.lock().await.clone();
        let _ = register_shortcuts(&app, ptt_hotkey, dictation_hotkey, prev_commit);
        return Err(e);
    }
    let _ = state.save_dictation_commit_hotkey(&hotkey);
    *state.current_dictation_commit_hotkey.lock().await = Some(hotkey);

    Ok(true)
}

#[tauri::command]
pub async fn unregister_dictation_commit_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    let ptt_hotkey = state.current_hotkey.lock().await.clone();
    let dictation_hotkey = state.current_dictation_hotkey.lock().await.clone();
    let prev_commit = state.current_dictation_commit_hotkey.lock().await.clone();
    if let Err(e) = register_shortcuts(&app, ptt_hotkey.clone(), dictation_hotkey.clone(), None) {
        let _ = register_shortcuts(&app, ptt_hotkey, dictation_hotkey, prev_commit);
        return Err(e);
    }

    state.delete_dictation_commit_hotkey();
    *state.current_dictation_commit_hotkey.lock().await = None;

    Ok(())
}

#[tauri::command]
pub async fn get_registered_hotkeys(
    state: State<'_, AppState>,
) -> Result<RegisteredHotkeys, ApiError> {
    let ptt_hotkey = state
        .current_hotkey
        .lock()
        .await
        .clone()
        .or_else(|| state.load_hotkey());
    let dictation_hotkey = state
        .current_dictation_hotkey
        .lock()
        .await
        .clone()
        .or_else(|| state.load_dictation_hotkey());
    let dictation_commit_hotkey = state
        .current_dictation_commit_hotkey
        .lock()
        .await
        .clone()
        .or_else(|| state.load_dictation_commit_hotkey());

    let mut ptt = Vec::new();
    if let Some(hotkey) = ptt_hotkey {
        ptt.push(hotkey);
    }
    let mut dictation = Vec::new();
    if let Some(hotkey) = dictation_hotkey {
        dictation.push(hotkey);
    }
    let mut dictation_commit = Vec::new();
    if let Some(hotkey) = dictation_commit_hotkey {
        dictation_commit.push(hotkey);
    }
    Ok(RegisteredHotkeys {
        ptt,
        dictation,
        dictation_commit,
    })
}

pub async fn restore_registered_hotkeys(app: &AppHandle) {
    let state = app.state::<AppState>();
    let ptt_hotkey = state.load_hotkey();
    let dictation_hotkey = state.load_dictation_hotkey();
    let dictation_commit_hotkey = state.load_dictation_commit_hotkey();

    if let Err(e) = register_shortcuts(
        app,
        ptt_hotkey.clone(),
        dictation_hotkey.clone(),
        dictation_commit_hotkey.clone(),
    ) {
        log::error!("failed to restore hotkeys: {e:?}");
        return;
    }

    *state.current_hotkey.lock().await = ptt_hotkey;
    *state.current_dictation_hotkey.lock().await = dictation_hotkey;
    *state.current_dictation_commit_hotkey.lock().await = dictation_commit_hotkey;
}

fn normalize_hotkey(hotkey: String) -> Result<String, ApiError> {
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

    Ok(hotkey)
}

fn ensure_no_conflict<'a>(
    hotkey: &str,
    existing: impl IntoIterator<Item = Option<&'a str>>,
) -> Result<(), ApiError> {
    if existing
        .into_iter()
        .flatten()
        .any(|other| other.eq_ignore_ascii_case(hotkey))
    {
        return Err(ApiError::new(
            "hotkey_conflict",
            "hotkeys must be different",
        ));
    }
    Ok(())
}

/// Which dictation event a bound shortcut should emit on press.
#[derive(Clone, Copy)]
enum ShortcutKind {
    /// Push-to-talk: emits pressed on key-down and released on key-up.
    PushToTalk,
    /// Dictation toggle: emits only on key-down.
    Dictation,
    /// Dictation commit: emits only on key-down.
    DictationCommit,
}

/// Bind a single global shortcut to its event emitter. Does **not** unregister
/// anything first — the caller owns clearing/rollback.
fn bind_shortcut(app: &AppHandle, hotkey: &str, kind: ShortcutKind) -> Result<(), ApiError> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    let app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey, move |_app, _shortcut, event| match kind {
            ShortcutKind::PushToTalk => {
                if event.state == ShortcutState::Pressed {
                    let _ = app_clone.emit("hotkey-pressed", ());
                } else {
                    let _ = app_clone.emit("hotkey-released", ());
                }
            }
            ShortcutKind::Dictation => {
                if event.state == ShortcutState::Pressed {
                    let _ = app_clone.emit("dictation-hotkey-pressed", ());
                }
            }
            ShortcutKind::DictationCommit => {
                if event.state == ShortcutState::Pressed {
                    let _ = app_clone.emit("dictation-commit-hotkey-pressed", ());
                }
            }
        })
        .map_err(map_register_error)
}

/// Clear all shortcuts, then bind each requested one. If any binding fails,
/// every shortcut bound in this call is unregistered so we never leave a
/// partial set live (e.g. a stale PTT hotkey after a failed dictation bind).
fn bind_all(app: &AppHandle, bindings: &[(String, ShortcutKind)]) -> Result<(), ApiError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    app.global_shortcut()
        .unregister_all()
        .map_err(|e| ApiError::new("hotkey_unregister_failed", e.to_string()))?;

    for (i, (hotkey, kind)) in bindings.iter().enumerate() {
        if let Err(e) = bind_shortcut(app, hotkey, *kind) {
            // Roll back the ones bound so far in this call.
            for (done, _) in &bindings[..i] {
                let _ = app.global_shortcut().unregister(done.as_str());
            }
            return Err(e);
        }
    }

    Ok(())
}

fn register_shortcuts(
    app: &AppHandle,
    ptt_hotkey: Option<String>,
    dictation_hotkey: Option<String>,
    dictation_commit_hotkey: Option<String>,
) -> Result<(), ApiError> {
    let mut bindings = Vec::with_capacity(3);
    if let Some(hotkey) = ptt_hotkey {
        bindings.push((hotkey, ShortcutKind::PushToTalk));
    }
    if let Some(hotkey) = dictation_hotkey {
        bindings.push((hotkey, ShortcutKind::Dictation));
    }
    if let Some(hotkey) = dictation_commit_hotkey {
        bindings.push((hotkey, ShortcutKind::DictationCommit));
    }

    bind_all(app, &bindings)
}

fn map_register_error(error: tauri_plugin_global_shortcut::Error) -> ApiError {
    let msg = error.to_string();
    if msg.contains("already registered") || msg.contains("already in use") {
        ApiError::new(
            "hotkey_already_in_use",
            "this hotkey is already in use by another application",
        )
    } else if msg.contains("permission") || msg.contains("access") {
        ApiError::new(
            "hotkey_permission_denied",
            "OS denied hotkey registration - try a different combination",
        )
    } else {
        ApiError::new("hotkey_register_failed", msg)
    }
}
