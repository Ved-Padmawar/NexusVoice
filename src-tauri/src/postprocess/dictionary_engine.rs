use std::collections::HashSet;
use std::sync::OnceLock;

use rphonetic::DoubleMetaphone;

use crate::database::models::dictionary::DictionaryEntry;

// ---------------------------------------------------------------------------
// Stop words — never fuzzy-correct common English words
// ---------------------------------------------------------------------------
fn stopwords() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        [
            "a","i","am","an","as","at","be","by","do","go","he","if","in","is","it",
            "me","my","no","of","on","or","so","to","up","us","we","and","are","but",
            "can","did","for","get","got","had","has","her","him","his","how","its",
            "let","may","not","now","off","old","one","our","out","own","put","run",
            "say","see","she","the","too","two","use","was","way","who","why","yet",
            "you","your","they","them","then","than","that","this","with","have",
            "from","been","will","were","when","what","said","just","also","into",
            "over","more","some","time","very","here","even","know","back","only",
            "come","like","make","most","much","need","same","such","take","well",
            "went","which","would","could","should","there","their","about","after",
            "where","these","those","being","doing","going","having","making","taking",
            "every","other","right","might","shall","while","still","again","never",
            "always","often","maybe","thing","think","great","small","large","first",
            "last","next","many","each","both","few","already","before","between",
        ].into()
    })
}

