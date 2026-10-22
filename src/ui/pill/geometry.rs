//! The pill's drawn sizes and its OS window's, kept in step by hand.

use crate::transcription::controller::PillState;

pub const CAPSULE_H: f32 = 32.0;
pub const CARD_W: f32 = 332.0;
pub const CARD_MIN_H: f32 = 84.0;
pub const CARD_MAX_H: f32 = 186.0;
pub const CARD_RADIUS: f32 = 28.0;
/// The strip below the transcript (42) plus the transcript's padding (19 + 8).
const TEXT_AND_STRIP: f32 = 69.0;

/// Slack so the border and shadow are never clipped by the window edge.
pub const BOTTOM_SLACK: f32 = 6.0;
const SIDE_SLACK: f32 = 4.0;

/// Room to animate in without the window resizing per frame.
const CARD_PAD: f32 = 16.0;

pub const BOTTOM_MARGIN: f32 = 72.0;

pub const fn capsule_width(state: &PillState) -> f32 {
    match state {
        PillState::Recording => 80.0,
        PillState::Processing | PillState::Downloading(_) => 32.0,
        PillState::Idle | PillState::Dictation { .. } | PillState::Error(_) => 104.0,
    }
}

/// Follows the text, between a card-sized floor and a scrolling ceiling.
pub fn card_height(text_height: f32) -> f32 {
    (text_height + TEXT_AND_STRIP).clamp(CARD_MIN_H, CARD_MAX_H)
}

pub const fn capsule_window() -> (f32, f32) {
    (
        capsule_width(&PillState::Idle) + SIDE_SLACK * 2.0,
        CAPSULE_H + BOTTOM_SLACK,
    )
}

pub const fn card_window() -> (f32, f32) {
    (
        CARD_W + CARD_PAD * 2.0,
        CARD_MAX_H + CARD_PAD + BOTTOM_SLACK,
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/pill_geometry.rs"]
mod tests;
