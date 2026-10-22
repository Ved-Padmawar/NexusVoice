//! System-wide hotkeys, created and used on the UI thread (Windows delivers
//! their messages there). Wayland users bind `--toggle-dictation` instead.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{PoisonError, RwLock};

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::api::{ApiError, HotkeyBindings};
use crate::state::{self, HotkeyKind};
use crate::transcription::controller::{self, Input};

static ACTIONS: RwLock<Option<HashMap<u32, HotkeyKind>>> = RwLock::new(None);

pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    bindings: HotkeyBindings,
}

impl Hotkeys {
    /// A saved hotkey that no longer registers is logged and left unbound.
    ///
    /// # Errors
    /// When the platform offers no global hotkeys at all.
    pub fn new() -> Result<Self, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        GlobalHotKeyEvent::set_event_handler(Some(on_event));
        let mut hotkeys = Self {
            manager,
            registered: Vec::new(),
            bindings: HotkeyBindings::default(),
        };
        if let Err(e) = hotkeys.apply(HotkeyBindings::load(state::get())) {
            log::error!("failed to restore hotkeys: {e}");
        }
        Ok(hotkeys)
    }

    pub fn bindings(&self) -> &HotkeyBindings {
        &self.bindings
    }

    /// Bind or unbind `kind` and save; a rejected change keeps working hotkeys.
    ///
    /// # Errors
    /// The hotkey is invalid, duplicates another, or the OS refused it.
    pub fn set(&mut self, kind: HotkeyKind, hotkey: Option<&str>) -> Result<(), ApiError> {
        let next = self.bindings.with(kind, hotkey)?;
        if let Err(e) = self.apply(next) {
            let previous = self.bindings.clone();
            let _ = self.apply(previous);
            return Err(e);
        }
        self.bindings.save(state::get(), kind);
        Ok(())
    }

    /// Replace every registration, all or nothing.
    fn apply(&mut self, bindings: HotkeyBindings) -> Result<(), ApiError> {
        let _ = self.manager.unregister_all(&self.registered);
        self.registered.clear();
        let mut actions = HashMap::new();

        for (kind, text) in bindings.bound() {
            let registered = HotKey::from_str(text)
                .map_err(|e| ApiError::new("hotkey_invalid", e.to_string()))
                .and_then(|hotkey| {
                    self.manager
                        .register(hotkey)
                        .map(|()| hotkey)
                        .map_err(|e| register_error(&e))
                });
            match registered {
                Ok(hotkey) => {
                    actions.insert(hotkey.id(), kind);
                    self.registered.push(hotkey);
                }
                Err(e) => {
                    let _ = self.manager.unregister_all(&self.registered);
                    self.registered.clear();
                    *ACTIONS.write().unwrap_or_else(PoisonError::into_inner) = None;
                    return Err(e);
                }
            }
        }

        *ACTIONS.write().unwrap_or_else(PoisonError::into_inner) = Some(actions);
        self.bindings = bindings;
        Ok(())
    }
}

fn on_event(event: GlobalHotKeyEvent) {
    let kind = ACTIONS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|actions| actions.get(&event.id).copied());
    let input = match (kind, event.state) {
        (Some(HotkeyKind::PushToTalk), HotKeyState::Pressed) => Input::PushToTalkPressed,
        (Some(HotkeyKind::PushToTalk), HotKeyState::Released) => Input::PushToTalkReleased,
        (Some(HotkeyKind::Dictation), HotKeyState::Pressed) => Input::DictationToggle,
        (Some(HotkeyKind::DictationCommit), HotKeyState::Pressed) => Input::DictationCommit,
        _ => return,
    };
    controller::send(input);
}

fn register_error(error: &global_hotkey::Error) -> ApiError {
    let msg = error.to_string();
    if matches!(error, global_hotkey::Error::AlreadyRegistered(_))
        || msg.contains("already registered")
        || msg.contains("already in use")
    {
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
