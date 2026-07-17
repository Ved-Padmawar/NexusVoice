use super::*;
use crate::inference::{Hypothesis, Word};

fn word(text: &str, start_ms: i64, end_ms: i64) -> Word {
    Word {
        text: text.to_string(),
        start_ms,
        end_ms,
        probability: 0.9,
    }
}

/// Words on a 100ms grid starting at `start_ms`.
fn hyp(words: &[(&str, i64)]) -> Hypothesis {
    Hypothesis {
        words: words
            .iter()
            .map(|(t, start)| word(t, *start, start + 100))
            .collect(),
    }
}

#[test]
fn nothing_is_confirmed_from_a_single_hypothesis() {
    let mut s = Stabilizer::new();
    s.observe(&hyp(&[("hello", 0), ("world", 100)]), 0);
    // One decode is not agreement — everything stays tentative.
    assert_eq!(s.commit_watermark_ms(), 0);
}

#[test]
fn two_agreeing_hypotheses_confirm_the_common_prefix() {
    let mut s = Stabilizer::new();
    s.observe(&hyp(&[("hello", 0), ("world", 100), ("today", 200)]), 0);
    s.observe(&hyp(&[("hello", 0), ("world", 100), ("today", 200)]), 0);
    // Only "hello" clears the edge guard: the newest 200ms of audio is held back
    // as unstable. "world" and "today" stay tentative until a later decode extends
    // past them — or until finish() commits them.
    assert_eq!(s.commit_watermark_ms(), 100);
    assert_eq!(s.finish(), "hello world today");
}

#[test]
fn later_decodes_extend_the_edge_and_confirm_earlier_words() {
    let mut s = Stabilizer::new();
    s.observe(&hyp(&[("hello", 0), ("world", 100), ("today", 200)]), 0);
    s.observe(&hyp(&[("hello", 0), ("world", 100), ("today", 200)]), 0);
    assert_eq!(s.commit_watermark_ms(), 100);

    // More audio arrives: the edge moves right, so "world"/"today" leave the guard
    // zone and can now be confirmed.
    let extended = hyp(&[
        ("hello", 0),
        ("world", 100),
        ("today", 200),
        ("and", 300),
        ("tomorrow", 400),
    ]);
    s.observe(&extended, 0);
    s.observe(&extended, 0);
    assert!(
        s.commit_watermark_ms() >= 300,
        "edge advanced, earlier words confirmed"
    );
}

#[test]
fn divergence_stops_confirmation_at_the_disagreement() {
    let mut s = Stabilizer::new();
    s.observe(
        &hyp(&[("meet", 0), ("up", 100), ("soon", 200), ("today", 300)]),
        0,
    );
    s.observe(
        &hyp(&[("meet", 0), ("up", 100), ("noon", 200), ("today", 300)]),
        0,
    );
    // "meet up" agrees; "soon" vs "noon" does not, so commitment stops there.
    assert_eq!(s.commit_watermark_ms(), 200);
}

#[test]
fn a_genuinely_repeated_phrase_is_kept_twice() {
    // The bug the text stitcher could not fix: identical words at different audio
    // times are two real occurrences, not a boundary duplicate.
    let mut s = Stabilizer::new();
    let h = hyp(&[
        ("no", 0),
        ("no", 100),
        ("no", 200),
        ("stop", 300),
        ("there", 400),
    ]);
    s.observe(&h, 0);
    s.observe(&h, 0);
    assert_eq!(s.finish(), "no no no stop there");
}

#[test]
fn the_same_word_at_the_same_time_is_not_duplicated() {
    let mut s = Stabilizer::new();
    let h = hyp(&[("hello", 0), ("world", 100), ("again", 200)]);
    s.observe(&h, 0);
    s.observe(&h, 0);
    s.observe(&h, 0);
    assert_eq!(s.finish(), "hello world again");
}

#[test]
fn timestamp_jitter_within_tolerance_still_agrees() {
    let mut s = Stabilizer::new();
    s.observe(&hyp(&[("hello", 0), ("world", 100), ("today", 200)]), 0);
    // Same words, times nudged — whisper is not frame-exact between decodes.
    s.observe(&hyp(&[("hello", 50), ("world", 150), ("today", 250)]), 0);
    assert!(s.commit_watermark_ms() > 0);
}

#[test]
fn timestamp_drift_beyond_tolerance_blocks_agreement() {
    let mut s = Stabilizer::new();
    s.observe(&hyp(&[("hello", 0), ("world", 100)]), 0);
    // Same text an entire second later is different speech, not the same word.
    s.observe(&hyp(&[("hello", 1000), ("world", 1100)]), 0);
    assert_eq!(s.commit_watermark_ms(), 0);
}

