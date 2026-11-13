use super::{needs_space, Before};

#[test]
fn a_space_separates_words_but_never_doubles_or_follows_an_opener() {
    assert!(needs_space(Before::Char('d'), "Hello", None));
    assert!(needs_space(Before::Char('.'), "Hello", None));
    assert!(!needs_space(Before::Char(' '), "Hello", None));
    assert!(!needs_space(Before::Char('\n'), "Hello", None));
    assert!(!needs_space(Before::Char('('), "Hello", None));
    assert!(!needs_space(Before::Char('“'), "Hello", None));
    assert!(!needs_space(Before::Start, "Hello", None));
    assert!(!needs_space(Before::Char('d'), ", and so on", None));
}

#[test]
fn an_unreadable_caret_follows_our_previous_paste_into_the_same_window_only() {
    assert!(needs_space(Before::Unknown, "Hello", Some('d')));
    assert!(!needs_space(Before::Unknown, "Hello", Some('\n')));
    assert!(!needs_space(Before::Unknown, ", and", Some('d')));
    assert!(!needs_space(Before::Unknown, "Hello", None));
    assert!(!needs_space(Before::Start, "Hello", Some('d')));
    assert!(!needs_space(Before::Char(' '), "Hello", Some('d')));
}
