//! `ui.json` outlives every release: a renamed field or variant silently
//! resets the user's theme and choices to defaults.

use super::{PillTheme, Route, SettingsTab, ThemeName, UiSettings, WaveformStyle};

#[test]
fn settings_round_trip_through_their_file_format() {
    let settings = UiSettings {
        theme: ThemeName::Blossom,
        pill_theme: PillTheme::Dawn,
        waveform_style: WaveformStyle::Spectrum,
        live_transcript: true,
        route: Route::Settings,
        settings_tab: SettingsTab::About,
        model_chosen: true,
        cuda_offer_seen: true,
    };
    let json = serde_json::to_string(&settings).expect("serialize");
    assert_eq!(
        serde_json::from_str::<UiSettings>(&json).expect("parse"),
        settings
    );
}

#[test]
fn the_file_uses_the_names_the_app_has_always_written() {
    let json = serde_json::to_value(UiSettings {
        theme: ThemeName::Abyss,
        pill_theme: PillTheme::Steel,
        waveform_style: WaveformStyle::Eq,
        settings_tab: SettingsTab::Voice,
        ..UiSettings::default()
    })
    .expect("serialize");
    assert_eq!(json["theme"], "abyss");
    assert_eq!(json["pillTheme"], "steel");
    assert_eq!(json["waveformStyle"], "eq");
    assert_eq!(json["settingsTab"], "voice");
    assert_eq!(json["modelChosen"], false);
}

#[test]
fn a_partial_or_older_file_keeps_what_it_has_and_defaults_the_rest() {
    let parsed: UiSettings =
        serde_json::from_str(r#"{"theme":"pine","someRemovedField":1}"#).expect("tolerant parse");
    assert_eq!(parsed.theme, ThemeName::Pine);
    assert_eq!(parsed.pill_theme, PillTheme::Steel);
    assert!(!parsed.model_chosen, "first-run picker must still show");
}

#[test]
fn the_defaults_match_a_fresh_install() {
    let fresh = UiSettings::default();
    assert_eq!(fresh.theme, ThemeName::Abyss);
    assert_eq!(fresh.waveform_style, WaveformStyle::Bars);
    assert_eq!(fresh.route, Route::Dashboard);
    assert_eq!(fresh.settings_tab, SettingsTab::Appearance);
    assert!(!fresh.live_transcript);
}
