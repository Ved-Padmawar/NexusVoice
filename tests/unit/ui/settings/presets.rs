//! The provider modal's rules. Stricter than `FormatConfig::is_usable` and the
//! user is locked out of a setup the backend would accept; looser and Save
//! stores a config that never formats anything.

use std::collections::HashMap;

use crate::llm::config::{FormatConfig, Profile};
use crate::ui::vendors::Vendor;

use super::{draft, is_configured, label, preset, profile_for_switch, shows_base_url, PRESETS};

fn profile(base_url: &str, model: &str, api_key: &str) -> Profile {
    Profile {
        base_url: base_url.into(),
        model: model.into(),
        api_key: api_key.into(),
    }
}

#[test]
fn the_buttons_agree_with_the_backends_usability_check() {
    let urls = ["", "  ", "http://localhost:11434/v1", "  http://x/v1  "];
    let models = ["", "   ", "qwen", "  qwen  "];
    let providers = PRESETS.iter().map(|p| p.id).chain(["some-new-provider"]);
    for provider in providers {
        for url in urls {
            for model in models {
                let config = FormatConfig {
                    enabled: true,
                    provider: provider.into(),
                    profiles: HashMap::from([(provider.into(), profile(url, model, ""))]),
                };
                assert_eq!(
                    is_configured(provider, url, model),
                    config.is_usable(),
                    "{provider} url={url:?} model={model:?}"
                );
            }
        }
    }
}

#[test]
fn anthropic_needs_a_model_but_never_a_base_url() {
    assert!(is_configured("anthropic", "", "claude-sonnet-5"));
    assert!(!is_configured("anthropic", "", ""));
    assert!(!is_configured(
        "anthropic",
        "https://api.anthropic.com",
        "  "
    ));
    assert!(!shows_base_url("anthropic"));
    assert!(shows_base_url("ollama"));
}

#[test]
fn switching_to_a_never_configured_provider_prefills_its_preset_url() {
    let switched = profile_for_switch("lmstudio", None);
    assert_eq!(switched.base_url, "http://localhost:1234/v1");
    assert_eq!(switched.model, "");
    assert_eq!(switched.api_key, "");
}

#[test]
fn custom_and_anthropic_prefill_nothing() {
    assert_eq!(profile_for_switch("custom", None).base_url, "");
    assert_eq!(profile_for_switch("anthropic", None).base_url, "");
}

#[test]
fn switching_back_restores_what_was_saved_for_that_provider() {
    let saved = profile("http://gpu-box:11434/v1", "llama3", "secret");
    let switched = profile_for_switch("ollama", Some(&saved));
    assert_eq!(switched.base_url, "http://gpu-box:11434/v1");
    assert_eq!(switched.model, "llama3");
    assert_eq!(switched.api_key, "secret");
}

#[test]
fn a_saved_profile_with_a_blank_url_still_gets_the_preset_url() {
    let saved = profile("", "llama3", "");
    let switched = profile_for_switch("openai", Some(&saved));
    assert_eq!(switched.base_url, "https://api.openai.com/v1");
    assert_eq!(switched.model, "llama3");
}

#[test]
fn a_draft_trims_the_form_and_keeps_every_other_providers_details() {
    let mut initial = FormatConfig {
        enabled: false,
        provider: "ollama".into(),
        profiles: HashMap::new(),
    };
    initial.profiles.insert(
        "openai".into(),
        profile("https://api.openai.com/v1", "gpt", "k"),
    );

    let next = draft(
        &initial,
        "lmstudio",
        &profile("  http://x/v1 ", " m ", " key\n"),
    );

    assert_eq!(next.provider, "lmstudio");
    assert!(!next.enabled, "drafting must not change enabled");
    let saved = &next.profiles["lmstudio"];
    assert_eq!(
        (
            saved.base_url.as_str(),
            saved.model.as_str(),
            saved.api_key.as_str()
        ),
        ("http://x/v1", "m", "key")
    );
    assert_eq!(next.profiles["openai"].model, "gpt");
    assert_eq!(initial.provider, "ollama", "the original is untouched");
}

#[test]
fn every_preset_has_a_unique_id_and_a_logo() {
    for (i, preset) in PRESETS.iter().enumerate() {
        assert!(
            Vendor::from_id(preset.id).is_some(),
            "{} has no logo",
            preset.id
        );
        assert!(
            PRESETS[i + 1..].iter().all(|other| other.id != preset.id),
            "{} is listed twice",
            preset.id
        );
    }
}

#[test]
fn an_unknown_provider_falls_back_to_the_first_preset_and_its_own_label() {
    assert_eq!(preset("gone").id, PRESETS[0].id);
    assert_eq!(label("gone"), "gone");
    assert_eq!(label("openai"), "OpenAI");
}
