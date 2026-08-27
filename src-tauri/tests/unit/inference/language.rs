use super::*;

#[test]
fn auto_is_not_a_language() {
    assert!(!is_supported(AUTO));
}

#[test]
fn every_code_is_unique_and_two_letters() {
    let mut codes: Vec<&str> = LANGUAGES.iter().map(|l| l.code).collect();
    let total = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), total, "duplicate language code in LANGUAGES");
    assert!(LANGUAGES.iter().all(|l| l.code.len() == 2));
}

#[test]
fn default_is_offered_bare_or_as_a_locale() {
    assert!(is_supported(DEFAULT));
    assert!(is_supported("en-GB"));
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
fn a_code_outside_the_table_still_passes_through() {
    // Models may advertise codes the table has no entry for; the engine
    // validates against the model, so resolve must not rewrite them.
    assert_eq!(resolve(Some("yue")), Some("yue"));
}

#[test]
fn a_known_code_passes_through() {
    assert_eq!(resolve(Some("ja")), Some("ja"));
    assert_eq!(resolve(Some("de")), Some("de"));
}

#[test]
fn a_locale_is_named_by_its_primary_subtag() {
    assert_eq!(display_name("de-DE", false), "German");
    assert_eq!(display_name("en-GB", true), "English (GB)");
    assert_eq!(display_name("ja", false), "Japanese");
    // No table entry — the bare code stands in.
    assert_eq!(display_name("yue", false), "yue");
    assert_eq!(primary_of("de-DE"), "de");
    assert_eq!(primary_of("de"), "de");
}
