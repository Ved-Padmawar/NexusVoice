//! Which keys paste the transcript. Pure, so the choice is tested without a
//! window.

use crate::platform::focus::AppCategory;
use crate::settings::PasteMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
}

/// The chord for `mode` into an app of `category`; `None` sends no keys.
/// Terminals read Ctrl+V as a literal keystroke, so `Auto` uses Ctrl+Shift+V there.
pub const fn resolve(mode: PasteMode, category: Option<AppCategory>) -> Option<Chord> {
    match mode {
        PasteMode::Auto => Some(match category {
            Some(AppCategory::Terminal) => Chord::CtrlShiftV,
            _ => Chord::CtrlV,
        }),
        PasteMode::CtrlV => Some(Chord::CtrlV),
        PasteMode::CtrlShiftV => Some(Chord::CtrlShiftV),
        PasteMode::ShiftInsert => Some(Chord::ShiftInsert),
        PasteMode::CopyOnly => None,
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/platform/injection/clipboard/chord.rs"]
mod tests;
