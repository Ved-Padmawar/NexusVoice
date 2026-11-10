use super::{needs_space, Before};

#[test]
fn a_space_separates_words_but_never_doubles_or_follows_an_opener() {
    assert!(needs_space(Before::Char('d'), "Hello"));
    assert!(needs_space(Before::Char('.'), "Hello"));
    assert!(!needs_space(Before::Char(' '), "Hello"));
    assert!(!needs_space(Before::Char('\n'), "Hello"));
    assert!(!needs_space(Before::Char('('), "Hello"));
    assert!(!needs_space(Before::Char('“'), "Hello"));
    assert!(!needs_space(Before::Start, "Hello"));
    assert!(!needs_space(Before::Unknown, "Hello"));
    assert!(!needs_space(Before::Char('d'), ", and so on"));
}
