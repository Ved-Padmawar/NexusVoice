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
    let json = serde_json::to_value(&err).expect("serialize");
    assert_eq!(json["code"], "hotkey_invalid");
    assert!(
        json["message"]
            .as_str()
            .expect("message")
            .contains("modifier"),
        "{json}"
    );
}

#[test]
fn a_hotkey_that_matches_another_is_rejected() {
    let err = ensure_no_conflict("Ctrl+Shift+D", [Some("Ctrl+Shift+D")]).expect_err("conflict");
    let json = serde_json::to_value(&err).expect("serialize");
    assert_eq!(json["code"], "hotkey_conflict");
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
