use super::*;

use crate::inference::catalog;

#[test]
fn every_catalog_language_has_a_name() {
    // A code without a name shows up raw in the picker and can't be searched.
    for model in catalog::all() {
        for code in &model.languages {
            assert!(
                catalog::language_name(primary_of(code)).is_some(),
                "{}: no language_names entry for {code}",
                model.id
            );
        }
    }
}

#[test]
fn every_model_lists_its_languages() {
    // An empty list would offer nothing but auto-detect in the picker.
    for model in catalog::all() {
        assert!(!model.languages.is_empty(), "{}: no languages", model.id);
    }
}

#[test]
fn unset_resolves_to_the_default_not_auto_detect() {
    // Auto-detect must be an explicit choice: it is what mixes languages
    // mid-sentence on the streaming path.
    assert_eq!(resolve(None), Some(DEFAULT));
}

#[test]
fn auto_sentinel_resolves_to_no_hint() {
    assert_eq!(resolve(Some(AUTO)), None);
}

#[test]
fn a_code_passes_through_unchanged() {
    // The engine validates against the model, so resolve must not rewrite codes.
    assert_eq!(resolve(Some("ja")), Some("ja"));
    assert_eq!(resolve(Some("en-GB")), Some("en-GB"));
}

#[test]
fn a_locale_is_named_by_its_primary_subtag() {
    assert_eq!(display_name("de-DE", false), "German");
    assert_eq!(display_name("en-GB", true), "English (GB)");
    assert_eq!(display_name("te", false), "Telugu");
    assert_eq!(display_name("yue", false), "Cantonese");
    assert_eq!(primary_of("de-DE"), "de");
    assert_eq!(primary_of("de"), "de");
}
