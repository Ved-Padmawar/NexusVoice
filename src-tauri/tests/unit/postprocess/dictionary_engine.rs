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
    let e = engine(vec![entry(1, "api", "API"), entry(2, "pdf", "PDF")]);
    for word in &["py", "io", "pf"] {
        assert!(
            e.correct(word).is_none(),
            "short \"{word}\" should not fuzzy"
        );
    }
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
