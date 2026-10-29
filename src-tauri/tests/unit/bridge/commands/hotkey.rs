//! Hotkey validation. A hotkey that registers but cannot fire breaks the app's
//! primary input with no error anywhere.

use super::normalize_hotkey;

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
