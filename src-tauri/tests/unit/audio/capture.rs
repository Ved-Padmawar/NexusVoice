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

// ── Sample conversion ──────────────────────────────────────────────────
// cpal hands us whatever format the device uses. A wrong conversion does not
// fail — it quietly distorts the audio and degrades every transcription.

use super::ToF32;

#[test]
fn f32_samples_pass_through_unchanged() {
    assert_eq!(0.5f32.to_f32(), 0.5);
    assert_eq!((-0.25f32).to_f32(), -0.25);
    assert_eq!(0.0f32.to_f32(), 0.0);
}

#[test]
fn i16_samples_scale_to_plus_minus_one() {
    assert_eq!(0i16.to_f32(), 0.0, "signed silence is zero");
    assert_eq!(i16::MAX.to_f32(), 1.0);
    assert!((0.5 - 16_383i16.to_f32()).abs() < 1e-3);
    // i16::MIN is one step past -1.0 by construction; it must not wrap positive.
    assert!(i16::MIN.to_f32() < 0.0);
    assert!((i16::MIN.to_f32() + 1.0).abs() < 1e-3);
}

#[test]
fn u16_samples_are_centred_before_scaling() {
    // Unsigned PCM puts silence at the midpoint, not at 0. Skipping the shift
    // would make every recording a full-scale DC offset.
    assert!(
        (u16::MAX / 2).to_f32().abs() < 1e-3,
        "unsigned silence must map to ~0, got {}",
        (u16::MAX / 2).to_f32()
    );
    assert_eq!(u16::MAX.to_f32(), 1.0);
    assert_eq!(0u16.to_f32(), -1.0);
}

#[test]
fn every_sample_format_stays_within_range() {
    // i16::MIN is the one exception: dividing by i16::MAX puts it a single step
    // past -1.0 (-1.0000305). `preprocess::to_16k` clamps it, so the overshoot
    // never reaches the model — but it must stay within that one step, and
    // must never wrap to a positive value.
    for raw in [0i16, 1, -1, i16::MAX, i16::MIN, 12_345, -12_345] {
        let v = raw.to_f32();
        assert!((-1.001..=1.0).contains(&v), "i16 {raw} -> {v}");
        assert_eq!(v < 0.0, raw < 0, "i16 {raw} flipped sign to {v}");
    }
    for raw in [0u16, 1, u16::MAX, u16::MAX / 2, 40_000] {
        let v = raw.to_f32();
        assert!((-1.0..=1.0).contains(&v), "u16 {raw} -> {v}");
    }
}
