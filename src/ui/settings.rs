//! Interface preferences, saved as `ui.json` in the app data dir. Backend
//! settings (model, language, hotkeys…) keep their own files.

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

use crate::platform::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    #[default]
    Abyss,
    Midnight,
    Steel,
    Pine,
    Canvas,
    Dawn,
    Breeze,
    Blossom,
}

impl ThemeName {
    pub const ALL: [Self; 8] = [
        Self::Abyss,
        Self::Midnight,
        Self::Steel,
        Self::Pine,
        Self::Canvas,
        Self::Dawn,
        Self::Breeze,
        Self::Blossom,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PillTheme {
    #[default]
    Steel,
    Midnight,
    Canvas,
    Dawn,
}

impl PillTheme {
    pub const ALL: [Self; 4] = [Self::Steel, Self::Midnight, Self::Canvas, Self::Dawn];
}

/// How the pill draws the capture levels. All four read the same frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaveformStyle {
    #[default]
    Bars,
    Memo,
    Eq,
    Spectrum,
}

impl WaveformStyle {
    /// The settings table lists styles itself; tests check it covers these.
    #[cfg(test)]
    pub const ALL: [Self; 4] = [Self::Bars, Self::Memo, Self::Eq, Self::Spectrum];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Route {
    #[default]
    Dashboard,
    Dictionary,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsTab {
    General,
    Voice,
    #[default]
    Appearance,
    About,
}

impl SettingsTab {
    pub const ALL: [Self; 4] = [Self::General, Self::Voice, Self::Appearance, Self::About];
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UiSettings {
    pub theme: ThemeName,
    pub pill_theme: PillTheme,
    pub waveform_style: WaveformStyle,
    /// Expand the pill into a transcript card. Streaming models only.
    pub live_transcript: bool,
    pub route: Route,
    pub settings_tab: SettingsTab,
    /// First-run model picker done; it never opens again on its own.
    pub model_chosen: bool,
    /// The CUDA offer was shown or CUDA was installed; it never returns.
    pub cuda_offer_seen: bool,
}

impl Global for UiSettings {}

impl UiSettings {
    fn path() -> std::path::PathBuf {
        paths::app_data_dir().join("ui.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Change the settings and save them. Views re-render through the global
    /// observer; nothing is written when the edit changed nothing.
    pub fn update(cx: &mut App, edit: impl FnOnce(&mut Self)) {
        let mut next = Self::global(cx).clone();
        edit(&mut next);
        if next == *Self::global(cx) {
            return;
        }
        match serde_json::to_string_pretty(&next) {
            Ok(json) => {
                if let Err(e) = std::fs::write(Self::path(), json) {
                    log::warn!("could not save ui settings: {e}");
                }
            }
            Err(e) => log::warn!("could not serialize ui settings: {e}"),
        }
        cx.set_global(next);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/settings.rs"]
mod tests;
