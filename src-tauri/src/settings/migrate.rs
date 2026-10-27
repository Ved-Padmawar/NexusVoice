//! First run after the upgrade to `settings.json`: read the one-file-per-setting
//! layout of earlier versions, moving formatter keys into the credential store.
//! The old files are removed once `settings.json` is written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::llm::FormatConfig;

use super::formatter::{Endpoint, FormatterSettings};
use super::{HotkeyBindings, Secrets, SettingsFile};

const HOTKEYS: &str = "hotkeys.json";
const MODEL: &str = "model_override";
const MODEL_UNLOAD: &str = "model_unload";
const LANGUAGE: &str = "language";
const INPUT_DEVICE: &str = "input_device";
const FORMATTER: &str = "format_config.json";

pub struct Imported {
    pub file: SettingsFile,
    sources: Vec<PathBuf>,
}

impl Imported {
    pub fn remove_sources(&self) {
        for path in &self.sources {
            if let Err(e) = std::fs::remove_file(path) {
                log::warn!("could not remove {}: {e}", path.display());
            }
        }
    }
}

pub fn import(dir: &Path, secrets: &Secrets) -> Imported {
    let mut sources = Vec::new();
    let mut read = |name: &str| {
        let path = dir.join(name);
        let text = std::fs::read_to_string(&path).ok()?;
        sources.push(path);
        Some(text)
    };
    let trimmed =
        |text: Option<String>| text.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    let hotkeys = read(HOTKEYS)
        .and_then(|json| serde_json::from_str::<HashMap<String, String>>(&json).ok())
        .map(|map| {
            let get = |key: &str| {
                map.get(key)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            };
            HotkeyBindings {
                push_to_talk: get("pushToTalk"),
                dictation: get("dictation"),
                dictation_commit: get("dictationCommit"),
            }
        })
        .unwrap_or_default();
    let model = trimmed(read(MODEL));
    let model_unload = read(MODEL_UNLOAD)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let language = trimmed(read(LANGUAGE));
    let input_device = trimmed(read(INPUT_DEVICE));
    let formatter = read(FORMATTER)
        .and_then(|json| serde_json::from_str::<FormatConfig>(&json).ok())
        .map(|config| formatter_from(config, secrets))
        .unwrap_or_default();

    Imported {
        file: SettingsFile {
            hotkeys,
            model,
            model_unload,
            language,
            input_device,
            formatter,
            ..SettingsFile::default()
        },
        sources,
    }
}

/// A key that cannot be moved into the store is dropped rather than written
/// back to disk; the user re-enters it.
fn formatter_from(config: FormatConfig, secrets: &Secrets) -> FormatterSettings {
    let endpoints = config
        .profiles
        .into_iter()
        .map(|(provider, profile)| {
            let key = profile.api_key.trim();
            if !key.is_empty() {
                if let Err(e) = secrets.set_api_key(&provider, key) {
                    log::error!("{e}");
                }
            }
            let endpoint = Endpoint {
                base_url: profile.base_url,
                model: profile.model,
            };
            (provider, endpoint)
        })
        .collect();
    FormatterSettings {
        enabled: config.enabled,
        provider: config.provider,
        endpoints,
    }
}
