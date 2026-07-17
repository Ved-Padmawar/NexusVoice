use super::{centis_to_ms, merge_subword_tokens, strip_hallucination_tokens};
use crate::inference::transcript::Word;

fn tok(text: &str, start_ms: i64, end_ms: i64, probability: f32) -> Word {
    Word {
        text: text.to_string(),
        start_ms,
        end_ms,
        probability,
    }
}

#[test]
fn centiseconds_convert_to_milliseconds() {
    assert_eq!(centis_to_ms(0), 0);
    assert_eq!(centis_to_ms(123), 1230);
}

#[test]
fn subword_fragments_merge_into_one_word() {
    // "unbelievable" arrives as three BPE pieces; only the first has a space.
    let merged = merge_subword_tokens(vec![
        tok(" unbe", 0, 100, 0.9),
        tok("lie", 100, 200, 0.7),
        tok("vable", 200, 300, 0.8),
    ]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].text, "unbelievable");
    assert_eq!(merged[0].start_ms, 0);
    assert_eq!(merged[0].end_ms, 300, "interval spans all fragments");
    assert!(
        (merged[0].probability - 0.7).abs() < f32::EPSILON,
        "word takes its weakest fragment's confidence"
    );
}

#[test]
fn leading_space_starts_a_new_word() {
    let merged = merge_subword_tokens(vec![
        tok(" hello", 0, 100, 0.9),
        tok(" world", 100, 200, 0.9),
    ]);
    assert_eq!(
        merged.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        vec!["hello", "world"]
    );
}

#[test]
fn punctuation_folds_into_the_preceding_word() {
    // Punctuation has no leading space, so it rides along with its word.
    let merged = merge_subword_tokens(vec![tok(" hello", 0, 100, 0.9), tok(",", 100, 110, 0.9)]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].text, "hello,");
    assert_eq!(merged[0].end_ms, 110);
}

#[test]
fn first_token_without_a_space_still_starts_a_word() {
    let merged = merge_subword_tokens(vec![tok("hello", 0, 100, 0.9)]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].text, "hello");
}

#[test]
fn whitespace_only_tokens_are_dropped() {
    let merged = merge_subword_tokens(vec![tok(" ", 0, 10, 0.9), tok(" hi", 10, 100, 0.9)]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].text, "hi");
}

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
