//! Pill window sizes. These must stay in step with `PillApp.tsx`; a window
//! smaller than its content clips the card instead of scrolling it.

use super::{capsule_window, card_window};

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
