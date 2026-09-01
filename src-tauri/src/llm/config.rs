//! Formatter LLM configuration, persisted as JSON in the app data dir.
//!
//! Each provider keeps its own endpoint details, so switching between them and
//! back preserves what was entered for each.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One provider's endpoint details.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Base URL up to (not including) `/chat/completions`. Unused by
    /// `anthropic`, which pins its own endpoint.
    #[serde(default)]
    pub base_url: String,
    /// Model name/id, e.g. `qwen3:0.6b` or `gpt-4o-mini`.
    #[serde(default)]
    pub model: String,
    /// API key. Empty → no `Authorization` header sent (local Ollama default).
    #[serde(default)]
    pub api_key: String,
}

/// Persisted formatter configuration.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatConfig {
    /// Whether the LLM formatting stage runs.
    #[serde(default)]
    pub enabled: bool,
    /// Preset id of the active provider (`"ollama"`, `"openai"`, …).
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Endpoint details per provider id.
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

fn default_provider() -> String {
    "ollama".to_string()
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_provider(),
            profiles: HashMap::new(),
        }
    }
}

impl FormatConfig {
    /// The active provider's details, or an empty profile if none is stored.
    pub fn active(&self) -> Profile {
        self.profiles
            .get(&self.provider)
            .cloned()
            .unwrap_or_default()
    }

    /// Whether the config has the minimum needed to make a request.
    ///
    /// `anthropic` pins its own endpoint (see `llm::anthropic`), so it doesn't
    /// need `base_url` — every other provider is OpenAI-compatible and does.
    pub fn is_usable(&self) -> bool {
        let p = self.active();
        if !self.enabled || p.model.trim().is_empty() {
            return false;
        }
        if self.provider == "anthropic" {
            return true;
        }
        !p.base_url.trim().is_empty()
    }

    /// Full chat-completions endpoint URL.
    ///
    /// Tolerant of how the user typed `base_url`:
    /// - if it already ends in `/chat/completions`, use it as-is;
    /// - if it has no path beyond the host (e.g. `http://127.0.0.1:1234`),
    ///   insert `/v1` — the common `OpenAI`-compatible prefix that LM Studio,
    ///   Ollama, `OpenAI`, etc. all expect — so a missing `/v1` isn't a footgun;
    /// - otherwise append `/chat/completions` to whatever path was given.
    pub fn endpoint(&self) -> String {
        let active = self.active();
        let base = active.base_url.trim().trim_end_matches('/');

        if base.ends_with("/chat/completions") {
            return base.to_string();
        }

        // Detect whether there's a path after the host. Strip the scheme, then
        // look for a '/' in the remainder.
        let after_scheme = base.split_once("://").map_or(base, |(_, rest)| rest);
        let has_path = after_scheme.contains('/');

        if has_path {
            format!("{base}/chat/completions")
        } else {
            format!("{base}/v1/chat/completions")
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/llm/config.rs"]
mod tests;
