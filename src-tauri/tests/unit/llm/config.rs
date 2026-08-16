use super::*;

fn cfg(base_url: &str, model: &str, enabled: bool) -> FormatConfig {
    FormatConfig {
        enabled,
        base_url: base_url.to_string(),
        model: model.to_string(),
        ..Default::default()
    }
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
    let c = FormatConfig {
        enabled: true,
        provider: "anthropic".to_string(),
        model: "claude-sonnet-5".to_string(),
        ..Default::default()
    };
    assert!(c.is_usable());
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
