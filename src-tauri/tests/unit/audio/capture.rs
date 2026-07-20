use super::{collapse_devices, InputDevice};

fn names(list: &[InputDevice]) -> Vec<&str> {
    list.iter().map(|d| d.name.as_str()).collect()
}

#[test]
fn dedupes_names_and_marks_default() {
    // ALSA-style duplicate aliases collapse; the OS default is flagged once.
    let raw = vec![
        "Microphone (USB Audio Device)".to_string(),
        "Microphone (USB Audio Device)".to_string(),
        "Headset Microphone".to_string(),
    ];
    let out = collapse_devices(raw, Some("Headset Microphone"));

    assert_eq!(
        names(&out),
        ["Microphone (USB Audio Device)", "Headset Microphone"]
    );
    assert!(!out[0].is_default);
    assert!(out[1].is_default, "the OS default must be marked");
}

#[test]
fn fails_open_to_default_when_empty() {
    // Enumeration returned nothing, but a default exists — surface it so the
    // picker is never empty and recording still works.
    let out = collapse_devices(Vec::new(), Some("Default Input"));
    assert_eq!(names(&out), ["Default Input"]);
    assert!(out[0].is_default);

    // No devices and no default: an empty list is correct.
    assert!(collapse_devices(Vec::new(), None).is_empty());
}
