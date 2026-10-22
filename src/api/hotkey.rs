//! Global hotkey bindings: validation and persistence. Registering them with
//! the OS is [`crate::platform::hotkeys`]'s job.

use crate::state::{AppState, HotkeyKind};

use super::error::ApiError;

/// The three dictation hotkeys. `None` is unbound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HotkeyBindings {
    pub push_to_talk: Option<String>,
    pub dictation: Option<String>,
    pub dictation_commit: Option<String>,
}

impl HotkeyBindings {
    pub const KINDS: [HotkeyKind; 3] = [
        HotkeyKind::PushToTalk,
        HotkeyKind::Dictation,
        HotkeyKind::DictationCommit,
    ];

    pub fn load(state: &AppState) -> Self {
        Self {
            push_to_talk: state.load_hotkey(HotkeyKind::PushToTalk),
            dictation: state.load_hotkey(HotkeyKind::Dictation),
            dictation_commit: state.load_hotkey(HotkeyKind::DictationCommit),
        }
    }

    pub fn get(&self, kind: HotkeyKind) -> Option<&str> {
        self.slot(kind).as_deref()
    }

    fn slot(&self, kind: HotkeyKind) -> &Option<String> {
        match kind {
            HotkeyKind::PushToTalk => &self.push_to_talk,
            HotkeyKind::Dictation => &self.dictation,
            HotkeyKind::DictationCommit => &self.dictation_commit,
        }
    }

    fn slot_mut(&mut self, kind: HotkeyKind) -> &mut Option<String> {
        match kind {
            HotkeyKind::PushToTalk => &mut self.push_to_talk,
            HotkeyKind::Dictation => &mut self.dictation,
            HotkeyKind::DictationCommit => &mut self.dictation_commit,
        }
    }

    /// Every bound hotkey with its kind.
    pub fn bound(&self) -> impl Iterator<Item = (HotkeyKind, &str)> {
        Self::KINDS
            .into_iter()
            .filter_map(|kind| self.get(kind).map(|hotkey| (kind, hotkey)))
    }

    /// These bindings with `kind` set to `hotkey` (or cleared by `None`),
    /// validated against the other two.
    ///
    /// # Errors
    /// `hotkey_invalid` for a hotkey that could never fire, `hotkey_conflict`
    /// when it is already bound to another action.
    pub fn with(&self, kind: HotkeyKind, hotkey: Option<String>) -> Result<Self, ApiError> {
        let mut next = self.clone();
        let hotkey = hotkey.map(normalize_hotkey).transpose()?;
        if let Some(hotkey) = &hotkey {
            let others = Self::KINDS
                .into_iter()
                .filter(|other| *other != kind)
                .map(|other| self.get(other));
            ensure_no_conflict(hotkey, others)?;
        }
        *next.slot_mut(kind) = hotkey;
        Ok(next)
    }

    /// Persist the binding for `kind`.
    pub fn save(&self, state: &AppState, kind: HotkeyKind) {
        match self.get(kind) {
            Some(hotkey) => {
                if let Err(e) = state.save_hotkey(kind, hotkey) {
                    log::warn!("could not save the {} hotkey: {e}", kind.as_str());
                }
            }
            None => state.delete_hotkey(kind),
        }
    }
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

#[cfg(test)]
#[path = "../../tests/unit/api/hotkey.rs"]
mod tests;
