use super::{Hypothesis, Word};

fn w(text: &str, start_ms: i64, end_ms: i64) -> Word {
    Word {
        text: text.to_string(),
        start_ms,
        end_ms,
        probability: 0.9,
    }
}

#[test]
fn text_joins_words_with_spaces() {
    let h = Hypothesis {
        words: vec![w(" hello", 0, 100), w(" world", 100, 200)],
    };
    assert_eq!(h.text(), "hello world");
}

#[test]
fn text_attaches_punctuation_to_previous_word() {
    let h = Hypothesis {
        words: vec![w(" hello", 0, 100), w(",", 100, 110), w(" world", 110, 200)],
    };
    assert_eq!(h.text(), "hello, world");
}

#[test]
fn normalized_strips_edge_punctuation_but_keeps_internal() {
    assert_eq!(w("Hello,", 0, 1).normalized(), "hello");
    assert_eq!(w(" don't", 0, 1).normalized(), "don't");
    assert_eq!(w("well-known.", 0, 1).normalized(), "well-known");
}

#[test]
fn normalized_keeps_distinct_numbers_distinct() {
    // Agreement must never treat different numeric values as the same word.
    assert_eq!(w(" 42.", 0, 1).normalized(), "42");
    assert_ne!(w("42", 0, 1).normalized(), w("43", 0, 1).normalized());
    assert_eq!(w("3.14", 0, 1).normalized(), "3.14");
}
