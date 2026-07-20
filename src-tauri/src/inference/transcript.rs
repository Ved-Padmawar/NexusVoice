//! Structured transcription output: the words of a single decode.

/// One decoded word. Subword BPE fragments are merged into whole words upstream.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
}

/// Join words into a transcript, attaching trailing punctuation to its word.
pub fn join_words(words: &[Word]) -> String {
    let mut out = String::new();
    for w in words {
        let t = w.text.trim();
        if t.is_empty() {
            continue;
        }
        // Punctuation attaches to the previous word rather than taking a space.
        let attaches = t
            .chars()
            .next()
            .is_some_and(|c| matches!(c, ',' | '.' | '!' | '?' | ';' | ':' | '\''));
        if !out.is_empty() && !attaches {
            out.push(' ');
        }
        out.push_str(t);
    }
    out
}

#[cfg(test)]
#[path = "../../tests/unit/inference/transcript.rs"]
mod tests;
