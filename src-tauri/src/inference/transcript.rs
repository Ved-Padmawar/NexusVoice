//! Structured transcription output: the words and segments of a single decode.

/// One decoded word. Subword BPE fragments are merged into whole words upstream.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    /// DTW-aligned end timestamp in centiseconds, if DTW was enabled.
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

/// Join segments into a transcript.
pub fn join_segments(segments: &[TimedSegment]) -> String {
    let words: Vec<Word> = segments.iter().flat_map(|s| s.words.clone()).collect();
    join_words(&words)
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
