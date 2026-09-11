use super::*;

fn with(provider: &str, base_url: &str, model: &str, enabled: bool) -> FormatConfig {
    let mut profiles = HashMap::new();
    profiles.insert(
        provider.to_string(),
        Profile {
            base_url: base_url.to_string(),
            model: model.to_string(),
            api_key: String::new(),
        },
    );
    FormatConfig {
        enabled,
        provider: provider.to_string(),
        profiles,
    }
}

fn cfg(base_url: &str, model: &str, enabled: bool) -> FormatConfig {
    with("ollama", base_url, model, enabled)
}

#[test]
fn default_is_disabled_and_unusable() {
    let c = FormatConfig::default();
    assert!(!c.enabled);
    assert!(!c.is_usable());
}

#[test]
fn is_usable_requires_enabled_url_and_model() {
    assert!(!cfg("http://localhost:1234", "m", false).is_usable()); // disabled
    assert!(!cfg("", "m", true).is_usable()); // no url
    assert!(!cfg("http://localhost:1234", "  ", true).is_usable()); // blank model
    assert!(cfg("http://localhost:1234", "qwen2.5-3b-instruct", true).is_usable());
}

#[test]
fn is_usable_anthropic_does_not_require_base_url() {
    assert!(with("anthropic", "", "claude-sonnet-5", true).is_usable());
}

#[test]
fn active_returns_the_selected_providers_details() {
    let mut c = with("ollama", "http://localhost:11434/v1", "qwen", true);
    c.profiles.insert(
        "lmstudio".to_string(),
        Profile {
            base_url: "http://localhost:1234/v1".to_string(),
            model: "phi-4".to_string(),
            api_key: String::new(),
        },
    );

    assert_eq!(c.active().model, "qwen");
    c.provider = "lmstudio".to_string();
    assert_eq!(c.active().model, "phi-4");
}

#[test]
fn switching_provider_keeps_every_profile() {
    // The bug this shape exists to prevent: configuring one provider used to
    // overwrite the last one's endpoint.
    let mut c = with("lmstudio", "http://localhost:1234/v1", "phi-4", true);
    c.profiles.insert(
        "ollama".to_string(),
        Profile {
            base_url: "http://localhost:11434/v1".to_string(),
            model: "qwen".to_string(),
            api_key: String::new(),
        },
    );

    c.provider = "ollama".to_string();
    assert_eq!(c.active().model, "qwen");

    c.provider = "lmstudio".to_string();
    assert_eq!(c.active().base_url, "http://localhost:1234/v1");
    assert_eq!(c.active().model, "phi-4");
}

#[test]
fn active_is_empty_for_a_provider_never_configured() {
    let mut c = cfg("http://localhost:11434/v1", "qwen", true);
    c.provider = "openai".to_string();
    assert!(c.active().model.is_empty());
    assert!(!c.is_usable());
}

#[test]
fn each_profile_keeps_its_own_api_key() {
    let mut c = with("openai", "https://api.openai.com/v1", "gpt-4o-mini", true);
    c.profiles.get_mut("openai").unwrap().api_key = "sk-one".to_string();
    c.profiles.insert(
        "openrouter".to_string(),
        Profile {
            base_url: "https://openrouter.ai/api/v1".to_string(),
            model: "llama-3.1-8b".to_string(),
            api_key: "sk-two".to_string(),
        },
    );

    assert_eq!(c.active().api_key, "sk-one");
    c.provider = "openrouter".to_string();
    assert_eq!(c.active().api_key, "sk-two");
}

#[test]
fn endpoint_inserts_v1_when_host_only() {
    // No path beyond host → insert the common /v1 prefix (LM Studio footgun).
    assert_eq!(
        cfg("http://127.0.0.1:1234", "m", true).endpoint(),
        "http://127.0.0.1:1234/v1/chat/completions"
    );
}

#[test]
fn endpoint_appends_when_path_present() {
    assert_eq!(
        cfg("http://localhost:11434/v1", "m", true).endpoint(),
        "http://localhost:11434/v1/chat/completions"
    );
}

#[test]
fn endpoint_uses_as_is_when_already_complete() {
    let url = "https://api.openai.com/v1/chat/completions";
    assert_eq!(cfg(url, "m", true).endpoint(), url);
}

#[test]
fn endpoint_tolerates_trailing_slash_and_whitespace() {
    assert_eq!(
        cfg("  http://localhost:1234/  ", "m", true).endpoint(),
        "http://localhost:1234/v1/chat/completions"
    );
}

// ── Persistence ────────────────────────────────────────────────────────
// `format_config.json` is written by older builds too, so every field carries
// `#[serde(default)]`. A missing one must load, not wipe the user's setup.

#[test]
fn an_empty_config_file_loads_as_the_default() {
    let c: FormatConfig = serde_json::from_str("{}").expect("parse");
    assert!(!c.enabled);
    assert_eq!(
        c.provider, "ollama",
        "a missing provider defaults to ollama"
    );
    assert!(c.profiles.is_empty());
    assert!(!c.is_usable());
}

#[test]
fn a_config_missing_the_profiles_map_still_loads() {
    // Written by a build that predates per-provider profiles.
    let c: FormatConfig = serde_json::from_str(r#"{"enabled": true}"#).expect("parse");
    assert!(c.enabled);
    assert!(c.profiles.is_empty());
    assert!(!c.is_usable(), "no model means not usable");
}

#[test]
fn a_profile_missing_its_api_key_loads_with_an_empty_key() {
    // A blank key is the local-server case, so absent must mean empty, not fail.
    let json = r#"{
        "enabled": true,
        "provider": "ollama",
        "profiles": {"ollama": {"baseUrl": "http://localhost:11434/v1", "model": "qwen"}}
    }"#;
    let c: FormatConfig = serde_json::from_str(json).expect("parse");
    assert_eq!(c.active().api_key, "");
    assert!(c.is_usable());
}

#[test]
fn a_config_round_trips_through_json_unchanged() {
    // What is saved must be what loads back, keys included.
    let mut original = with("openai", "https://api.openai.com/v1", "gpt-4o-mini", true);
    original.profiles.get_mut("openai").unwrap().api_key = "sk-secret".to_string();

    let reloaded: FormatConfig =
        serde_json::from_str(&serde_json::to_string(&original).expect("write")).expect("read");

    assert_eq!(reloaded.enabled, original.enabled);
    assert_eq!(reloaded.provider, original.provider);
    assert_eq!(reloaded.active().base_url, "https://api.openai.com/v1");
    assert_eq!(reloaded.active().model, "gpt-4o-mini");
    assert_eq!(reloaded.active().api_key, "sk-secret");
}

#[test]
fn the_json_keys_are_camel_case() {
    // The frontend reads this same shape through the generated bindings.
    let json = serde_json::to_string(&with("ollama", "http://x/v1", "m", true)).expect("write");
    assert!(json.contains("\"baseUrl\""), "{json}");
    assert!(json.contains("\"apiKey\""), "{json}");
}
