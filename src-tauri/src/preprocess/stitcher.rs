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

/// Normalize a word for overlap comparison only: trim leading/trailing
/// non-alphanumeric characters and lowercase. Internal punctuation (apostrophes,
/// hyphens) is preserved so "don't" == "don't" and "well-known" == "well-known".
fn normalize(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// Merge two adjacent transcript strings by removing duplicated words at the
/// boundary introduced by the overlap window.
///
/// Algorithm: examine the last N words of `prev` and the first N words of `next`
/// (N = `STITCH_WINDOW`). For *every* alignment offset — each position in
/// `prev`'s tail paired with each position in `next`'s head — measure the
/// longest fuzzy-matching run. Taking the best run over all offsets (not just
/// the suffix-vs-prefix one) means a word inserted or deleted at the boundary
/// shifts the overlap without defeating the match — the prior version only
/// aligned `prev`'s suffix to `next`'s prefix, so any boundary insertion or
/// deletion left both overlapping passages in the output.
fn merge_pair(prev: &str, next: &str) -> String {
    const STITCH_WINDOW: usize = 12; // words to examine at each boundary
    const MIN_OVERLAP: usize = 3; // shorter runs are too weak to trust as overlap

    let prev_words: Vec<&str> = prev.split_whitespace().collect();
    let next_words: Vec<&str> = next.split_whitespace().collect();

    let pw = prev_words.len().min(STITCH_WINDOW);
    let nw = next_words.len().min(STITCH_WINDOW);
    if pw == 0 || nw == 0 {
        return format!("{prev} {next}");
    }

    // Absolute index of where prev's tail window begins.
    let tail_base = prev_words.len() - pw;
    let prev_tail = &prev_words[tail_base..];
    let next_head = &next_words[..nw];

    // Anchor the overlap at next's first word, but let it align to any position
    // in prev's tail (varying `ps`) — that absorbs a substituted or *deleted*
    // boundary word. A word inserted before the overlap in `next` is left
    // unmerged: the keep-both fallback never drops the user's dictation.
    let mut best: Option<(usize, usize)> = None; // (matched, next_end)

    for ps in 0..pw {
        let mut matched = 0usize;
        let mut mismatches = 0usize;
        // Overlap length, always ending on a matched word — a tolerated
        // substitution is kept only if real matches follow it, never trailing
        // (which would drop the diverging words).
        let mut overlap = 0usize;
        let mut k = 0usize;
        while ps + k < pw && k < nw {
            if normalize(prev_tail[ps + k]) == normalize(next_head[k]) {
                matched += 1;
                overlap = k + 1;
            } else {
                // One substitution allowed once the run is established (≥3 words).
                mismatches += 1;
                if mismatches > 1 || matched < 3 {
                    break;
                }
            }
            k += 1;
        }

        if matched >= MIN_OVERLAP && best.is_none_or(|(m, _)| matched > m) {
            best = Some((matched, overlap));
        }
    }

    if let Some((_, next_end)) = best {
        // prev is committed text — keep it whole; drop only next's overlapping
        // prefix and append its remainder. This never discards prev's own words
        // even when the overlap aligns mid-tail (next dropped/changed a word).
        let tail = next_words[next_end..].join(" ");
        if tail.is_empty() {
            prev.to_string()
        } else {
            format!("{prev} {tail}")
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

    #[test]
    fn stitch_tolerates_one_word_mismatch_in_long_overlap() {
        // Whisper transcribed the boundary word differently in each chunk
        // ("test" vs "text") — fuzzy alignment must still find the overlap.
        let a = "hello world this is a test of stitching".to_string();
        let b = "this is a text of stitching and more words".to_string();
        let result = stitch_transcripts(&[a, b]);
        assert!(
            result.contains("and more words"),
            "tail missing: {result}"
        );
        assert_eq!(
            result.matches("of stitching").count(),
            1,
            "overlap not deduplicated: {result}"
        );
    }

    #[test]
    fn stitch_short_overlap_still_requires_exact_match() {
        // A 1-word fuzzy "match" would merge unrelated text — short overlaps
        // must stay exact.
        let a = "the quick brown".to_string();
        let b = "crown jumped over".to_string();
        let result = stitch_transcripts(&[a, b]);
        assert_eq!(result, "the quick brown crown jumped over");
    }

    #[test]
    fn stitch_handles_deleted_boundary_word() {
        // `next` dropped "really" — the overlap is offset within prev's tail, so
        // suffix-vs-prefix alignment would miss it. Offset search must catch it.
        let a = "we should really meet up soon".to_string();
        let b = "meet up soon at the cafe".to_string();
        let result = stitch_transcripts(&[a, b]);
        assert_eq!(
            result.matches("meet up soon").count(),
            1,
            "offset overlap not deduplicated: {result}"
        );
        assert!(result.contains("at the cafe"), "tail missing: {result}");
        assert!(result.contains("really"), "prev content dropped: {result}");
    }

    #[test]
    fn stitch_diverging_after_overlap_keeps_both_tails() {
        // prev and next share "meet up soon" then diverge. The overlap must end
        // at "soon" — absorbing the divergence point would drop prev's "and
        // confirm" (and corrupt the join).
        let a = "meet up soon and confirm".to_string();
        let b = "meet up soon at the cafe".to_string();
        let result = stitch_transcripts(&[a, b]);
        assert_eq!(result, "meet up soon and confirm at the cafe");
    }

    #[test]
    fn stitch_unrelated_chunks_keep_all_words() {
        // No genuine overlap — must not drop any dictation, even with offset search.
        let a = "the meeting starts at noon".to_string();
        let b = "please bring your laptop".to_string();
        let result = stitch_transcripts(&[a, b]);
        assert_eq!(result, "the meeting starts at noon please bring your laptop");
    }
}
