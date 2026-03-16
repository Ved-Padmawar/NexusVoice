use std::collections::HashSet;
use std::sync::OnceLock;

static COMMON_WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn common_words() -> &'static HashSet<&'static str> {
    COMMON_WORDS.get_or_init(|| {
        include_str!("common_english_words.txt")
            .lines()
            .filter(|l| !l.is_empty())
            .collect()
    })
}

/// Returns true if the word looks domain-specific / worth learning:
/// - CamelCase (e.g. "ChromaDB", "NexusVoice")
/// - ALL_CAPS acronym (e.g. "GPU", "API")
/// - contains digits (e.g. "GPT4", "H264")
fn is_technical_pattern(word: &str) -> bool {
    // Contains digit
    if word.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    // ALL CAPS acronym (2+ uppercase letters, no lowercase)
    let upper_count = word.chars().filter(|c| c.is_uppercase()).count();
    let lower_count = word.chars().filter(|c| c.is_lowercase()).count();
    if upper_count >= 2 && lower_count == 0 {
        return true;
    }
    // CamelCase: has both upper and lower, and uppercase is not just the first char
    if upper_count > 0 && lower_count > 0 {
        let has_interior_upper = word.chars().skip(1).any(|c| c.is_uppercase());
        if has_interior_upper {
            return true;
        }
    }
    false
}

/// Extract trackable words from transcribed text.
/// Preserves original casing for storage.
/// Filters out:
/// - words shorter than 4 chars
/// - common English words (via embedded word list)
/// - purely numeric tokens
///
/// Allows through:
/// - words with technical patterns (CamelCase, acronyms, digits)
/// - words not found in common English vocabulary
pub fn extract_trackable_words(text: &str) -> Vec<String> {
    let common = common_words();

    text.split_whitespace()
        .filter_map(|token| {
            // Strip leading/trailing punctuation, preserve internal casing
            let word: String = token
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();

            if word.len() < 4 {
                return None;
            }

            // Technical pattern words are always worth learning
            if is_technical_pattern(&word) {
                return Some(word);
            }

            // For plain words, check against common English vocabulary
            let lower = word.to_lowercase();
            if common.contains(lower.as_str()) {
                return None;
            }

            // Only alphabetic tokens past this point (no mixed alnum that aren't technical)
            if !word.chars().all(|c| c.is_alphabetic()) {
                return None;
            }

            Some(word)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_common_words() {
        let words = extract_trackable_words("work answer short check tell fast deep why because thing wait");
        assert!(words.is_empty(), "common words should be filtered: {:?}", words);
    }

    #[test]
    fn keeps_technical_terms() {
        let words = extract_trackable_words("ChromaDB NexusVoice PostgreSQL GPT4 whisper-rs");
        assert!(words.contains(&"ChromaDB".to_string()), "ChromaDB should be kept");
        assert!(words.contains(&"NexusVoice".to_string()), "NexusVoice should be kept");
        assert!(words.contains(&"GPT4".to_string()), "GPT4 should be kept");
    }

    #[test]
    fn filters_short_words() {
        let words = extract_trackable_words("a an it the and for");
        assert!(words.is_empty());
    }

    #[test]
    fn preserves_original_casing() {
        let words = extract_trackable_words("NexusVoice");
        assert_eq!(words[0], "NexusVoice");
    }
}
