/// Stitch a list of transcribed chunk texts into a single string.
///
/// For adjacent chunk pairs, finds the longest common word sequence at the
/// tail/head boundary and merges there, eliminating duplicated words introduced
/// by the overlap window.
pub fn stitch_transcripts(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    if parts.len() == 1 {
        return parts[0].trim().to_string();
    }

    let mut result = parts[0].trim().to_string();

    for next in &parts[1..] {
        let next = next.trim();
        if next.is_empty() {
            continue;
        }
        result = merge_pair(&result, next);
    }

    result
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Merge two adjacent transcript strings by removing duplicated words at the
/// boundary introduced by the overlap window.
///
/// Algorithm: look at the last N words of `prev` and the first N words of
/// `next` (N = `STITCH_WINDOW`). Find the longest suffix of `prev_words` that
/// matches a prefix of `next_words`, then concatenate without the duplicate.
fn merge_pair(prev: &str, next: &str) -> String {
    const STITCH_WINDOW: usize = 12; // words to examine at each boundary

    let prev_words: Vec<&str> = prev.split_whitespace().collect();
    let next_words: Vec<&str> = next.split_whitespace().collect();

    let pw = prev_words.len().min(STITCH_WINDOW);
    let nw = next_words.len().min(STITCH_WINDOW);

    let prev_tail = &prev_words[prev_words.len() - pw..];
    let next_head = &next_words[..nw];

    // Find longest overlap: suffix of prev_tail == prefix of next_head
    let mut best_len = 0usize;
    for len in 1..=pw.min(nw) {
        if prev_tail[pw - len..] == next_head[..len] {
            best_len = len;
        }
    }

    if best_len > 0 {
        let next_remainder = next_words[best_len..].join(" ");
        if next_remainder.is_empty() {
            prev.to_string()
        } else {
            format!("{prev} {next_remainder}")
        }
    } else {
        format!("{prev} {next}")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stitch_removes_duplicate_words() {
        let a = "hello world this is a test".to_string();
        let b = "this is a test and more words".to_string();
        let result = stitch_transcripts(&[a, b]);
        let count = result.matches("this is a test").count();
        assert_eq!(count, 1, "duplicate overlap not removed: {result}");
        assert!(result.contains("and more words"), "tail missing: {result}");
    }

    #[test]
    fn stitch_no_overlap_joins_with_space() {
        let a = "hello world".to_string();
        let b = "goodbye world".to_string();
        let result = stitch_transcripts(&[a, b]);
        assert_eq!(result, "hello world goodbye world");
    }

    #[test]
    fn stitch_single_part_returns_as_is() {
        let result = stitch_transcripts(&["only one".to_string()]);
        assert_eq!(result, "only one");
    }
}
