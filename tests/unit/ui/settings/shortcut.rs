//! The recorder's output is saved as-is and parsed by `global-hotkey` at
//! registration. A token it rejects leaves the user with a hotkey that saves
//! fine and then never fires (or fails to register on every launch).

use std::str::FromStr;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use gpui_kit::{Keystroke, Modifiers as Held};

use super::{badge_label, Recording};

/// Modifiers from a space-separated list: `"ctrl shift"`.
fn held(names: &str) -> Held {
    let has = |name| names.split_whitespace().any(|n| n == name);
    Held {
        control: has("ctrl"),
        alt: has("alt"),
        shift: has("shift"),
        platform: has("super"),
        function: false,
    }
}

fn stroke(names: &str, key: &str) -> Keystroke {
    Keystroke {
        modifiers: held(names),
        key: key.to_string(),
        key_char: None,
    }
}

fn shortcut(names: &str, key: &str) -> String {
    let mut recording = Recording::default();
    assert!(recording.press_key(&stroke(names, key)), "{key:?} was refused");
    recording.shortcut().expect("a main key was pressed")
}

#[test]
fn the_recorded_string_is_what_the_legacy_recorder_wrote() {
    assert_eq!(shortcut("ctrl shift", "space"), "Ctrl+Shift+Space");
    assert_eq!(shortcut("alt", "d"), "Alt+D");
    assert_eq!(shortcut("super shift alt ctrl", "k"), "Ctrl+Alt+Shift+Super+K");
}

#[test]
fn modifiers_are_ordered_whatever_order_they_were_pressed_in() {
    let mut recording = Recording::default();
    for pressed in ["super", "super shift", "super shift ctrl", "super shift ctrl alt"] {
        recording.press_modifiers(&held(pressed));
    }
    assert!(recording.press_key(&stroke("super shift ctrl alt", "x")));
    assert_eq!(recording.shortcut().as_deref(), Some("Ctrl+Alt+Shift+Super+X"));
    assert_eq!(
        recording.tokens().collect::<Vec<_>>(),
        ["Super", "Shift", "Ctrl", "Alt", "X"],
        "badges show the order of pressing"
    );
}

#[test]
fn a_recorded_string_parses_to_the_same_keys_in_global_hotkey() {
    assert_eq!(
        HotKey::from_str(&shortcut("ctrl shift", "space")).expect("parses"),
        HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space)
    );
    assert_eq!(
        HotKey::from_str(&shortcut("alt super", "pageup")).expect("parses"),
        HotKey::new(Some(Modifiers::ALT | Modifiers::META), Code::PageUp)
    );
}

#[test]
fn every_key_the_recorder_names_is_accepted_by_global_hotkey() {
    let keys = [
        ("space", Code::Space),
        ("enter", Code::Enter),
        ("tab", Code::Tab),
        ("escape", Code::Escape),
        ("backspace", Code::Backspace),
        ("delete", Code::Delete),
        ("insert", Code::Insert),
        ("home", Code::Home),
        ("end", Code::End),
        ("pageup", Code::PageUp),
        ("pagedown", Code::PageDown),
        ("up", Code::ArrowUp),
        ("down", Code::ArrowDown),
        ("left", Code::ArrowLeft),
        ("right", Code::ArrowRight),
        ("a", Code::KeyA),
        ("z", Code::KeyZ),
        ("0", Code::Digit0),
        ("9", Code::Digit9),
        ("`", Code::Backquote),
        ("\\", Code::Backslash),
        ("[", Code::BracketLeft),
        ("]", Code::BracketRight),
        (",", Code::Comma),
        ("=", Code::Equal),
        ("-", Code::Minus),
        (".", Code::Period),
        ("'", Code::Quote),
        (";", Code::Semicolon),
        ("/", Code::Slash),
    ];
    for (key, code) in keys {
        let text = shortcut("ctrl", key);
        let parsed = HotKey::from_str(&text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
        assert_eq!(parsed, HotKey::new(Some(Modifiers::CONTROL), code), "{text}");
    }
}

#[test]
fn function_keys_parse_up_to_the_last_one_global_hotkey_knows() {
    for number in 1..=35 {
        let text = shortcut("ctrl", &format!("f{number}"));
        assert_eq!(text, format!("Ctrl+F{number}"));
        HotKey::from_str(&text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
    }
    let mut recording = Recording::default();
    for key in ["f0", "f36", "fx", "f"] {
        assert!(!recording.press_key(&stroke("ctrl", key)), "{key} was accepted");
    }
    assert!(recording.is_empty());
}

#[test]
fn a_key_without_a_hotkey_name_is_refused_and_records_nothing() {
    let mut recording = Recording::default();
    for key in ["+", "é", "capslock", "!", "unknown", ""] {
        assert!(!recording.press_key(&stroke("ctrl", key)), "{key:?} was accepted");
    }
    assert!(recording.is_empty(), "a refused key must not leave its modifiers behind");
    assert_eq!(recording.shortcut(), None);
}

#[test]
fn modifiers_alone_make_no_shortcut_but_stay_on_the_badges() {
    let mut recording = Recording::default();
    recording.press_modifiers(&held("ctrl shift"));
    assert_eq!(recording.shortcut(), None);
    assert_eq!(recording.tokens().collect::<Vec<_>>(), ["Ctrl", "Shift"]);

    // Letting go of Shift before the key keeps it, as the legacy recorder did.
    recording.press_modifiers(&held("ctrl"));
    assert!(recording.press_key(&stroke("ctrl", "x")));
    assert_eq!(recording.shortcut().as_deref(), Some("Ctrl+Shift+X"));
}

#[test]
fn pressing_a_second_main_key_replaces_the_first() {
    let mut recording = Recording::default();
    assert!(recording.press_key(&stroke("ctrl", "a")));
    assert!(recording.press_key(&stroke("ctrl", "b")));
    assert_eq!(recording.shortcut().as_deref(), Some("Ctrl+B"));
}

#[test]
fn badges_use_short_labels_but_leave_other_keys_alone() {
    assert_eq!(badge_label("Escape"), "Esc");
    assert_eq!(badge_label("Delete"), "Del");
    assert_eq!(badge_label("ArrowLeft"), "Left");
    assert_eq!(badge_label("Space"), "Space");
    assert_eq!(badge_label("Ctrl"), "Ctrl");
}
