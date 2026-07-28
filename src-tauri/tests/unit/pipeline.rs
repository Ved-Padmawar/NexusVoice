use super::{common_prefix_len, from_ms, normalize_word, prompt_tail, push_text};

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
