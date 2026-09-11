use super::*;
use crate::database::models::dictionary::DictionaryEntry;

fn entry(id: i64, term: &str, replacement: &str) -> DictionaryEntry {
    DictionaryEntry {
        id,
        term: term.to_string(),
        replacement: replacement.to_string(),
        hits: 0,
        created_at: chrono::NaiveDateTime::default(),
    }
}

fn engine(entries: Vec<DictionaryEntry>) -> DictionaryCorrectionEngine {
    DictionaryCorrectionEngine::new(entries)
}

// ── Exact matches ─────────────────────────────────────────────────────
#[test]
fn exact_match_wins() {
    let e = engine(vec![entry(1, "teh", "the")]);
    let r = e.correct("teh").expect("hit");
    assert!(r.exact);
    assert_eq!(r.replacement, "the");
}

#[test]
fn exact_short_match() {
    let e = engine(vec![entry(1, "ui", "UI"), entry(2, "api", "API")]);
    assert_eq!(e.correct("ui").unwrap().replacement, "UI");
    assert_eq!(e.correct("api").unwrap().replacement, "API");
}

#[test]
fn mixed_case_input_exact_matches() {
    let e = engine(vec![entry(1, "api", "API"), entry(2, "python", "Python")]);
    assert_eq!(e.correct("Api").unwrap().replacement, "API");
    assert_eq!(e.correct("Python").unwrap().replacement, "Python");
}

// ── Fuzzy matches ──────────────────────────────────────────────────────
#[test]
fn fuzzy_one_edit_deletion() {
    let e = engine(vec![entry(1, "recieve", "receive")]);
    assert_eq!(e.correct("recive").unwrap().replacement, "receive");
}

#[test]
fn fuzzy_transposition() {
    let e = engine(vec![entry(1, "docker", "Docker")]);
    assert_eq!(e.correct("dcoker").unwrap().replacement, "Docker");
}

// ── Guards ────────────────────────────────────────────────────────────
#[test]
fn stopwords_never_corrected() {
    // Every word here is 4+ chars and sits one edit from a dictionary term, so
    // only the stopword guard can reject them — the length and max-distance
    // guards do not apply. With the guard gone, "i went home" would be
    // rewritten to "i won't home".
    let e = engine(vec![
        entry(1, "wont", "won't"),
        entry(2, "thier", "their"),
        entry(3, "wich", "which"),
    ]);
    for word in &["went", "their", "which"] {
        assert!(
            e.correct(word).is_none(),
            "stopword \"{word}\" should not correct"
        );
    }
}

#[test]
fn short_stopwords_are_also_left_alone() {
    let e = engine(vec![entry(1, "api", "API"), entry(2, "ui", "UI")]);
    for word in &["am", "on", "my", "the", "and", "in", "us", "go"] {
        assert!(
            e.correct(word).is_none(),
            "stopword \"{word}\" should not correct"
        );
    }
}

#[test]
fn short_words_no_fuzzy() {
    // "api" is 3 chars and exactly one edit from "apis", which the ratio-based
    // max distance would otherwise allow (len 3 -> max_dist 1). Only the
    // min-length guard rejects it, so this pins that guard specifically.
    let e = engine(vec![entry(1, "apis", "APIs"), entry(2, "pdfs", "PDFs")]);
    for word in &["api", "pdf"] {
        assert!(
            e.correct(word).is_none(),
            "short \"{word}\" should not fuzzy"
        );
    }
    // Sanity: the same entry does match once the input clears the length floor.
    assert_eq!(e.correct("apia").unwrap().replacement, "APIs");
}

#[test]
fn digit_tokens_skipped() {
    let e = engine(vec![entry(1, "api", "API")]);
    assert!(e.correct("v2").is_none());
    assert!(e.correct("mp3").is_none());
    assert!(e.correct("gpt4").is_none());
}

#[test]
fn all_uppercase_tokens_skipped() {
    let e = engine(vec![entry(1, "python", "Python")]);
    assert!(e.correct("PYTHON").is_none());
}

#[test]
fn ambiguous_match_skipped() {
    let e = engine(vec![
        entry(1, "docker", "Docker"),
        entry(2, "dockex", "Dockex"),
    ]);
    // "docke" is distance 1 from both — ambiguous
    assert!(e.correct("docke").is_none());
}

// ── apply_to_text ──────────────────────────────────────────────────────
#[test]
fn apply_to_text_corrects_words() {
    let e = engine(vec![entry(1, "teh", "the"), entry(2, "gonna", "going to")]);
    let (text, _) = e.apply_to_text("teh dog is gonna run");
    assert_eq!(text, "the dog is going to run");
}

