//! Whether a pasted transcript needs a space in front of it. Pure, so the rule
//! is tested without a window.
//!
//! Each dictation is pasted on its own, so a second one would otherwise run
//! straight into the end of the first ("...worldHello"). Only the target's
//! caret decides: a guess would put a stray space at the start of a new line.

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

pub fn needs_space(before: Before, text: &str) -> bool {
    let Before::Char(previous) = before else {
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
