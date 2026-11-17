//! Whether a pasted transcript needs a space in front of it. Pure, so the rule
//! is tested without a window.
//!
//! Each dictation is pasted on its own, so a second one would otherwise run
//! straight into the end of the first ("...worldHello"). The target's caret
//! decides. Only when the target cannot say (terminals) does the end of our
//! own previous paste into the same window stand in: any other guess would put
//! a stray space at the start of a new line.

/// A caret preceded by one of these wants no space: the text opens something.
const OPENERS: [char; 10] = ['(', '[', '{', '"', '\'', '“', '‘', '«', '‹', '„'];

/// A transcript starting with one of these attaches to what is before it.
const ATTACHES_BACKWARD: [char; 9] = ['.', ',', ';', ':', '!', '?', ')', ']', '}'];

/// What sits just before the caret in the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Before {
    Char(char),
    /// The caret is at the very start of the text.
    Start,
    /// The target did not say.
    Unknown,
}

/// `last_pasted_end` is the last character of our previous paste, given only
/// when it went into this same window.
pub fn needs_space(before: Before, text: &str, last_pasted_end: Option<char>) -> bool {
    let ((Before::Char(previous), _) | (Before::Unknown, Some(previous))) =
        (before, last_pasted_end)
    else {
        return false;
    };
    let Some(first) = text.chars().next() else {
        return false;
    };
    !previous.is_whitespace()
        && !OPENERS.contains(&previous)
        && !first.is_whitespace()
        && !ATTACHES_BACKWARD.contains(&first)
}

#[cfg(test)]
#[path = "../../../../tests/unit/platform/injection/clipboard/spacing.rs"]
mod tests;
