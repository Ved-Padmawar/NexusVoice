//! The formatter providers on offer and the rules for a usable endpoint.
//! `FormatConfig::is_usable` is what really decides at dictation time; the
//! checks here only gate the modal's buttons, so they must never be stricter.

use crate::llm::config::{FormatConfig, Profile};

pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    /// Prefilled for a provider that was never configured.
    pub base_url: &'static str,
    pub needs_key: bool,
    pub model_hint: &'static str,
}

/// Every preset but `anthropic` speaks the OpenAI-compatible API.
pub const PRESETS: [Preset; 6] = [
    Preset {
        id: "ollama",
        label: "Ollama (local/cloud)",
        base_url: "http://localhost:11434/v1",
        needs_key: false,
        model_hint: "qwen2.5:3b-instruct",
    },
    Preset {
        id: "lmstudio",
        label: "LM Studio (local)",
        base_url: "http://localhost:1234/v1",
        needs_key: false,
        model_hint: "qwen2.5-3b-instruct",
    },
    Preset {
        id: "openai",
        label: "OpenAI",
        base_url: "https://api.openai.com/v1",
        needs_key: true,
        model_hint: "gpt-5.5",
    },
    Preset {
        id: "openrouter",
        label: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1",
        needs_key: true,
        model_hint: "meta-llama/llama-3.1-8b-instruct",
    },
    Preset {
        id: "anthropic",
        label: "Anthropic",
        base_url: "",
        needs_key: true,
        model_hint: "claude-sonnet-5",
    },
    Preset {
        id: "custom",
        label: "Custom",
        base_url: "",
        needs_key: false,
        model_hint: "model name",
    },
];

/// The preset for `id`; the first for an id nobody knows.
pub fn preset(id: &str) -> &'static Preset {
    PRESETS.iter().find(|p| p.id == id).unwrap_or(&PRESETS[0])
}

/// A provider's name for the card summary; an unknown id stands for itself.
pub fn label(id: &str) -> &str {
    PRESETS.iter().find(|p| p.id == id).map_or(id, |p| p.label)
}

/// Anthropic pins its own endpoint, so a base URL field would mislead.
pub fn shows_base_url(provider: &str) -> bool {
    provider != "anthropic"
}

/// Whether `provider` has the minimum to make a request: a model, and a base
/// URL unless it is Anthropic. The API key is never required: blank sends no
/// auth header, which is how local servers run.
pub fn is_configured(provider: &str, base_url: &str, model: &str) -> bool {
    !model.trim().is_empty() && (provider == "anthropic" || !base_url.trim().is_empty())
}

/// What the form shows after switching to `provider`: its saved details, with
/// the preset URL standing in only for a provider that never had one.
pub fn profile_for_switch(provider: &str, saved: Option<&Profile>) -> Profile {
    let saved = saved.cloned().unwrap_or_default();
    let fallback_url = match provider {
        "custom" | "anthropic" => "",
        _ => preset(provider).base_url,
    };
    Profile {
        base_url: if saved.base_url.is_empty() {
            fallback_url.to_string()
        } else {
            saved.base_url
        },
        ..saved
    }
}

/// `initial` with `profile`, trimmed, saved under `provider` and made active.
pub fn draft(initial: &FormatConfig, provider: &str, profile: &Profile) -> FormatConfig {
    let mut config = initial.clone();
    config.provider = provider.to_string();
    config.profiles.insert(
        provider.to_string(),
        Profile {
            base_url: profile.base_url.trim().to_string(),
            model: profile.model.trim().to_string(),
            api_key: profile.api_key.trim().to_string(),
        },
    );
    config
}

#[cfg(test)]
#[path = "../../../../../../tests/unit/ui/settings/presets.rs"]
mod tests;
