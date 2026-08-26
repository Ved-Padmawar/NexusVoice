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
