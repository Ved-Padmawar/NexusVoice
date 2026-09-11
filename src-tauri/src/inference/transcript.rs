//! Structured transcription output: the words and segments of a single decode.

/// One decoded word. Subword BPE fragments are merged into whole words upstream.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    /// End timestamp in centiseconds. Whisper advertises only segment-level
    /// timing, so every word of a segment carries that segment's end; archs that
    /// emit word rows (parakeet, granite) give a real per-word one.
    pub end_cs: Option<i64>,
}

/// One whisper segment: its cleaned words plus where it ends on the decode's
/// audio timeline. The end time is what lets the streaming pipeline trim the
/// audio window at a segment boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedSegment {
    pub words: Vec<Word>,
    /// Segment end relative to the start of the decoded audio, in milliseconds.
    pub end_ms: i64,
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

/// Strip the dash whisper prepends to short utterances, rendering them as
/// dialogue ("- Hello."). Leading run only: an interior dash is real
/// punctuation, and an all-dash transcript is hallucination, so empty is right.
#[must_use]
pub fn strip_leading_dashes(text: &str) -> &str {
    text.trim_start_matches(|c: char| matches!(c, '-' | '–' | '—') || c.is_whitespace())
}

#[cfg(test)]
#[path = "../../tests/unit/inference/transcript.rs"]
mod tests;
