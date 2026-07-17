//! Structured transcription output: words carrying absolute audio times.

/// One decoded word, timed in milliseconds from session start.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    /// Decoder confidence in [0, 1].
    pub probability: f32,
}

impl Word {
    /// Lexical form for agreement comparison. Internal punctuation stays so
    /// "don't" and "well-known" survive intact.
    pub fn normalized(&self) -> String {
        self.text
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    }
}

/// A decode's result over one window.
#[derive(Debug, Clone, Default)]
pub struct Hypothesis {
    pub words: Vec<Word>,
}

impl Hypothesis {
    pub fn text(&self) -> String {
        let mut out = String::new();
        for w in &self.words {
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
}

#[cfg(test)]
#[path = "../../tests/unit/inference/transcript.rs"]
mod tests;
