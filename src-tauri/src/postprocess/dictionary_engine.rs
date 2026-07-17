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
            "a", "i", "am", "an", "as", "at", "be", "by", "do", "go", "he", "if", "in", "is", "it",
            "me", "my", "no", "of", "on", "or", "so", "to", "up", "us", "we", "and", "are", "but",
            "can", "did", "for", "get", "got", "had", "has", "her", "him", "his", "how", "its",
            "let", "may", "not", "now", "off", "old", "one", "our", "out", "own", "put", "run",
            "say", "see", "she", "the", "too", "two", "use", "was", "way", "who", "why", "yet",
            "you", "your", "they", "them", "then", "than", "that", "this", "with", "have", "from",
            "been", "will", "were", "when", "what", "said", "just", "also", "into", "over", "more",
            "some", "time", "very", "here", "even", "know", "back", "only", "come", "like", "make",
            "most", "much", "need", "same", "such", "take", "well", "went", "which", "would",
            "could", "should", "there", "their", "about", "after", "where", "these", "those",
            "being", "doing", "going", "having", "making", "taking", "every", "other", "right",
            "might", "shall", "while", "still", "again", "never", "always", "often", "maybe",
            "thing", "think", "great", "small", "large", "first", "last", "next", "many", "each",
            "both", "few", "already", "before", "between",
        ]
        .into()
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
    pub const fn new(entries: Vec<DictionaryEntry>) -> Self {
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
            let end = token.rfind(|c: char| c.is_alphabetic()).map_or(0, |i| {
                i + token[i..].chars().next().map_or(0, char::len_utf8)
            });

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
        if input.len() >= 2 && input.chars().all(char::is_uppercase) {
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
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
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
#[path = "../../tests/unit/postprocess/dictionary_engine.rs"]
mod tests;
