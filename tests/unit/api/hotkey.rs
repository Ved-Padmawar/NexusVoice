//! Hotkey validation. A hotkey that registers but cannot fire, or one that
//! shadows another, breaks the app's primary input with no error anywhere.

use super::{ensure_no_conflict, normalize_hotkey};

#[test]
fn a_modifier_plus_a_key_is_accepted() {
    assert_eq!(
        normalize_hotkey("Ctrl+Shift+D".to_string()).expect("valid"),
        "Ctrl+Shift+D"
    );
    assert_eq!(
        normalize_hotkey("Alt+Space".to_string()).expect("valid"),
        "Alt+Space"
    );
    assert_eq!(
        normalize_hotkey("Super+K".to_string()).expect("valid"),
        "Super+K"
    );
}

#[test]
fn surrounding_whitespace_is_trimmed() {
    assert_eq!(
        normalize_hotkey("  Ctrl+Shift+D \n".to_string()).expect("valid"),
        "Ctrl+Shift+D"
    );
}

#[test]
fn an_empty_hotkey_is_rejected() {
    for raw in ["", "   ", "\n\t"] {
        assert!(
            normalize_hotkey(raw.to_string()).is_err(),
            "{raw:?} should be rejected"
        );
    }
}

#[test]
fn a_bare_key_with_no_modifier_is_rejected() {
    // A global shortcut on a bare letter would swallow that key everywhere.
    assert!(normalize_hotkey("D".to_string()).is_err());
    assert!(normalize_hotkey("Space".to_string()).is_err());
}

#[test]
fn modifiers_with_no_key_are_rejected() {
    // Nothing to trigger on — it would register and never fire.
    assert!(normalize_hotkey("Ctrl".to_string()).is_err());
    assert!(normalize_hotkey("Ctrl+Shift".to_string()).is_err());
    assert!(normalize_hotkey("Ctrl+Alt+Shift+Super".to_string()).is_err());
}

#[test]
fn rejection_carries_the_invalid_code_the_frontend_matches_on() {
    let err = normalize_hotkey("Ctrl".to_string()).expect_err("rejected");
    assert_eq!(err.code(), "hotkey_invalid");
    assert!(err.message().contains("modifier"), "{err}");
}

#[test]
fn a_hotkey_that_matches_another_is_rejected() {
    let err = ensure_no_conflict("Ctrl+Shift+D", [Some("Ctrl+Shift+D")]).expect_err("conflict");
    assert_eq!(err.code(), "hotkey_conflict");
}

#[test]
fn conflict_detection_ignores_case() {
    // The recorder's casing varies with how the key was pressed; two hotkeys
    // differing only in case would both register and one would never fire.
    assert!(ensure_no_conflict("ctrl+shift+d", [Some("Ctrl+Shift+D")]).is_err());
}

#[test]
fn unset_hotkeys_never_conflict() {
    // `None` is "not bound", which must not collide with anything.
    assert!(ensure_no_conflict("Ctrl+Shift+D", [None, None]).is_ok());
    assert!(ensure_no_conflict("Ctrl+Shift+D", []).is_ok());
}

#[test]
fn a_distinct_hotkey_is_allowed_alongside_the_others() {
    // All three dictation hotkeys are checked against each other at once.
    assert!(ensure_no_conflict(
        "Ctrl+Shift+X",
        [Some("Ctrl+Shift+D"), None, Some("Alt+Space")]
    )
    .is_ok());

    // …and the third slot is checked too, not just the first.
    assert!(
        ensure_no_conflict("Alt+Space", [Some("Ctrl+Shift+D"), None, Some("Alt+Space")]).is_err()
    );
}

mod bindings {
    use crate::api::HotkeyBindings;
    use crate::state::HotkeyKind;

    fn bound() -> HotkeyBindings {
        HotkeyBindings {
            push_to_talk: Some("Ctrl+Space".to_string()),
            dictation: Some("Ctrl+Shift+D".to_string()),
            dictation_commit: None,
        }
    }

    #[test]
    fn setting_a_hotkey_changes_only_its_own_slot() {
        let next = bound()
            .with(
                HotkeyKind::DictationCommit,
                Some(" Ctrl+Enter ".to_string()),
            )
            .expect("valid");
        assert_eq!(next.get(HotkeyKind::DictationCommit), Some("Ctrl+Enter"));
        assert_eq!(next.get(HotkeyKind::PushToTalk), Some("Ctrl+Space"));
        assert_eq!(next.get(HotkeyKind::Dictation), Some("Ctrl+Shift+D"));
    }

    #[test]
    fn a_hotkey_may_be_rebound_to_itself_but_not_to_another_action() {
        // Saving an unchanged recorder must not report a conflict with itself.
        assert!(bound()
            .with(HotkeyKind::PushToTalk, Some("Ctrl+Space".to_string()))
            .is_ok());
        let err = bound()
            .with(HotkeyKind::DictationCommit, Some("ctrl+space".to_string()))
            .expect_err("taken by push-to-talk");
        assert_eq!(err.code(), "hotkey_conflict");
    }

    #[test]
    fn clearing_unbinds_and_drops_it_from_the_registration_set() {
        let next = bound()
            .with(HotkeyKind::Dictation, None)
            .expect("clearing is valid");
        assert_eq!(next.get(HotkeyKind::Dictation), None);
        let registered: Vec<_> = next.bound().map(|(kind, _)| kind).collect();
        assert_eq!(registered, [HotkeyKind::PushToTalk]);
    }

    #[test]
    fn a_rejected_change_leaves_the_bindings_untouched() {
        let before = bound();
        assert!(before
            .with(HotkeyKind::Dictation, Some("D".to_string()))
            .is_err());
        assert_eq!(before, bound());
    }
}
