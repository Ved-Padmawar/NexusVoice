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

/// A dictionary entry with the key it matches on: its term lowercased with
/// spaces removed, so a term typed as "Next JS" matches "next js" as spoken.
#[derive(Clone)]
struct Keyed {
    key: String,
    entry: DictionaryEntry,
}

impl Keyed {
    fn result(&self, distance: usize, exact: bool) -> CorrectionResult {
        CorrectionResult {
            term: self.entry.term.clone(),
            replacement: self.entry.replacement.clone(),
            distance,
            exact,
        }
    }
}

/// Longest run of words tried as one dictionary term.
const MAX_SPAN: usize = 3;

/// One whitespace-separated token: the whitespace before it, then any
/// punctuation around its word. `word` is empty for a token without letters,
/// whose text is then all `prefix`.
struct Token<'a> {
    gap: &'a str,
    prefix: &'a str,
    word: &'a str,
    suffix: &'a str,
}

fn tokenize(text: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let gap_len = rest
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(rest.len());
        let (gap, after) = rest.split_at(gap_len);
        let token_len = after.find(char::is_whitespace).unwrap_or(after.len());
        let (token, remainder) = after.split_at(token_len);
        rest = remainder;

        let start = token
            .find(|c: char| c.is_alphabetic())
            .unwrap_or(token.len());
        let end = token.rfind(|c: char| c.is_alphabetic()).map_or(0, |i| {
            i + token[i..].chars().next().map_or(0, char::len_utf8)
        });
        tokens.push(if start < end {
            Token {
                gap,
                prefix: &token[..start],
                word: &token[start..end],
                suffix: &token[end..],
            }
        } else {
            Token {
                gap,
                prefix: token,
                word: "",
                suffix: "",
            }
        });
    }
    tokens
}

/// In-memory dictionary correction engine.
/// Constructed from a snapshot of dictionary entries — no DB access at correction time.
#[derive(Clone)]
pub struct DictionaryCorrectionEngine {
    entries: Vec<Keyed>,
}

impl DictionaryCorrectionEngine {
    pub fn new(entries: Vec<DictionaryEntry>) -> Self {
        let entries = entries
            .into_iter()
            .map(|entry| Keyed {
                key: entry
                    .term
                    .split_whitespace()
                    .collect::<String>()
                    .to_lowercase(),
                entry,
            })
            .collect();
        Self { entries }
    }

