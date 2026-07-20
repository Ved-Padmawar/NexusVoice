use super::friendly_capture_error;

#[test]
fn permission_denial_is_named() {
    let msg = friendly_capture_error("Access is denied. (os error 5)");
    assert!(msg.to_lowercase().contains("privacy"), "{msg}");
}

#[test]
fn exclusive_use_points_at_other_apps() {
    let msg = friendly_capture_error("device is already in use by another application");
    assert!(
        msg.contains("Teams") || msg.to_lowercase().contains("exclusively"),
        "{msg}"
    );
}

#[test]
fn disconnect_is_named() {
    let msg = friendly_capture_error("the device is no longer available");
    assert!(msg.to_lowercase().contains("disconnected"), "{msg}");
}

#[test]
fn missing_device_is_named() {
    let msg = friendly_capture_error("no input device available");
    assert!(msg.to_lowercase().contains("no microphone"), "{msg}");
}

#[test]
fn unknown_error_passes_through_with_context() {
    let raw = "some bizarre backend failure 0xdeadbeef";
    let msg = friendly_capture_error(raw);
    assert!(msg.contains(raw), "unknown error was swallowed: {msg}");
}

#[test]
fn classification_is_case_insensitive() {
    let msg = friendly_capture_error("ACCESS IS DENIED");
    assert!(msg.to_lowercase().contains("privacy"), "{msg}");
}
