use super::app_theme::THEMES;
use super::levels::SpeechLevels;
use super::waveform_style::STYLES;
use crate::ui::settings::{ThemeName, WaveformStyle};

#[test]
fn every_choice_is_listed_once_in_the_stored_order() {
    // The swatches index their palettes and waveforms by position, so a new
    // variant without a row would shift every swatch after it.
    assert_eq!(THEMES.map(|(name, ..)| name), ThemeName::ALL);
    assert_eq!(STYLES.map(|(style, ..)| style), WaveformStyle::ALL);
}

#[test]
fn preview_levels_stay_in_range_whatever_the_frame_time() {
    let mut speech = SpeechLevels::new();
    for step in 0..20_000_u32 {
        // Zero, tiny, normal and stalled frames, plus a bogus negative one.
        let dt = [0.0, 0.004, 0.016, 0.5, -1.0][(step % 5) as usize];
        for level in speech.step(dt) {
            assert!((0.0..=1.0).contains(&level), "level {level} at step {step}");
        }
    }
}

#[test]
fn preview_speech_has_both_loud_syllables_and_quiet_breaths() {
    let mut speech = SpeechLevels::new();
    let (mut loudest, mut quietest) = (0.0_f32, 1.0_f32);
    for _ in 0..60 * 60 {
        let peak = speech.step(1.0 / 60.0).into_iter().fold(0.0, f32::max);
        loudest = loudest.max(peak);
        quietest = quietest.min(peak);
    }
    assert!(loudest > 0.4, "speech never got loud: {loudest}");
    assert!(quietest < 0.1, "no breath between phrases: {quietest}");
}
