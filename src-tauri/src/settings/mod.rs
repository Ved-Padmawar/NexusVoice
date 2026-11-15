//! Saved preferences: one versioned `settings.json`, replaced atomically on
//! every change. API keys never touch the file; they live in the OS
//! credential store (`secrets`).

pub mod formatter;
mod migrate;
mod secrets;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::mirror::{Committed, Patch, Slice, Store};
use crate::models::idle::ModelUnload;
use crate::models::language::{self, LanguageSettings};
use crate::models::library::ModelLibrary;
use crate::text::llm::FormatConfig;

use formatter::{FormatterDraft, FormatterSettings, FormatterView};
pub use secrets::Secrets;

const FILE_NAME: &str = "settings.json";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PillTheme {
    #[default]
    Steel,
    Midnight,
    Aurora,
    Canvas,
    Dawn,
    Glass,
}

/// How the pill draws the capture levels. All four read the same frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum WaveformStyle {
    #[default]
    Bars,
    Memo,
    Eq,
    Spectrum,
}

/// How a finished transcript is pasted where the clipboard route applies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PasteMode {
    /// Ctrl+Shift+V in a terminal, Ctrl+V anywhere else.
    #[default]
    Auto,
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
    /// Leave the transcript on the clipboard and send no keys.
    CopyOnly,
}

/// Whether dictations keep their audio, and for how many of the newest ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedAudio {
    pub enabled: bool,
    pub keep: u32,
}

impl SavedAudio {
    pub const KEEP_RANGE: std::ops::RangeInclusive<u32> = 1..=100;

    fn apply(&mut self, change: SavedAudioSetting) {
        match change {
            SavedAudioSetting::Enabled(on) => self.enabled = on,
            SavedAudioSetting::Keep(keep) => self.keep = keep,
        }
    }
}

/// One saved-audio choice, changed on its own, so a quick toggle and a change
/// of the limit can't overwrite each other.
#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum SavedAudioSetting {
    Enabled(bool),
    Keep(u32),
}

impl Default for SavedAudio {
    fn default() -> Self {
        Self {
            enabled: true,
            keep: 10,
        }
    }
}

/// Preferences both windows read: the settings screen edits them, the pill
/// draws with them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct PillSettings {
    pub theme: PillTheme,
    pub waveform_style: WaveformStyle,
    /// Expand the pill into a card that fills in while recording.
    pub live_transcript: bool,
    /// Chime once the microphone is live, in either recording mode.
    pub start_sound: bool,
}

/// One pill preference, changed on its own, so two quick edits of different
/// ones can't overwrite each other.
#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum PillSetting {
    Theme(PillTheme),
    WaveformStyle(WaveformStyle),
    LiveTranscript(bool),
    StartSound(bool),
}

impl PillSettings {
    fn apply(&mut self, change: PillSetting) {
        match change {
            PillSetting::Theme(theme) => self.theme = theme,
            PillSetting::WaveformStyle(style) => self.waveform_style = style,
            PillSetting::LiveTranscript(on) => self.live_transcript = on,
            PillSetting::StartSound(on) => self.start_sound = on,
        }
    }
}

/// The global shortcuts, by role. `None` leaves a role unbound.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct HotkeyBindings {
    pub push_to_talk: Option<String>,
    pub dictation: Option<String>,
    pub dictation_commit: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SettingsFile {
    version: u32,
    hotkeys: HotkeyBindings,
    /// Catalog id of the chosen model; `None` follows the hardware pick.
    model: Option<String>,
    model_unload: ModelUnload,
    /// ISO code or `auto`; `None` when never chosen.
    language: Option<String>,
    /// Input device id (`host:device`); `None` follows the OS default.
    input_device: Option<String>,
    keep_mic_open: bool,
    saved_audio: SavedAudio,
    paste_mode: PasteMode,
    formatter: FormatterSettings,
    pill: PillSettings,
}

/// What a write can change: the file, and which providers have a key in the
/// credential store, so building the view never asks the store.
#[derive(Debug, Clone, PartialEq)]
struct Saved {
    file: SettingsFile,
    keyed: BTreeSet<String>,
}

/// What the windows mirror of the saved preferences. Keys never belong here.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Catalog id of the model recordings use: the saved choice, else the
    /// hardware pick.
    pub active_model: String,
    /// The saved shortcuts; the OS may grant fewer (`Patch::Hotkeys`).
    pub hotkeys: HotkeyBindings,
    /// The active model's languages, with the saved one marked.
    pub language: LanguageSettings,
    pub model_unload: ModelUnload,
    /// Input device id; `None` follows the OS default.
    pub input_device: Option<String>,
    pub keep_mic_open: bool,
    pub saved_audio: SavedAudio,
    pub paste_mode: PasteMode,
    pub formatter: FormatterView,
    pub pill: PillSettings,
}

impl Slice for Saved {
    fn patch(&self, rev: u64) -> Patch {
        let file = &self.file;
        let model = ModelLibrary::active_for(file.model.as_deref());
        Patch::Settings {
            rev,
            value: SettingsView {
                active_model: model.id.clone(),
                hotkeys: file.hotkeys.clone(),
                language: language::options(model, file.language.as_deref()),
                model_unload: file.model_unload,
                input_device: file.input_device.clone(),
                keep_mic_open: file.keep_mic_open,
                saved_audio: file.saved_audio,
                paste_mode: file.paste_mode,
                formatter: file.formatter.view(&self.keyed),
                pill: file.pill,
            },
        }
    }
}

