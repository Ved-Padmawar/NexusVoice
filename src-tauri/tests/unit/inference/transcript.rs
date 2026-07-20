use super::{join_words, Word};

fn w(text: &str) -> Word {
    Word {
        text: text.to_string(),
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