#[test]
fn apply_to_text_preserves_punctuation() {
    let e = engine(vec![entry(1, "teh", "the")]);
    let (text, _) = e.apply_to_text("teh, dog.");
    assert_eq!(text, "the, dog.");
}

#[test]
fn apply_to_text_stopwords_unchanged() {
    let e = engine(vec![entry(1, "api", "API"), entry(2, "ui", "UI")]);
    let (text, _) = e.apply_to_text("i am on my way");
    assert_eq!(text, "i am on my way");
}

#[test]
fn apply_to_text_long_sentence() {
    let e = engine(vec![
        entry(1, "github", "GitHub"),
        entry(2, "api", "API"),
        entry(3, "json", "JSON"),
        entry(4, "url", "URL"),
    ]);
    let (text, _) =
        e.apply_to_text("so i was using the github api to fetch some json data from the url");
    assert_eq!(
        text,
        "so i was using the GitHub API to fetch some JSON data from the URL"
    );
}

#[test]
fn empty_dictionary_returns_text_unchanged() {
    let e = engine(vec![]);
    let (text, _) = e.apply_to_text("hello world");
    assert_eq!(text, "hello world");
}

#[test]
fn apply_to_text_returns_matched_terms() {
    let e = engine(vec![entry(1, "teh", "the"), entry(2, "gonna", "going to")]);
    let (_, terms) = e.apply_to_text("teh dog is gonna run");
    assert_eq!(terms, vec!["teh", "gonna"]);
}

// ── Phonetic fallback (Double Metaphone) ───────────────────────────────
// Catches sound-alike ASR errors Levenshtein misses. Fires only when no
// edit-distance match was found, and only on an unambiguous phonetic hit.

#[test]
fn a_sound_alike_misrecognition_is_corrected() {
    // "neksus" is 3 edits from "nexus" — well past max_dist — but the same
    // phonetically, which is exactly how ASR gets proper nouns wrong.
    let e = engine(vec![entry(1, "nexus", "Nexus")]);
    let r = e.correct("neksus").expect("phonetic hit");
    assert_eq!(r.replacement, "Nexus");
    assert!(!r.exact, "a phonetic match is not an exact match");
}

#[test]
fn an_ambiguous_phonetic_match_is_skipped() {
    // Two entries sounding alike means we cannot tell which was meant; leaving
    // the word alone beats silently picking the wrong one.
    let e = engine(vec![entry(1, "smith", "Smith"), entry(2, "smyth", "Smyth")]);
    assert!(e.correct("smythe").is_none());
}

#[test]
fn the_phonetic_fallback_respects_the_first_letter_constraint() {
    // "fone"/"phone" sound identical, but a first-letter jump is too risky a
    // rewrite to make automatically.
    let e = engine(vec![entry(1, "phone", "phone")]);
    assert!(e.correct("fone").is_none());
}

#[test]
fn a_word_unlike_every_entry_is_left_alone() {
    // Neither Levenshtein nor Double Metaphone should reach for a match here.
    let e = engine(vec![
        entry(1, "nexus", "Nexus"),
        entry(2, "docker", "Docker"),
    ]);
    assert!(e.correct("elephant").is_none());
    assert!(e.correct("zebra").is_none());
}

// ── First-letter constraint on fuzzy matching ─────────────────────────

#[test]
fn fuzzy_matching_will_not_change_the_first_letter() {
    // "bocker" is one edit from "docker", but correcting across the first letter
    // turns an unrelated word into a dictionary term.
    let e = engine(vec![entry(1, "docker", "Docker")]);
    assert!(e.correct("bocker").is_none());
}

// ── apply_to_text edges ───────────────────────────────────────────────

#[test]
fn apply_to_text_keeps_tokens_that_have_no_letters() {
    // Standalone numbers and symbols pass through untouched.
    let e = engine(vec![entry(1, "teh", "the")]);
    let (text, terms) = e.apply_to_text("42 -- teh 7");
    assert_eq!(text, "42 -- the 7");
    assert_eq!(terms, vec!["teh"]);
}

#[test]
fn apply_to_text_preserves_a_wrapping_quote_or_bracket() {
    // The prefix/suffix split must keep both sides of the word.
    let e = engine(vec![entry(1, "teh", "the")]);
    let (text, _) = e.apply_to_text("(teh) \"teh\"");
    assert_eq!(text, "(the) \"the\"");
}

#[test]
fn apply_to_text_reports_a_repeated_match_once_per_occurrence() {
    // Hit counting relies on this: three uses is three hits, not one.
    let e = engine(vec![entry(1, "api", "API")]);
    let (text, terms) = e.apply_to_text("the api and the api and the api");
    assert_eq!(text, "the API and the API and the API");
    assert_eq!(terms, vec!["api", "api", "api"]);
}
