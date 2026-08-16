use super::{common_prefix_len, from_ms, normalize_word, prompt_tail, push_text, StreamingSession};
use crate::inference::transcript::{TimedSegment, Word};

fn word(text: &str, end_cs: i64) -> Word {
    Word {
        text: text.to_string(),
        end_cs: Some(end_cs),
    }
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| normalize_word(w)).collect()
}

#[test]
fn agreement_ignores_case_and_punctuation() {
    // Whisper flips "Okay," ↔ "okay" between decodes; that must still agree.
    let a = words(&["Okay,", "so", "we", "start"]);
    let b = words(&["okay", "so", "we", "started"]);
    assert_eq!(common_prefix_len(&a, &b), 3);
}

#[test]
fn agreement_is_empty_on_disjoint_hypotheses() {
    let a = words(&["hello", "world"]);
    let b = words(&["goodbye", "world"]);
    assert_eq!(common_prefix_len(&a, &b), 0);
}

#[test]
fn agreement_handles_unequal_lengths() {
    let a = words(&["one", "two"]);
    let b = words(&["one", "two", "three"]);
    assert_eq!(common_prefix_len(&a, &b), 2);
    assert_eq!(common_prefix_len(&b, &a), 2);
}

#[test]
fn normalize_strips_everything_but_alphanumerics() {
    assert_eq!(normalize_word("Okay,"), "okay");
    assert_eq!(normalize_word("it's"), "its");
    assert_eq!(normalize_word("—"), "");
}

#[test]
fn prompt_tail_keeps_only_the_last_words() {
    let committed = (0..40).map(|i| i.to_string()).collect::<Vec<_>>().join(" ");
    let tail = prompt_tail(&committed);
    assert_eq!(tail.split_whitespace().count(), 30);
    assert!(tail.starts_with("10 "));
    assert!(tail.ends_with(" 39"));
}

#[test]
fn prompt_tail_of_empty_text_is_empty() {
    assert_eq!(prompt_tail(""), "");
}

#[test]
fn push_text_separates_with_a_single_space() {
    let mut out = String::new();
    push_text(&mut out, "hello");
    push_text(&mut out, " world ");
    push_text(&mut out, "");
    assert_eq!(out, "hello world");
}

#[test]
fn from_ms_maps_to_native_rate() {
    assert_eq!(from_ms(1000, 48_000), 48_000);
    assert_eq!(from_ms(500, 44_100), 22_050);
    assert_eq!(from_ms(-5, 48_000), 0);
}

#[test]
fn force_trim_cuts_a_single_growing_segment_at_a_word_boundary() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![
            word(" one", 400),
            word(" two", 800),
            word(" three", 1200),
            word(" four", 1600),
            word(" five", 2000),
        ],
        end_ms: 2000,
    }];
    session.confirmed_words = 4;
    let total_len = 20 * native_rate as usize;

    session.force_trim(total_len, native_rate);

    assert_eq!(session.committed, "one two three four");
    assert_eq!(session.window_start, from_ms(1600 * 10, native_rate));
    assert_eq!(session.confirmed_words, 0);
    assert_eq!(session.segments[0].words.len(), 1);
    assert_eq!(session.segments[0].words[0].text, " five");
}

#[test]
fn force_trim_is_a_noop_when_nothing_is_confirmed() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![word(" one", 400), word(" two", 800)],
        end_ms: 2000,
    }];
    session.confirmed_words = 0;
    let total_len = 20 * native_rate as usize;

    session.force_trim(total_len, native_rate);

    assert_eq!(session.committed, "");
    assert_eq!(session.window_start, 0);
}

#[test]
fn force_trim_is_a_noop_without_dtw_timestamps() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![Word {
            text: " hello".to_string(),
            end_cs: None,
        }],
        end_ms: 2000,
    }];
    session.confirmed_words = 1;
    let total_len = 20 * native_rate as usize;

    session.force_trim(total_len, native_rate);

    assert_eq!(session.committed, "");
    assert_eq!(session.window_start, 0);
}

#[test]
fn force_trim_is_a_noop_when_it_would_leave_too_little_window() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![word(" hello", 1990)],
        end_ms: 2000,
    }];
    session.confirmed_words = 1;
    let total_len = 20 * native_rate as usize;

    session.force_trim(total_len, native_rate);

    assert_eq!(session.committed, "");
    assert_eq!(session.window_start, 0);
}
