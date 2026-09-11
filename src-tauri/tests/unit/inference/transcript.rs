use super::{join_words, Word};

fn w(text: &str) -> Word {
    Word {
        text: text.to_string(),
        end_cs: None,
    }
}

#[test]
fn text_joins_words_with_spaces() {
    assert_eq!(join_words(&[w(" hello"), w(" world")]), "hello world");
}

#[test]
fn text_attaches_punctuation_to_previous_word() {
    assert_eq!(
        join_words(&[w(" hello"), w(","), w(" world")]),
        "hello, world"
    );
}

#[test]
fn text_skips_blank_words_without_double_spacing() {
    assert_eq!(
        join_words(&[w("hello"), w("   "), w("world")]),
        "hello world"
    );
}

#[test]
fn an_apostrophe_fragment_attaches_to_its_word() {
    // Whisper splits contractions into separate word rows; spacing them would
    // produce "don 't" in the pasted transcript.
    assert_eq!(join_words(&[w(" don"), w("'t")]), "don't");
    assert_eq!(join_words(&[w(" it"), w("'s"), w(" fine")]), "it's fine");
}

#[test]
fn every_attaching_punctuation_mark_is_handled() {
    for mark in [",", ".", "!", "?", ";", ":"] {
        assert_eq!(
            join_words(&[w(" word"), w(mark)]),
            format!("word{mark}"),
            "{mark} should attach"
        );
    }
}

#[test]
fn an_opening_bracket_takes_its_own_space() {
    // Only trailing punctuation attaches; an opening mark starts a new token.
    assert_eq!(join_words(&[w(" he"), w("(sic)")]), "he (sic)");
}

#[test]
fn leading_punctuation_does_not_start_the_transcript_with_a_space() {
    assert_eq!(join_words(&[w(","), w(" hello")]), ", hello");
}

#[test]
fn an_empty_word_list_joins_to_an_empty_string() {
    assert_eq!(join_words(&[]), "");
    assert_eq!(join_words(&[w("  "), w("\t")]), "");
}

// ── strip_leading_dashes ───────────────────────────────────────────────
// Whisper renders a short utterance as dialogue ("- Hello."), so the transcript
// arrives with a dash the user never spoke.

use super::strip_leading_dashes;

#[test]
fn a_leading_dash_and_its_space_are_removed() {
    assert_eq!(strip_leading_dashes("- Hello there."), "Hello there.");
}

#[test]
fn every_dash_character_whisper_emits_is_handled() {
    // Hyphen, en dash and em dash all appear depending on the model.
    assert_eq!(strip_leading_dashes("- one"), "one");
    assert_eq!(strip_leading_dashes("– two"), "two");
    assert_eq!(strip_leading_dashes("— three"), "three");
}

#[test]
fn a_run_of_dashes_and_whitespace_is_removed() {
    assert_eq!(strip_leading_dashes("  --  — Hello"), "Hello");
    assert_eq!(strip_leading_dashes("\n- \t- Hello"), "Hello");
}

#[test]
fn an_interior_dash_is_real_punctuation_and_survives() {
    // Hyphenated words and mid-sentence dashes are things the user said.
    assert_eq!(
        strip_leading_dashes("well-known state-of-the-art work"),
        "well-known state-of-the-art work"
    );
    assert_eq!(
        strip_leading_dashes("I thought — briefly — about it"),
        "I thought — briefly — about it"
    );
}

#[test]
fn a_trailing_dash_is_left_alone() {
    // Only the leading run is hallucination; a trailing dash is the speaker
    // trailing off.
    assert_eq!(strip_leading_dashes("wait -"), "wait -");
}

#[test]
fn a_transcript_of_nothing_but_dashes_collapses_to_empty() {
    // Pure hallucination from silence. Empty is what tells the caller nothing
    // was said, so it must not survive as stray punctuation to paste.
    assert_eq!(strip_leading_dashes("- - —"), "");
    assert_eq!(strip_leading_dashes("   "), "");
    assert_eq!(strip_leading_dashes(""), "");
}

#[test]
fn ordinary_text_is_returned_untouched() {
    let text = "The quick brown fox.";
    assert_eq!(strip_leading_dashes(text), text);
}

#[test]
fn a_leading_minus_on_a_number_is_also_stripped() {
    // A known, accepted trade-off: dictating "minus five" as "-5" loses the
    // sign. Dialogue dashes are far more common than a leading signed number,
    // and this pins the behaviour so the trade-off is a decision, not a
    // surprise.
    assert_eq!(strip_leading_dashes("-5 degrees"), "5 degrees");
}