fn is_stopword(word: &str) -> bool {
    stopwords().contains(word)
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionResult {
    pub term: String,
    pub replacement: String,
    pub distance: usize,
    pub exact: bool,
}

/// In-memory dictionary correction engine.
/// Constructed from a snapshot of dictionary entries — no DB access at correction time.
#[derive(Clone)]
pub struct DictionaryCorrectionEngine {
    entries: Vec<DictionaryEntry>,
}

impl DictionaryCorrectionEngine {
    pub fn new(entries: Vec<DictionaryEntry>) -> Self {
        Self { entries }
    }

    /// Apply dictionary corrections to a full text string word-by-word.
    /// Punctuation attached to words is preserved.
    /// Returns the corrected text and the list of matched terms (for hit tracking).
    pub fn apply_to_text(&self, text: &str) -> (String, Vec<String>) {
        if self.entries.is_empty() {
            return (text.to_string(), vec![]);
        }
        let mut result = Vec::new();
        let mut matched_terms: Vec<String> = Vec::new();
        for token in text.split_whitespace() {
            let start = token
                .find(|c: char| c.is_alphabetic())
                .unwrap_or(token.len());
            let end = token
                .rfind(|c: char| c.is_alphabetic())
                .map(|i| i + token[i..].chars().next().map_or(0, |ch| ch.len_utf8()))
                .unwrap_or(0);

            if start >= end {
                result.push(token.to_string());
                continue;
            }

            let prefix = &token[..start];
            let word = &token[start..end];
            let suffix = &token[end..];

            let corrected = match self.correct(word) {
                Some(c) => {
                    matched_terms.push(c.term.clone());
                    c.replacement
                }
                None => word.to_string(),
            };
            result.push(format!("{prefix}{corrected}{suffix}"));
        }
        (result.join(" "), matched_terms)
    }

    pub fn correct(&self, input: &str) -> Option<CorrectionResult> {
        let lower = input.to_lowercase();

        // 1. Skip tokens with digits (e.g. "v2", "mp3", "gpt4")
        if input.chars().any(|c| c.is_ascii_digit()) {
            return None;
        }

        // 2. Skip all-uppercase tokens ≥2 chars — already an acronym
        if input.len() >= 2 && input.chars().all(|c| c.is_uppercase()) {
            return None;
        }

        // 3. Exact match (case-insensitive, any length)
        if let Some(entry) = self.entries.iter().find(|e| e.term == lower) {
            return Some(CorrectionResult {
                term: entry.term.clone(),
                replacement: entry.replacement.clone(),
                distance: 0,
                exact: true,
            });
        }

        // 4. Skip stopwords — never fuzzy-correct common English words
        if is_stopword(&lower) {
            return None;
        }

        // 5. Min length guard — no fuzzy on very short words
        if lower.len() < 4 {
            return None;
        }

        // 6. Ratio-based max distance: min(2, floor(len * 0.35))
        //    len4→1, len5→1, len6→2, len7→2, len8→2, ...
        let max_dist = 2.min((lower.len() as f32 * 0.35) as usize);

        let mut best: Option<(usize, &DictionaryEntry)> = None;
        let mut second_best_dist = usize::MAX;

        for entry in &self.entries {
            // 7. First-letter constraint
            if entry.term.chars().next() != lower.chars().next() {
                continue;
            }

            let dist = strsim::levenshtein(&lower, &entry.term);
            if dist > max_dist {
                continue;
            }

            match best {
                Some((best_dist, _)) if dist < best_dist => {
                    second_best_dist = best_dist;
                    best = Some((dist, entry));
                }
                Some(_) if dist < second_best_dist => {
                    second_best_dist = dist;
                }
                None => best = Some((dist, entry)),
                _ => {}
            }
        }

        // 8. Ambiguity check — only apply if clear winner
        if let Some((best_dist, entry)) = best {
            if best_dist + 1 < second_best_dist {
                return Some(CorrectionResult {
                    term: entry.term.clone(),
                    replacement: entry.replacement.clone(),
                    distance: best_dist,
                    exact: false,
                });
            }
        }

        // 9. Phonetic fallback via Double Metaphone — catches sound-alike ASR errors
        //    that Levenshtein misses (e.g. "neksus" → "nexus", "fastrack" → "fasttrack").
        //    Only fires when no Levenshtein match was found above.
        //    Requires unambiguous phonetic match: exactly one dictionary entry shares codes.
        let dm = DoubleMetaphone::default();
        let input_codes = dm.double_metaphone(&lower);
        let ip = input_codes.primary();
        let ia = input_codes.alternate();
        if !ip.is_empty() {
            let mut phonetic_match: Option<&DictionaryEntry> = None;
            let mut phonetic_ambiguous = false;
            for entry in &self.entries {
                if entry.term.chars().next() != lower.chars().next() {
                    continue;
                }
                let entry_codes = dm.double_metaphone(&entry.term);
                let ep = entry_codes.primary();
                let ea = entry_codes.alternate();
                let matches = ep == ip || ea == ip || ep == ia || ea == ia;
                if matches {
                    if phonetic_match.is_some() {
                        phonetic_ambiguous = true;
                        break;
                    }
                    phonetic_match = Some(entry);
                }
            }
            if !phonetic_ambiguous {
                if let Some(entry) = phonetic_match {
                    return Some(CorrectionResult {
                        term: entry.term.clone(),
                        replacement: entry.replacement.clone(),
                        distance: usize::MAX, // phonetic match — no edit distance
                        exact: false,
                    });
                }
            }
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::dictionary::DictionaryEntry;

    fn entry(id: i64, term: &str, replacement: &str) -> DictionaryEntry {
        DictionaryEntry {
            id,
            term: term.to_string(),
            replacement: replacement.to_string(),
            hits: 0,
            created_at: chrono::NaiveDateTime::default(),
        }
    }

    fn engine(entries: Vec<DictionaryEntry>) -> DictionaryCorrectionEngine {
        DictionaryCorrectionEngine::new(entries)
    }

    // ── Exact matches ─────────────────────────────────────────────────────
    #[test]
    fn exact_match_wins() {
        let e = engine(vec![entry(1, "teh", "the")]);
        let r = e.correct("teh").expect("hit");
        assert!(r.exact);
        assert_eq!(r.replacement, "the");
    }

    #[test]
    fn exact_short_match() {
        let e = engine(vec![entry(1, "ui", "UI"), entry(2, "api", "API")]);
        assert_eq!(e.correct("ui").unwrap().replacement, "UI");
        assert_eq!(e.correct("api").unwrap().replacement, "API");
    }

    #[test]
    fn mixed_case_input_exact_matches() {
        let e = engine(vec![entry(1, "api", "API"), entry(2, "python", "Python")]);
        assert_eq!(e.correct("Api").unwrap().replacement, "API");
        assert_eq!(e.correct("Python").unwrap().replacement, "Python");
    }

    // ── Fuzzy matches ──────────────────────────────────────────────────────
    #[test]
    fn fuzzy_one_edit_deletion() {
        let e = engine(vec![entry(1, "recieve", "receive")]);
        assert_eq!(e.correct("recive").unwrap().replacement, "receive");
    }

    #[test]
    fn fuzzy_transposition() {
        let e = engine(vec![entry(1, "docker", "Docker")]);
        assert_eq!(e.correct("dcoker").unwrap().replacement, "Docker");
    }

    // ── Guards ────────────────────────────────────────────────────────────
    #[test]
    fn stopwords_never_corrected() {
        let e = engine(vec![entry(1, "api", "API"), entry(2, "ui", "UI")]);
        for word in &["am", "on", "my", "the", "and", "in", "us", "go"] {
            assert!(e.correct(word).is_none(), "stopword \"{word}\" should not correct");
        }
    }

    #[test]
    fn short_words_no_fuzzy() {
        let e = engine(vec![entry(1, "api", "API"), entry(2, "pdf", "PDF")]);
        for word in &["py", "io", "pf"] {
            assert!(e.correct(word).is_none(), "short \"{word}\" should not fuzzy");
        }
    }

    #[test]
    fn digit_tokens_skipped() {
        let e = engine(vec![entry(1, "api", "API")]);
        assert!(e.correct("v2").is_none());
        assert!(e.correct("mp3").is_none());
        assert!(e.correct("gpt4").is_none());
    }

    #[test]
    fn all_uppercase_tokens_skipped() {
        let e = engine(vec![entry(1, "python", "Python")]);
        assert!(e.correct("PYTHON").is_none());
    }

    #[test]
    fn ambiguous_match_skipped() {
        let e = engine(vec![
            entry(1, "docker", "Docker"),
            entry(2, "dockex", "Dockex"),
        ]);
        // "docke" is distance 1 from both — ambiguous
        assert!(e.correct("docke").is_none());
    }

    // ── apply_to_text ──────────────────────────────────────────────────────
    #[test]
    fn apply_to_text_corrects_words() {
        let e = engine(vec![entry(1, "teh", "the"), entry(2, "gonna", "going to")]);
        let (text, _) = e.apply_to_text("teh dog is gonna run");
        assert_eq!(text, "the dog is going to run");
    }

    #[test]
    fn apply_to_text_preserves_punctuation() {
        let e = engine(vec![entry(1, "teh", "the")]);
        let (text, _) = e.apply_to_text("teh, dog.");
        assert_eq!(text, "the, dog.");
    }

    #[test]
    fn apply_to_text_stopwords_unchanged() {
        let e = engine(vec![entry(1, "api", "API"), entry(2, "ui", "UI")]);
        let (text, _) = e.apply_to_text("i am on my way");
        assert_eq!(text, "i am on my way");
    }

    #[test]
    fn apply_to_text_long_sentence() {
        let e = engine(vec![
            entry(1, "github", "GitHub"),
            entry(2, "api", "API"),
            entry(3, "json", "JSON"),
            entry(4, "url", "URL"),
        ]);
        let (text, _) = e.apply_to_text("so i was using the github api to fetch some json data from the url");
        assert_eq!(text, "so i was using the GitHub API to fetch some JSON data from the URL");
    }

    #[test]
    fn empty_dictionary_returns_text_unchanged() {
        let e = engine(vec![]);
        let (text, _) = e.apply_to_text("hello world");
        assert_eq!(text, "hello world");
    }

    #[test]
    fn apply_to_text_returns_matched_terms() {
        let e = engine(vec![entry(1, "teh", "the"), entry(2, "gonna", "going to")]);
        let (_, terms) = e.apply_to_text("teh dog is gonna run");
        assert_eq!(terms, vec!["teh", "gonna"]);
    }
}
