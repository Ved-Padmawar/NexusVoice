use super::is_loopback;

#[test]
fn loopback_endpoints_are_flagged() {
    for name in [
        "Stereo Mix (Realtek)",
        "What U Hear",
        "CABLE Output (VB-Audio Virtual Cable)",
        "Line In (Sound Card)",
        "Wave Out Mix",
    ] {
        assert!(is_loopback(name), "should be flagged: {name}");
    }
}

#[test]
fn real_microphones_are_not_flagged() {
    for name in [
        "Microphone (Realtek High Definition Audio)",
        "Headset Microphone",
        "Blue Yeti",
        "Default",
    ] {
        assert!(!is_loopback(name), "should not be flagged: {name}");
    }
}
