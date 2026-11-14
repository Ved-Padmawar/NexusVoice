//! Smart-formatting provider settings. Each provider keeps its own endpoint,
//! so switching between them and back preserves what was entered for each.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::text::llm::config::{check_base_url, FormatConfig, Profile};

use super::Secrets;

const DEFAULT_PROVIDER: &str = "ollama";

/// One provider's endpoint as saved. Its key is in the credential store.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Endpoint {
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// One provider's endpoint from the provider modal. Saving it touches only that
/// endpoint, so a toggle landing mid-save can't bring back an older one.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatterDraft {
    pub provider: String,
    pub profile: ProfileUpdate,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdate {
    pub base_url: String,
    pub model: String,
    /// `None` keeps the saved key, an empty string removes it.
    pub api_key: Option<String>,
}

impl FormatterDraft {
    /// Reject an address no request could reach. An empty address is a draft
    /// in progress, not an error.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.provider == "anthropic" || self.profile.base_url.trim().is_empty() {
            return Ok(());
        }
        check_base_url(&self.profile.base_url)
    }

    pub fn store_key(&self, secrets: &Secrets) -> Result<(), String> {
        match self.profile.api_key.as_deref().map(str::trim) {
            None => Ok(()),
            Some("") => secrets.delete_api_key(&self.provider),
            Some(key) => secrets.set_api_key(&self.provider, key),
        }
    }

    fn endpoint(&self) -> Endpoint {
        Endpoint {
            base_url: self.profile.base_url.trim().to_string(),
            model: self.profile.model.trim().to_string(),
        }
    }

    /// Make this the active endpoint and turn formatting on.
    pub fn apply(&self, saved: &mut FormatterSettings) {
        saved
            .endpoints
            .insert(self.provider.clone(), self.endpoint());
        saved.provider.clone_from(&self.provider);
        saved.enabled = true;
    }

    pub fn resolve(&self, secrets: &Secrets) -> FormatConfig {
        let api_key = match self.profile.api_key.as_deref() {
            Some(key) => key.trim().to_string(),
            None => secrets.api_key(&self.provider).unwrap_or_default(),
        };
        active_config(true, &self.provider, self.endpoint(), api_key)
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
