//! The pill's hit test: clicks land on the pill and pass through its margin.

use super::{contains, PillHitbox};

const CAPSULE: PillHitbox = PillHitbox {
    x: 130.5,
    y: 171.0,
    width: 104.0,
    height: 32.0,
};

#[test]
fn a_cursor_over_the_pill_is_caught_at_any_scale() {
    // Window client area at (1000, 500) physical; pill centre in CSS px.
    for scale in [1.0, 1.25, 1.75, 2.0] {
        let cursor = (1000.0 + 182.5 * scale, 500.0 + 187.0 * scale);
        assert!(
            contains(CAPSULE, (1000.0, 500.0), scale, cursor),
            "missed at {scale}x"
        );
    }
}

#[test]
fn a_cursor_in_the_transparent_margin_passes_through() {
    let origin = (1000.0, 500.0);
    // Above the capsule, where the card would grow: the window is there, the pill isn't.
    assert!(!contains(
        CAPSULE,
        origin,
        1.75,
        (1000.0 + 182.5 * 1.75, 500.0 + 100.0 * 1.75)
    ));
    // Just past the right edge.
    assert!(!contains(
        CAPSULE,
        origin,
        1.0,
        (1000.0 + 234.6, 500.0 + 187.0)
    ));
}