pub struct Settings {
    path: PathBuf,
    saved: Store<Saved>,
    secrets: Secrets,
}

impl Settings {
    /// Read `settings.json`, importing the per-setting files of older
    /// versions when it does not exist yet.
    pub fn load(app_data_dir: &Path, secrets: Secrets) -> Self {
        let path = app_data_dir.join(FILE_NAME);
        let file = if let Ok(json) = std::fs::read_to_string(&path) {
            serde_json::from_str(&json).unwrap_or_else(|e| {
                log::error!("settings.json is unreadable, using defaults: {e}");
                SettingsFile::default()
            })
        } else {
            let imported = migrate::import(app_data_dir, &secrets);
            match write_atomic(&path, &imported.file) {
                Ok(()) => imported.remove_sources(),
                Err(e) => log::error!("could not write settings.json: {e}"),
            }
            imported.file
        };
        let keyed = file
            .formatter
            .endpoints
            .keys()
            .filter(|provider| secrets.api_key(provider).is_some())
            .cloned()
            .collect();
        Self {
            path,
            saved: Store::new(Saved { file, keyed }),
            secrets,
        }
    }

    /// The only write: apply `change` and write the file; memory and the
    /// windows change only once the file has.
    fn update(&self, change: impl FnOnce(&mut Saved)) -> std::io::Result<Committed> {
        self.saved
            .update(change, |next| write_atomic(&self.path, &next.file))
    }

    fn read<T>(&self, get: impl FnOnce(&SettingsFile) -> T) -> T {
        self.saved.read(|saved| get(&saved.file))
    }

    pub fn hotkeys(&self) -> HotkeyBindings {
        self.read(|f| f.hotkeys.clone())
    }

    pub fn set_hotkeys(&self, hotkeys: HotkeyBindings) -> std::io::Result<Committed> {
        self.update(|s| s.file.hotkeys = hotkeys)
    }

    pub fn model(&self) -> Option<String> {
        self.read(|f| f.model.clone())
    }

    pub fn set_model(&self, id: Option<String>) -> std::io::Result<Committed> {
        self.update(|s| s.file.model = id)
    }

    pub fn model_unload(&self) -> ModelUnload {
        self.read(|f| f.model_unload)
    }

    pub fn set_model_unload(&self, policy: ModelUnload) -> std::io::Result<Committed> {
        self.update(|s| s.file.model_unload = policy)
    }

    pub fn language(&self) -> Option<String> {
        self.read(|f| f.language.clone())
    }

    pub fn set_language(&self, code: Option<String>) -> std::io::Result<Committed> {
        self.update(|s| s.file.language = code)
    }

    pub fn input_device(&self) -> Option<String> {
        self.read(|f| f.input_device.clone())
    }

    pub fn set_input_device(&self, id: Option<String>) -> std::io::Result<Committed> {
        self.update(|s| s.file.input_device = id)
    }

    pub fn keep_mic_open(&self) -> bool {
        self.read(|f| f.keep_mic_open)
    }

    pub fn set_keep_mic_open(&self, keep: bool) -> std::io::Result<Committed> {
        self.update(|s| s.file.keep_mic_open = keep)
    }

    pub fn saved_audio(&self) -> SavedAudio {
        self.read(|f| f.saved_audio)
    }

    pub fn set_saved_audio(&self, change: SavedAudioSetting) -> std::io::Result<Committed> {
        self.update(|s| s.file.saved_audio.apply(change))
    }

    pub fn paste_mode(&self) -> PasteMode {
        self.read(|f| f.paste_mode)
    }

    pub fn set_paste_mode(&self, mode: PasteMode) -> std::io::Result<Committed> {
        self.update(|s| s.file.paste_mode = mode)
    }

    pub fn pill(&self) -> PillSettings {
        self.read(|f| f.pill)
    }

    pub fn set_pill(&self, change: PillSetting) -> std::io::Result<Committed> {
        self.update(|s| s.file.pill.apply(change))
    }

    pub fn set_formatter_enabled(&self, enabled: bool) -> std::io::Result<Committed> {
        self.update(|s| s.file.formatter.enabled = enabled)
    }

    /// The endpoint goes to the file, its key to the credential store.
    pub fn save_formatter_endpoint(&self, draft: &FormatterDraft) -> Result<Committed, String> {
        draft.store_key(&self.secrets)?;
        self.update(|s| {
            draft.apply(&mut s.file.formatter);
            match draft.has_key() {
                Some(true) => {
                    s.keyed.insert(draft.provider.clone());
                }
                Some(false) => {
                    s.keyed.remove(&draft.provider);
                }
                None => {}
            }
        })
        .map_err(|e| e.to_string())
    }

    /// The formatter as the finalize step calls it, with its key.
    pub fn format_config(&self) -> FormatConfig {
        self.read(|f| f.formatter.resolve(&self.secrets))
    }

    /// An unsaved draft from the settings screen, completed with the saved
    /// key where the draft leaves it unchanged.
    pub fn format_config_for(&self, draft: &FormatterDraft) -> FormatConfig {
        draft.resolve(&self.secrets)
    }
}

/// Write to a sibling temp file, then rename it over the target, so a crash
/// mid-write leaves the previous file intact.
fn write_atomic(path: &Path, file: &SettingsFile) -> std::io::Result<()> {
    let file = SettingsFile {
        version: VERSION,
        ..file.clone()
    };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
#[path = "../../tests/unit/settings/mod.rs"]
mod tests;
