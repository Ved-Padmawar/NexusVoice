//! Pill window sizes. The window must always be at least as large as the pill
//! drawn inside it; a smaller one clips the card instead of scrolling it.

use super::{
    capsule_width, capsule_window, card_height, card_window, CAPSULE_H, CARD_MAX_H, CARD_MIN_H,
    CARD_W,
};
use crate::transcription::controller::PillState;

#[test]
fn the_card_window_is_larger_than_the_capsule_in_both_axes() {
    // The window is grown once to the card's ceiling; a smaller card window
    // would clip the expanded transcript.
    let (cap_w, cap_h) = capsule_window();
    let (card_w, card_h) = card_window();
    assert!(
        card_w > cap_w,
        "card {card_w} must be wider than capsule {cap_w}"
    );
    assert!(
        card_h > cap_h,
        "card {card_h} must be taller than capsule {cap_h}"
    );
}

#[test]
fn both_windows_have_positive_finite_dimensions() {
    for (label, (w, h)) in [("capsule", capsule_window()), ("card", card_window())] {
        assert!(w.is_finite() && h.is_finite(), "{label} is not finite");
        assert!(w > 0.0 && h > 0.0, "{label} is {w}x{h}");
    }
}

#[test]
fn every_window_leaves_slack_for_the_border_and_shadow() {
    // Without the slack the border is clipped by the window edge.
    let (cap_w, cap_h) = capsule_window();
    assert!(cap_w > 104.0, "no horizontal slack: {cap_w}");
    assert!(cap_h > 32.0, "no vertical slack: {cap_h}");

    let (card_w, card_h) = card_window();
    assert!(card_w > 332.0, "no horizontal slack: {card_w}");
    assert!(card_h > 186.0, "no vertical slack: {card_h}");
}

#[test]
fn the_capsule_window_holds_the_widest_capsule() {
    let widest = [
        PillState::Idle,
        PillState::Recording,
        PillState::Dictation { paused: false },
        PillState::Processing,
        PillState::Downloading(0),
        PillState::Error(String::new()),
    ]
    .iter()
    .map(capsule_width)
    .fold(0.0, f32::max);
    let (width, height) = capsule_window();
    assert!(width > widest && height > CAPSULE_H);
}

#[test]
fn the_card_window_holds_the_largest_card() {
    let (width, height) = card_window();
    assert!(width > CARD_W && height > CARD_MAX_H);
}

#[test]
fn spinner_states_collapse_to_a_circle() {
    assert!((capsule_width(&PillState::Processing) - CAPSULE_H).abs() < f32::EPSILON);
    assert!((capsule_width(&PillState::Downloading(40)) - CAPSULE_H).abs() < f32::EPSILON);
}

#[test]
fn the_card_height_follows_the_text_between_its_bounds() {
    assert!((card_height(0.0) - CARD_MIN_H).abs() < f32::EPSILON);
    assert!((card_height(1000.0) - CARD_MAX_H).abs() < f32::EPSILON);
    // Three 21.87px lines of text, plus the transcript's padding and the strip.
    assert!((card_height(65.61) - 134.61).abs() < 1e-3);
}
