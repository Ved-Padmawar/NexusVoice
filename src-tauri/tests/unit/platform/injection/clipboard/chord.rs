use super::{resolve, Chord};
use crate::platform::focus::AppCategory;
use crate::settings::PasteMode;

#[test]
fn auto_pastes_into_a_terminal_with_shift_and_a_fixed_mode_ignores_the_app() {
    let terminal = Some(AppCategory::Terminal);
    assert_eq!(resolve(PasteMode::Auto, terminal), Some(Chord::CtrlShiftV));
    assert_eq!(
        resolve(PasteMode::Auto, Some(AppCategory::Code)),
        Some(Chord::CtrlV)
    );
    assert_eq!(resolve(PasteMode::Auto, None), Some(Chord::CtrlV));
    assert_eq!(
        resolve(PasteMode::ShiftInsert, terminal),
        Some(Chord::ShiftInsert)
    );
    assert_eq!(resolve(PasteMode::CopyOnly, terminal), None);
}
