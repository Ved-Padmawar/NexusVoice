//! A hotkey that shadows another registers twice and one of them never fires.

use super::{bind_available, conflicts, HotkeyError, HotkeyKind};
use crate::settings::HotkeyBindings;

fn bindings(
    push_to_talk: Option<&str>,
    dictation: Option<&str>,
    commit: Option<&str>,
) -> HotkeyBindings {
    HotkeyBindings {
        push_to_talk: push_to_talk.map(str::to_string),
        dictation: dictation.map(str::to_string),
        dictation_commit: commit.map(str::to_string),
    }
}

#[test]
fn a_hotkey_bound_to_another_role_conflicts() {
    let bound = bindings(Some("Ctrl+Shift+D"), None, None);
    assert!(conflicts(&bound, HotkeyKind::Dictation, "Ctrl+Shift+D"));
}

#[test]
fn conflict_detection_ignores_case() {
    let bound = bindings(Some("Ctrl+Shift+D"), None, None);
    assert!(conflicts(&bound, HotkeyKind::Dictation, "ctrl+shift+d"));
}

#[test]
fn rebinding_a_role_to_its_own_hotkey_is_no_conflict() {
    let bound = bindings(Some("Ctrl+Shift+D"), None, None);
    assert!(!conflicts(&bound, HotkeyKind::PushToTalk, "Ctrl+Shift+D"));
}

#[test]
fn unbound_roles_never_conflict() {
    let bound = HotkeyBindings::default();
    assert!(!conflicts(&bound, HotkeyKind::PushToTalk, "Ctrl+Shift+D"));
}

#[test]
fn one_shortcut_in_use_elsewhere_leaves_the_others_bound() {
    let saved = bindings(Some("Ctrl+A"), Some("Ctrl+B"), Some("Ctrl+C"));
    let bound = bind_available(&saved, |_, hotkey| match hotkey {
        "Ctrl+B" => Err(HotkeyError::InUse),
        _ => Ok(()),
    });
    assert_eq!(bound, bindings(Some("Ctrl+A"), None, Some("Ctrl+C")));
}

#[test]
fn unset_roles_are_never_bound() {
    let saved = bindings(Some("Ctrl+A"), None, None);
    let mut attempts = Vec::new();
    let bound = bind_available(&saved, |kind, _| {
        attempts.push(kind);
        Ok(())
    });
    assert_eq!(attempts, [HotkeyKind::PushToTalk]);
    assert_eq!(bound, saved);
}

#[test]
fn every_other_role_is_checked() {
    let bound = bindings(Some("Ctrl+Shift+D"), None, Some("Alt+Space"));
    assert!(!conflicts(&bound, HotkeyKind::Dictation, "Ctrl+Shift+X"));
    assert!(conflicts(&bound, HotkeyKind::Dictation, "Alt+Space"));
}
