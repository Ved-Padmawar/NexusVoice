//! A model family with no logo, or a logo path that points nowhere, renders a
//! blank slot: no error is raised anywhere.

use std::path::Path;

use super::Vendor;

const ALL: [Vendor; 9] = [
    Vendor::OpenAi,
    Vendor::Anthropic,
    Vendor::Ollama,
    Vendor::LmStudio,
    Vendor::OpenRouter,
    Vendor::Nvidia,
    Vendor::Qwen,
    Vendor::Moonshine,
    Vendor::Custom,
];

#[test]
fn hyphens_are_ignored_when_resolving_a_family() {
    assert_eq!(Vendor::for_family("qwen3-asr"), Some(Vendor::Qwen));
    assert_eq!(Vendor::for_family("qwen3asr"), Some(Vendor::Qwen));
}

#[test]
fn an_unknown_family_has_no_vendor() {
    assert_eq!(Vendor::for_family("not-a-family"), None);
    assert_eq!(Vendor::for_family(""), None);
}

#[test]
fn every_family_the_catalog_ships_has_a_logo() {
    let models = crate::inference::catalog::all();
    assert!(!models.is_empty());
    for model in models {
        let vendor = Vendor::for_family(&model.family)
            .unwrap_or_else(|| panic!("{}: family {:?} has no vendor", model.id, model.family));
        assert!(vendor.asset().is_some(), "{}: {vendor:?} has no logo", model.id);
    }
}

#[test]
fn every_logo_path_exists_in_the_bundled_assets() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    for vendor in ALL {
        if let Some(path) = vendor.asset() {
            assert!(assets.join(path).is_file(), "{vendor:?}: {path} is missing");
        }
    }
}

#[test]
fn provider_ids_resolve_to_their_own_vendor() {
    for (id, vendor) in [
        ("ollama", Vendor::Ollama),
        ("lmstudio", Vendor::LmStudio),
        ("openai", Vendor::OpenAi),
        ("openrouter", Vendor::OpenRouter),
        ("anthropic", Vendor::Anthropic),
        ("custom", Vendor::Custom),
    ] {
        assert_eq!(Vendor::from_id(id), Some(vendor), "{id}");
    }
    assert_eq!(Vendor::from_id("some-new-provider"), None);
}
