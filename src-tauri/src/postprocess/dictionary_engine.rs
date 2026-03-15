use crate::database::models::dictionary::DictionaryEntry;

#[derive(Debug, Clone)]
pub struct DictionaryCorrectionConfig {
    pub max_distance: usize,
}

impl Default for DictionaryCorrectionConfig {
    fn default() -> Self {
        Self { max_distance: 2 }
    }
}

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
    config: DictionaryCorrectionConfig,
}

impl DictionaryCorrectionEngine {
    pub fn new(entries: Vec<DictionaryEntry>, config: DictionaryCorrectionConfig) -> Self {
        Self { entries, config }
    }

    #[cfg(test)]
    pub fn with_default_config(entries: Vec<DictionaryEntry>) -> Self {
        Self {
            entries,
            config: DictionaryCorrectionConfig::default(),
        }
    }

    /// Apply dictionary corrections to a full text string word-by-word.
    /// Punctuation attached to words is preserved.
    pub fn apply_to_text(&self, text: &str) -> String {
        if self.entries.is_empty() {
            return text.to_string();
        }
        let mut result = Vec::new();
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
                Some(c) => c.replacement,
                None => word.to_string(),
            };
            result.push(format!("{prefix}{corrected}{suffix}"));
        }
        result.join(" ")
    }

    pub fn correct(&self, input: &str) -> Option<CorrectionResult> {
        // Exact match first
        if let Some(entry) = self.entries.iter().find(|e| e.term == input) {
            return Some(CorrectionResult {
                term: entry.term.clone(),
                replacement: entry.replacement.clone(),
                distance: 0,
                exact: true,
            });
        }

        // Fuzzy match
        let mut best: Option<(usize, &DictionaryEntry)> = None;
        for entry in &self.entries {
            let distance = levenshtein(input, &entry.term);
            if distance > self.config.max_distance {
                continue;
            }
            match best {
                Some((best_dist, _)) if distance >= best_dist => {}
                _ => best = Some((distance, entry)),
            }
        }

        best.map(|(distance, entry)| CorrectionResult {
            term: entry.term.clone(),
            replacement: entry.replacement.clone(),
            distance,
            exact: false,
        })
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let mut prev_row: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr_row = vec![0; b_chars.len() + 1];

    for (i, a_char) in a_chars.iter().enumerate() {
        curr_row[0] = i + 1;
        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };
            curr_row[j + 1] = std::cmp::min(
                std::cmp::min(curr_row[j] + 1, prev_row[j + 1] + 1),
                prev_row[j] + cost,
            );
        }
        prev_row.clone_from_slice(&curr_row);
    }

    prev_row[b_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::dictionary::DictionaryEntry;
    fn entry(id: i64, term: &str, replacement: &str) -> DictionaryEntry {
        DictionaryEntry {
            id,
            term: term.to_string(),
            replacement: replacement.to_string(),
            created_at: chrono::NaiveDateTime::default(),
        }
    }

    #[test]
    fn exact_match_wins() {
        let engine = DictionaryCorrectionEngine::with_default_config(vec![
            entry(1, "teh", "the"),
        ]);
        let result = engine.correct("teh").expect("hit");
        assert!(result.exact);
        assert_eq!(result.replacement, "the");
        assert_eq!(result.distance, 0);
    }

    #[test]
    fn fuzzy_match_returns_best() {
        let engine = DictionaryCorrectionEngine::with_default_config(vec![
            entry(1, "receive", "receive"),
            entry(2, "recieve", "receive"),
        ]);
        let result = engine.correct("recive").expect("hit");
        assert!(!result.exact);
        assert_eq!(result.replacement, "receive");
    }

    #[test]
    fn apply_to_text_corrects_words() {
        let engine = DictionaryCorrectionEngine::with_default_config(vec![
            entry(1, "teh", "the"),
            entry(2, "gonna", "going to"),
        ]);
        let result = engine.apply_to_text("teh dog is gonna run");
        assert_eq!(result, "the dog is going to run");
    }

    #[test]
    fn apply_to_text_preserves_punctuation() {
        let engine = DictionaryCorrectionEngine::with_default_config(vec![
            entry(1, "teh", "the"),
        ]);
        let result = engine.apply_to_text("teh, dog.");
        assert_eq!(result, "the, dog.");
    }

    #[test]
    fn no_match_returns_none() {
        let engine = DictionaryCorrectionEngine::new(
            vec![],
            DictionaryCorrectionConfig { max_distance: 1 },
        );
        assert!(engine.correct("unknown").is_none());
    }

    #[test]
    fn empty_dictionary_returns_text_unchanged() {
        let engine = DictionaryCorrectionEngine::with_default_config(vec![]);
        assert_eq!(engine.apply_to_text("hello world"), "hello world");
    }
}
