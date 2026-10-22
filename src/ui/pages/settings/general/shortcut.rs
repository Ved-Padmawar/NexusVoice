//! Turning the keys held in the hotkey recorder into the string the backend
//! stores, e.g. `Ctrl+Shift+Space`. `global-hotkey` parses that string when the
//! hotkey is registered, so every token here must be one it accepts.

use gpui_kit::{Keystroke, Modifiers};

/// Held modifiers in the order shortcuts are written.
const MODIFIERS: [Modifier; 4] = [Modifier::Ctrl, Modifier::Alt, Modifier::Shift, Modifier::Super];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

impl Modifier {
    fn token(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
            Self::Super => "Super",
        }
    }

    fn is_held(self, held: &Modifiers) -> bool {
        match self {
            Self::Ctrl => held.control,
            Self::Alt => held.alt,
            Self::Shift => held.shift,
            Self::Super => held.platform,
        }
    }
}

/// The keys pressed since recording began, in the order they went down.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recording {
    modifiers: Vec<Modifier>,
    key: Option<String>,
}

impl Recording {
    pub fn is_empty(&self) -> bool {
        self.modifiers.is_empty() && self.key.is_none()
    }

    /// Note every modifier now held; one let go stays recorded.
    pub fn press_modifiers(&mut self, held: &Modifiers) {
        for modifier in MODIFIERS {
            if modifier.is_held(held) && !self.modifiers.contains(&modifier) {
                self.modifiers.push(modifier);
            }
        }
    }

    /// Whether a recorded modifier is no longer held: the user let go, which
    /// ends the recording.
    pub fn modifier_released(&self, held: &Modifiers) -> bool {
        self.modifiers.iter().any(|m| !m.is_held(held))
    }

    /// Record a main key along with the modifiers held with it. Returns
    /// `false` for a key a hotkey cannot be bound to.
    pub fn press_key(&mut self, keystroke: &Keystroke) -> bool {
        let Some(token) = key_token(&keystroke.key) else {
            return false;
        };
        self.press_modifiers(&keystroke.modifiers);
        self.key = Some(token);
        true
    }

    /// The hotkey string, or `None` until a main key was pressed.
    pub fn shortcut(&self) -> Option<String> {
        let key = self.key.as_deref()?;
        let mut parts: Vec<&str> = MODIFIERS
            .into_iter()
            .filter(|m| self.modifiers.contains(m))
            .map(Modifier::token)
            .collect();
        parts.push(key);
        Some(parts.join("+"))
    }

    /// Badge tokens in the order the keys were pressed.
    pub fn tokens(&self) -> impl Iterator<Item = &str> {
        self.modifiers
            .iter()
            .map(|m| m.token())
            .chain(self.key.as_deref())
    }
}

/// The `global-hotkey` name of a GPUI key, or `None` where it has none.
fn key_token(key: &str) -> Option<String> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return single_char_token(c);
    }
    let named = match key {
        "space" => "Space",
        "enter" => "Enter",
        "tab" => "Tab",
        "escape" => "Escape",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "insert" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        _ => return function_key(key),
    };
    Some(named.to_string())
}

fn single_char_token(c: char) -> Option<String> {
    match c {
        c if c.is_ascii_alphanumeric() => Some(c.to_ascii_uppercase().to_string()),
        // `+` separates tokens, so it cannot name a key.
        '`' | '\\' | '[' | ']' | ',' | '=' | '-' | '.' | '\'' | ';' | '/' => Some(c.to_string()),
        _ => None,
    }
}

/// `f1`..`f35`, the range `global-hotkey` knows.
fn function_key(key: &str) -> Option<String> {
    let number: u8 = key.strip_prefix('f')?.parse().ok()?;
    (1..=35).contains(&number).then(|| format!("F{number}"))
}

/// How a stored token is labelled on a key badge.
pub fn badge_label(token: &str) -> &str {
    match token {
        "Super" if cfg!(target_os = "macos") => "Cmd",
        "Super" => "Win",
        "Delete" => "Del",
        "Escape" => "Esc",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        other => other,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/ui/settings/shortcut.rs"]
mod tests;
