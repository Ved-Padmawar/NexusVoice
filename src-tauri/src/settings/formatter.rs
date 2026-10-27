//! Smart-formatting provider settings. Each provider keeps its own endpoint,
//! so switching between them and back preserves what was entered for each.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::llm::config::{FormatConfig, Profile};

use super::Secrets;

const DEFAULT_PROVIDER: &str = "ollama";

/// One provider's endpoint as saved. Its key is in the credential store.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Endpoint {
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormatterSettings {
    pub enabled: bool,
    /// Preset id of the active provider (`ollama`, `openai`, …).
    pub provider: String,
    pub endpoints: HashMap<String, Endpoint>,
}

impl Default for FormatterSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: DEFAULT_PROVIDER.to_string(),
            endpoints: HashMap::new(),
        }
    }
}

impl FormatterSettings {
    pub fn view(&self, secrets: &Secrets) -> FormatterView {
        FormatterView {
            enabled: self.enabled,
            provider: self.provider.clone(),
            profiles: self
                .endpoints
                .iter()
                .map(|(provider, endpoint)| {
                    let view = ProfileView {
                        base_url: endpoint.base_url.clone(),
                        model: endpoint.model.clone(),
                        has_api_key: secrets.api_key(provider).is_some(),
                    };
                    (provider.clone(), view)
                })
                .collect(),
        }
    }

    pub fn resolve(&self, secrets: &Secrets) -> FormatConfig {
        let endpoint = self
            .endpoints
            .get(&self.provider)
            .cloned()
            .unwrap_or_default();
        let api_key = secrets.api_key(&self.provider).unwrap_or_default();
        active_config(self.enabled, &self.provider, endpoint, api_key)
    }
}

/// The settings screen's view: whether a key is saved, never the key.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatterView {
    pub enabled: bool,
    pub provider: String,
    pub profiles: HashMap<String, ProfileView>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
}

/// A change from the settings screen.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatterUpdate {
    pub enabled: bool,
    pub provider: String,
    pub profiles: HashMap<String, ProfileUpdate>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdate {
    pub base_url: String,
    pub model: String,
    /// `None` keeps the saved key, an empty string removes it.
    pub api_key: Option<String>,
}

impl FormatterUpdate {
    pub fn store_keys(&self, secrets: &Secrets) -> Result<(), String> {
        for (provider, profile) in &self.profiles {
            match profile.api_key.as_deref().map(str::trim) {
                None => {}
                Some("") => secrets.delete_api_key(provider)?,
                Some(key) => secrets.set_api_key(provider, key)?,
            }
        }
        Ok(())
    }

    pub fn into_settings(self) -> FormatterSettings {
        FormatterSettings {
            enabled: self.enabled,
            provider: self.provider,
            endpoints: self
                .profiles
                .into_iter()
                .map(|(provider, p)| {
                    let endpoint = Endpoint {
                        base_url: p.base_url.trim().to_string(),
                        model: p.model.trim().to_string(),
                    };
                    (provider, endpoint)
                })
                .collect(),
        }
    }

    pub fn resolve(&self, secrets: &Secrets) -> FormatConfig {
        let profile = self.profiles.get(&self.provider);
        let endpoint = profile.map_or_else(Endpoint::default, |p| Endpoint {
            base_url: p.base_url.trim().to_string(),
            model: p.model.trim().to_string(),
        });
        let api_key = match profile.and_then(|p| p.api_key.as_deref()) {
            Some(key) => key.trim().to_string(),
            None => secrets.api_key(&self.provider).unwrap_or_default(),
        };
        active_config(self.enabled, &self.provider, endpoint, api_key)
    }
}

fn active_config(
    enabled: bool,
    provider: &str,
    endpoint: Endpoint,
    api_key: String,
) -> FormatConfig {
    let profile = Profile {
        base_url: endpoint.base_url,
        model: endpoint.model,
        api_key,
    };
    FormatConfig {
        enabled,
        provider: provider.to_string(),
        profiles: HashMap::from([(provider.to_string(), profile)]),
    }
}
