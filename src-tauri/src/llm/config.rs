//! Formatter LLM configuration: which OpenAI-compatible provider/endpoint to
//! call, persisted as JSON in the app data dir.
//!
//! All supported providers (Ollama, `OpenAI`, `OpenRouter`, and any custom
//! endpoint) speak the `OpenAI` `/chat/completions` API, so a single config
//! shape covers them. The API key is optional — local Ollama needs none, but a
//! key-protected Ollama or any cloud provider supplies one.

use serde::{Deserialize, Serialize};

/// Persisted formatter configuration. `enabled` is the toggle; the rest is the
/// endpoint config entered via the provider modal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatConfig {
    /// Whether the LLM formatting stage runs.
    #[serde(default)]
    pub enabled: bool,
    /// Provider preset id (`"ollama" | "openai" | "openrouter" | "custom"`).
    /// Purely informational for the UI; behavior is driven by the fields below.
    #[serde(default)]
    pub provider: String,
    /// Base URL up to (not including) `/chat/completions`, e.g.
    /// `http://localhost:11434/v1`.
    #[serde(default)]
    pub base_url: String,
    /// Model name/id, e.g. `qwen3:0.6b` or `gpt-4o-mini`.
    #[serde(default)]
    pub model: String,
    /// API key. Empty → no `Authorization` header sent (local Ollama default).
    #[serde(default)]
    pub api_key: String,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "ollama".to_string(),
            base_url: String::new(),
            model: String::new(),
            api_key: String::new(),
        }
    }
}

impl FormatConfig {
    /// Whether the config has the minimum needed to make a request.
    ///
    /// `anthropic` pins its own endpoint (see `llm::anthropic`), so it doesn't
    /// need `base_url` — every other provider is OpenAI-compatible and does.
    pub fn is_usable(&self) -> bool {
        if !self.enabled || self.model.trim().is_empty() {
            return false;
        }
        if self.provider == "anthropic" {
            return true;
        }
        !self.base_url.trim().is_empty()
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
        let base = self.base_url.trim().trim_end_matches('/');

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
