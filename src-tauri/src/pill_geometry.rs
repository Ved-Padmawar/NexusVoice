//! Pill window sizes. Kept in step with the pill element's own box in
//! `PillApp.tsx`, which sizes what is drawn inside these windows.

const CAPSULE_W: f64 = 104.0;
const CAPSULE_H: f64 = 32.0;
const CARD_W: f64 = 332.0;
const CARD_MAX_H: f64 = 186.0;

/// Slack so the border and shadow are never clipped by the window edge.
/// Mirrored by `body { padding-bottom }` in PillApp.css.
const BOTTOM_SLACK: f64 = 6.0;
const SIDE_SLACK: f64 = 4.0;

/// Room to animate in without the window resizing per frame.
const CARD_PAD: f64 = 16.0;

pub const fn capsule_window() -> (f64, f64) {
    (CAPSULE_W + SIDE_SLACK * 2.0, CAPSULE_H + BOTTOM_SLACK)
}

pub const fn card_window() -> (f64, f64) {
    (
        CARD_W + CARD_PAD * 2.0,
        CARD_MAX_H + CARD_PAD + BOTTOM_SLACK,
    )
}
