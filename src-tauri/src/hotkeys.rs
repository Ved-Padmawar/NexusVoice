//! Global shortcuts: bound with the OS while the app runs, saved in settings.

use serde::Deserialize;
use specta::Type;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_specta::Event;
use tokio::sync::Mutex;

use crate::events::{
    DictationCommitHotkeyPressed, DictationHotkeyPressed, HotkeyPressed, HotkeyReleased,
};
use crate::settings::{HotkeyBindings, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyKind {
    /// Records while held.
    PushToTalk,
    /// Starts, pauses and resumes hands-free dictation.
    Dictation,
    /// Finishes hands-free dictation.
    DictationCommit,
}

impl HotkeyKind {
    const ALL: [Self; 3] = [Self::PushToTalk, Self::Dictation, Self::DictationCommit];

    fn of(self, bindings: &HotkeyBindings) -> Option<&str> {
        match self {
            Self::PushToTalk => bindings.push_to_talk.as_deref(),
            Self::Dictation => bindings.dictation.as_deref(),
            Self::DictationCommit => bindings.dictation_commit.as_deref(),
        }
    }

    fn slot(self, bindings: &mut HotkeyBindings) -> &mut Option<String> {
        match self {
            Self::PushToTalk => &mut bindings.push_to_talk,
            Self::Dictation => &mut bindings.dictation,
            Self::DictationCommit => &mut bindings.dictation_commit,
        }
    }
}

#[derive(Debug)]
pub enum HotkeyError {
    /// Another role already uses this combination.
    Conflict,
    /// Another application holds the combination.
    InUse,
    PermissionDenied,
    Register(String),
    Unregister(String),
    Save(String),
}

/// Whether `hotkey` is already bound to a role other than `kind`. Case is
/// ignored: the recorder's casing varies with how the key was pressed.
pub fn conflicts(bindings: &HotkeyBindings, kind: HotkeyKind, hotkey: &str) -> bool {
    HotkeyKind::ALL
        .into_iter()
        .filter(|&other| other != kind)
        .filter_map(|other| other.of(bindings))
        .any(|bound| bound.eq_ignore_ascii_case(hotkey))
}

/// The shortcuts bound with the OS right now.
#[derive(Default)]
pub struct Hotkeys {
    bound: Mutex<HotkeyBindings>,
}

impl Hotkeys {
    /// Bind the saved shortcuts at startup.
    pub async fn restore(&self, app: &AppHandle, settings: &Settings) {
        let saved = settings.hotkeys();
        match bind_all(app, &saved) {
            Ok(()) => *self.bound.lock().await = saved,
            Err(e) => log::error!("failed to restore hotkeys: {e:?}"),
        }
    }

    /// Bind `hotkey` to `kind`, or unbind it with `None`. A rejected change
    /// rebinds the previous set, so a failure never drops working hotkeys.
    pub async fn set(
        &self,
        app: &AppHandle,
        settings: &Settings,
        kind: HotkeyKind,
        hotkey: Option<String>,
    ) -> Result<(), HotkeyError> {
        let mut bound = self.bound.lock().await;
        if hotkey
            .as_deref()
            .is_some_and(|h| conflicts(&bound, kind, h))
        {
            return Err(HotkeyError::Conflict);
        }

        let mut next = bound.clone();
        *kind.slot(&mut next) = hotkey;
        if let Err(e) = bind_all(app, &next) {
            let _ = bind_all(app, &bound);
            return Err(e);
        }
        *bound = next.clone();
        settings
            .set_hotkeys(next)
            .map_err(|e| HotkeyError::Save(e.to_string()))
    }
}

/// Clear all shortcuts, then bind each requested one. If any binding fails,
/// every shortcut bound in this call is unregistered, so a partial set is
/// never left live.
fn bind_all(app: &AppHandle, bindings: &HotkeyBindings) -> Result<(), HotkeyError> {
    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|e| HotkeyError::Unregister(e.to_string()))?;

    let mut done = Vec::new();
    for kind in HotkeyKind::ALL {
        let Some(hotkey) = kind.of(bindings) else {
            continue;
        };
        if let Err(e) = bind(app, hotkey, kind) {
            for hotkey in done {
                let _ = shortcuts.unregister(hotkey);
            }
            return Err(e);
        }
        done.push(hotkey);
    }
    Ok(())
}

/// Push-to-talk reports press and release; the dictation keys only press.
fn bind(app: &AppHandle, hotkey: &str, kind: HotkeyKind) -> Result<(), HotkeyError> {
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey, move |_app, _shortcut, event| {
            let pressed = event.state == ShortcutState::Pressed;
            let _ = match kind {
                HotkeyKind::PushToTalk if pressed => HotkeyPressed.emit(&handle),
                HotkeyKind::PushToTalk => HotkeyReleased.emit(&handle),
                HotkeyKind::Dictation if pressed => DictationHotkeyPressed.emit(&handle),
                HotkeyKind::DictationCommit if pressed => {
                    DictationCommitHotkeyPressed.emit(&handle)
                }
                HotkeyKind::Dictation | HotkeyKind::DictationCommit => Ok(()),
            };
        })
        .map_err(|e| register_error(&e))
}

fn register_error(error: &tauri_plugin_global_shortcut::Error) -> HotkeyError {
    let msg = error.to_string();
    if msg.contains("already registered") || msg.contains("already in use") {
        HotkeyError::InUse
    } else if msg.contains("permission") || msg.contains("access") {
        HotkeyError::PermissionDenied
    } else {
        HotkeyError::Register(msg)
    }
}

#[cfg(test)]
#[path = "../tests/unit/hotkeys.rs"]
mod tests;
