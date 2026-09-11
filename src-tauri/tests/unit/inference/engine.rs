use super::{split_segment_text, strip_hallucination_tokens};

#[test]
fn strips_embedded_blank_audio_token() {
    assert_eq!(
        strip_hallucination_tokens("[Blank_Audio] so anyway we continue"),
        " so anyway we continue"
    );
}

#[test]
fn strips_token_only_segment_to_empty() {
    assert!(strip_hallucination_tokens(" [BLANK_AUDIO] ")
        .trim()
        .is_empty());
}

#[test]
fn strips_multiple_tokens_and_collapses_spaces() {
    assert_eq!(
        strip_hallucination_tokens("hello [noise] world [SILENCE]"),
        "hello world "
    );
}

#[test]
fn leaves_normal_text_untouched() {
    let s = "the audio was blank but fine";
    assert_eq!(strip_hallucination_tokens(s), s);
}

#[test]
fn segment_text_splits_into_words_carrying_the_segment_end() {
    let words = split_segment_text("hello there world", 2_500);
    assert_eq!(
        words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        vec!["hello", "there", "world"]
    );
    // Milliseconds convert to centiseconds for the pipeline's trim logic.
    assert!(words.iter().all(|w| w.end_cs == Some(250)));
}

#[test]
fn segment_text_drops_hallucination_tokens() {
    let words = split_segment_text("hello [noise] world", 1_000);
    assert_eq!(
        words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        vec!["hello", "world"]
    );
}

#[test]
fn segment_text_with_no_timestamp_leaves_end_unset() {
    let words = split_segment_text("hello", -1);
    assert_eq!(words.len(), 1);
    assert_eq!(words[0].end_cs, None);
}

#[test]
fn empty_segment_text_yields_no_words() {
    assert!(split_segment_text("   ", 500).is_empty());
}

#[test]
fn every_hallucination_token_is_stripped() {
    // All five are emitted by whisper on silence; missing one pastes it into
    // the user's document.
    for token in [
        "[BLANK_AUDIO]",
        "[SILENCE]",
        "[NOISE]",
        "[MUSIC]",
        "(MUSIC)",
    ] {
        let out = strip_hallucination_tokens(&format!("hello {token} world"));
        assert!(
            !out.to_lowercase().contains(&token.to_lowercase()),
            "{token} survived: {out}"
        );
        assert!(out.contains("hello") && out.contains("world"), "{out}");
    }
}

#[test]
fn token_stripping_is_case_insensitive() {
    for variant in ["[blank_audio]", "[Blank_Audio]", "[BLANK_AUDIO]"] {
        assert!(
            strip_hallucination_tokens(variant).trim().is_empty(),
            "{variant} survived"
        );
    }
}

#[test]
fn repeated_tokens_are_all_removed() {
    // The inner loop exists for this; a single pass would leave the rest.
    let out = strip_hallucination_tokens("[noise][noise][noise] speech [noise]");
    assert!(!out.to_lowercase().contains("noise"), "{out}");
    assert_eq!(out.trim(), "speech");
}

#[test]
fn stripping_never_leaves_a_doubled_space() {
    let out = strip_hallucination_tokens("one [noise] two [silence] three");
    assert!(!out.contains("  "), "doubled space left behind: {out:?}");
    assert_eq!(
        out.split_whitespace().collect::<Vec<_>>(),
        ["one", "two", "three"]
    );
}

// ── resolve_language ───────────────────────────────────────────────────
// The decoder rejects an unadvertised language code outright, failing *every*
// decode — so a bad resolution here is not a subtle quality loss, it is a model
// that transcribes nothing, or one that transcribes in the wrong language.

use super::resolve_language;

fn advertised(codes: &[&str]) -> Vec<String> {
    codes.iter().map(|c| (*c).to_string()).collect()
}

#[test]
fn no_requested_language_means_auto_detect() {
    // `None` is an explicit choice and must never be filled in with a default.
    assert_eq!(
        resolve_language(&advertised(&["en-US", "de-DE"]), None),
        None
    );
    assert_eq!(resolve_language(&[], None), None);
}

#[test]
fn an_exactly_advertised_code_is_used_as_is() {
    let codes = advertised(&["en-US", "de-DE", "ja-JP"]);
    assert_eq!(
        resolve_language(&codes, Some("de-DE")),
        Some("de-DE".to_string())
    );
}

#[test]
fn a_bare_code_resolves_to_the_models_locale_for_it() {
    // Settings store bare codes; models advertise BCP-47. Passing "de" straight
    // through to a model that only knows "de-DE" fails every decode.
    let codes = advertised(&["en-GB", "de-DE", "ja-JP"]);
    assert_eq!(
        resolve_language(&codes, Some("de")),
        Some("de-DE".to_string())
    );
    assert_eq!(
        resolve_language(&codes, Some("en")),
        Some("en-GB".to_string())
    );
}

#[test]
fn a_model_advertising_nothing_accepts_any_hint() {
    // An empty capability list means "unconstrained", not "supports nothing" —
    // treating it as the latter would pin every such model to English.
    assert_eq!(resolve_language(&[], Some("ja")), Some("ja".to_string()));
    assert_eq!(resolve_language(&[], Some("yue")), Some("yue".to_string()));
}

#[test]
fn an_unsupported_language_falls_back_to_the_models_english() {
    // Better to transcribe in English than to fail every decode.
    let codes = advertised(&["en-US", "de-DE"]);
    assert_eq!(
        resolve_language(&codes, Some("ja")),
        Some("en-US".to_string())
    );
}

#[test]
fn an_unsupported_language_auto_detects_when_the_model_has_no_english() {
    // No English to fall back to, so hand the decision to the model rather than
    // sending a code it will reject.
    let codes = advertised(&["de-DE", "fr-FR"]);
    assert_eq!(resolve_language(&codes, Some("ja")), None);
}

#[test]
fn the_fallback_prefers_english_by_primary_subtag() {
    // Any English locale will do; the model decides the accent.
    for locale in ["en-US", "en-GB", "en-AU"] {
        let codes = advertised(&[locale, "de-DE"]);
        assert_eq!(
            resolve_language(&codes, Some("ko")),
            Some(locale.to_string()),
            "failed for {locale}"
        );
    }
}

#[test]
fn an_exact_match_is_preferred_over_a_locale_rewrite() {
    // A model advertising both the bare code and a locale must keep the bare one
    // the user actually asked for.
    let codes = advertised(&["en", "en-GB"]);
    assert_eq!(resolve_language(&codes, Some("en")), Some("en".to_string()));
}