    /// Apply dictionary corrections to a full text string. Each position first
    /// tries a run of neighbouring words as one term ("next js" → "Next.js",
    /// where speech-to-text split a name), then the single word.
    /// Punctuation attached to words and the whitespace between them (newlines
    /// in formatted output) are preserved.
    /// Returns the corrected text and the list of matched terms (for hit tracking).
    pub fn apply_to_text(&self, text: &str) -> (String, Vec<String>) {
        if self.entries.is_empty() {
            return (text.to_string(), vec![]);
        }
        let tokens = tokenize(text);
        let mut result = String::with_capacity(text.len());
        let mut matched_terms: Vec<String> = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let token = &tokens[i];
            result.push_str(token.gap);
            result.push_str(token.prefix);
            let consumed = if let Some((consumed, correction)) = self
                .correct_span(&tokens[i..])
                .or_else(|| self.correct_word(token.word).map(|c| (1, c)))
            {
                result.push_str(&correction.replacement);
                matched_terms.push(correction.term);
                consumed
            } else {
                result.push_str(token.word);
                1
            };
            result.push_str(tokens[i + consumed - 1].suffix);
            i += consumed;
        }
        (result, matched_terms)
    }

    /// The longest run of up to [`MAX_SPAN`] words that exactly matches one
    /// term — a near miss would swallow a word ("docker hi"). Never joined
    /// across punctuation or a line break.
    fn correct_span(&self, tokens: &[Token]) -> Option<(usize, CorrectionResult)> {
        (2..=MAX_SPAN.min(tokens.len())).rev().find_map(|len| {
            let span = &tokens[..len];
            let joinable = span.iter().all(|t| !t.word.is_empty())
                && span[..len - 1].iter().all(|t| t.suffix.is_empty())
                && span[1..]
                    .iter()
                    .all(|t| t.prefix.is_empty() && !t.gap.contains('\n'));
            if !joinable {
                return None;
            }
            let joined: String = span.iter().map(|t| t.word).collect();
            self.exact(&joined.to_lowercase()).map(|c| (len, c))
        })
    }

    /// [`Self::correct`] that keeps a word's ending: "Docker's", "Dockers".
    fn correct_word(&self, word: &str) -> Option<CorrectionResult> {
        let reattach = |ending: &str, c: CorrectionResult| CorrectionResult {
            replacement: format!("{}{ending}", c.replacement),
            ..c
        };
        for ending in ["'s", "’s"] {
            if let Some(stem) = word.strip_suffix(ending) {
                return self.correct(stem).map(|c| reattach(ending, c));
            }
        }
        let whole = self.correct(word);
        if whole.as_ref().is_some_and(|c| c.exact) {
            return whole;
        }
        // Plural only of an exact term spelled as its replacement; "postgress"
        // is a misspelling.
        word.strip_suffix('s')
            .filter(|stem| !stem.ends_with('s'))
            .and_then(|stem| self.exact(&stem.to_lowercase()))
            .filter(|c| c.replacement.eq_ignore_ascii_case(&c.term))
            .map(|c| reattach("s", c))
            .or(whole)
    }

    /// Case-insensitive exact match; `lower` must already be lowercase.
    fn exact(&self, lower: &str) -> Option<CorrectionResult> {
        self.entries
            .iter()
            .find(|k| k.key == lower)
            .map(|k| k.result(0, true))
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
        if let Some(exact) = self.exact(&lower) {
            return Some(exact);
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

        let mut best: Option<(usize, &Keyed)> = None;
        let mut second_best_dist = usize::MAX;

        for keyed in &self.entries {
            // 7. First-letter constraint
            if keyed.key.chars().next() != lower.chars().next() {
                continue;
            }

            let dist = strsim::levenshtein(&lower, &keyed.key);
            if dist > max_dist {
                continue;
            }

            match best {
                Some((best_dist, _)) if dist < best_dist => {
                    second_best_dist = best_dist;
                    best = Some((dist, keyed));
                }
                Some(_) if dist < second_best_dist => {
                    second_best_dist = dist;
                }
                None => best = Some((dist, keyed)),
                _ => {}
            }
        }

        // 8. Ambiguity check — only apply if clear winner
        if let Some((best_dist, keyed)) = best {
            if best_dist + 1 < second_best_dist {
                return Some(keyed.result(best_dist, false));
            }
        }

        // 9. Sound-alike fallback ("neksus" → "nexus"), unambiguous matches only.
        //    ASCII (the encoder panics on other scripts), 5+ letters ("tory" ≠ "tauri").
        if !lower.is_ascii() || lower.len() < 5 {
            return None;
        }
        let dm = DoubleMetaphone::default();
        let input_codes = dm.double_metaphone(&lower);
        let ip = input_codes.primary();
        let ia = input_codes.alternate();
        if !ip.is_empty() {
            let mut phonetic_match: Option<&Keyed> = None;
            let mut phonetic_ambiguous = false;
            for keyed in &self.entries {
                if !keyed.key.is_ascii() || keyed.key.chars().next() != lower.chars().next() {
                    continue;
                }
                let entry_codes = dm.double_metaphone(&keyed.key);
                let ep = entry_codes.primary();
                let ea = entry_codes.alternate();
                let matches = ep == ip || ea == ip || ep == ia || ea == ia;
                if matches {
                    if phonetic_match.is_some() {
                        phonetic_ambiguous = true;
                        break;
                    }
                    phonetic_match = Some(keyed);
                }
            }
            if !phonetic_ambiguous {
                if let Some(keyed) = phonetic_match {
                    // Phonetic match — no edit distance.
                    return Some(keyed.result(usize::MAX, false));
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