#[test]
fn punctuation_and_case_do_not_block_agreement() {
    let mut s = Stabilizer::new();
    s.observe(&hyp(&[("Hello,", 0), ("world", 100), ("today", 200)]), 0);
    s.observe(&hyp(&[("hello", 0), ("World.", 100), ("today", 200)]), 0);
    assert!(s.commit_watermark_ms() > 0);
}

#[test]
fn an_empty_hypothesis_does_not_move_the_watermark() {
    let mut s = Stabilizer::new();
    s.observe(&hyp(&[("hello", 0), ("world", 100), ("today", 200)]), 0);
    let before = s.commit_watermark_ms();
    s.observe(&Hypothesis::default(), 0);
    assert_eq!(s.commit_watermark_ms(), before);
    // The pending buffer survives, so the next real decode can still confirm it.
    s.observe(&hyp(&[("hello", 0), ("world", 100), ("today", 200)]), 0);
    assert_eq!(s.finish(), "hello world today");
}

#[test]
fn finish_commits_tentative_words_so_the_tail_is_not_dropped() {
    let mut s = Stabilizer::new();
    // Only ever seen once — never confirmed by agreement.
    s.observe(&hyp(&[("last", 0), ("words", 100)]), 0);
    assert_eq!(s.commit_watermark_ms(), 0);
    assert_eq!(
        s.finish(),
        "last words",
        "release must commit tentative words — no chunk is coming to confirm them"
    );
}

#[test]
fn prompt_excludes_text_whose_audio_is_still_in_the_window() {
    let mut s = Stabilizer::new();
    let h = hyp(&[
        ("alpha", 0),
        ("bravo", 100),
        ("charlie", 200),
        ("delta", 300),
    ]);
    s.observe(&h, 0);
    s.observe(&h, 0);
    // A window starting at 200ms must not be prompted with words heard at/after it.
    let prompt = s.prompt_before(200, 30);
    assert!(prompt.contains("alpha"), "prompt: {prompt}");
    assert!(
        !prompt.contains("charlie") && !prompt.contains("delta"),
        "prompt leaked words the decoder is about to hear again: {prompt}"
    );
}

#[test]
fn prompt_is_capped_to_the_most_recent_words() {
    let mut s = Stabilizer::new();
    let words: Vec<(&str, i64)> = vec![
        ("one", 0),
        ("two", 100),
        ("three", 200),
        ("four", 300),
        ("five", 400),
        ("six", 500),
    ];
    let h = hyp(&words);
    s.observe(&h, 0);
    s.observe(&h, 0);
    // The edge guard leaves "five"/"six" unconfirmed, so the newest *confirmed*
    // words are "three four" — the prompt only ever draws from confirmed text.
    let prompt = s.prompt_before(10_000, 2);
    assert_eq!(prompt, "three four");
}

#[test]
fn prompt_is_empty_before_anything_is_confirmed() {
    let s = Stabilizer::new();
    assert_eq!(s.prompt_before(10_000, 30), "");
}

#[test]
fn sliding_windows_do_not_drop_earlier_speech() {
    // The real pipeline: chunk 1 decodes 0-8s, chunk 2 re-decodes only from the
    // 1.5s overlap (6.5s). Words chunk 2 never heard must survive.
    let mut s = Stabilizer::new();
    s.observe(
        &hyp(&[
            ("alpha", 0),
            ("bravo", 1000),
            ("charlie", 6500),
            ("delta", 6600),
        ]),
        0,
    );
    s.observe(
        &hyp(&[("charlie", 6500), ("delta", 6600), ("echo", 6700)]),
        6500,
    );
    let out = s.finish();
    assert!(out.contains("alpha"), "earlier speech dropped: {out}");
    assert!(out.contains("bravo"), "earlier speech dropped: {out}");
    assert_eq!(out, "alpha bravo charlie delta echo");
}

#[test]
fn a_word_the_window_covers_is_still_replaceable() {
    // Inside the window, disagreement must still win — promotion applies only to
    // audio the decode couldn't hear.
    let mut s = Stabilizer::new();
    s.observe(&hyp(&[("early", 0), ("soon", 6500), ("after", 6600)]), 0);
    s.observe(
        &hyp(&[("noon", 6500), ("after", 6600), ("end", 6700)]),
        6500,
    );
    let out = s.finish();
    assert!(out.contains("early"), "pre-window word dropped: {out}");
    assert!(out.contains("noon"), "newer hypothesis should win: {out}");
    assert!(!out.contains("soon"), "stale contested word kept: {out}");
}
