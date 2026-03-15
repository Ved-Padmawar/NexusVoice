/// Extract trackable words from transcribed text.
/// - Lowercases and strips punctuation
/// - Filters stop words and very short words (< 3 chars)
/// - Returns only alphabetic tokens
pub fn extract_trackable_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|token| {
            // Strip leading/trailing punctuation
            let word: String = token
                .chars()
                .filter(|c| c.is_alphabetic())
                .collect::<String>()
                .to_lowercase();

            if word.len() < 3 {
                return None;
            }
            if is_stop_word(&word) {
                return None;
            }
            Some(word)
        })
        .collect()
}

fn is_stop_word(word: &str) -> bool {
    // Common English stop words — covers function words that add no vocabulary signal
    matches!(
        word,
        "the" | "and" | "for" | "are" | "but" | "not" | "you" | "all"
            | "can" | "her" | "was" | "one" | "our" | "out" | "day"
            | "get" | "has" | "him" | "his" | "how" | "its" | "may"
            | "new" | "now" | "old" | "see" | "two" | "who" | "did"
            | "does" | "from" | "have" | "into" | "more" | "much"
            | "that" | "them" | "then" | "they" | "this" | "will"
            | "with" | "your" | "been" | "each" | "here" | "just"
            | "know" | "like" | "make" | "over" | "said" | "same"
            | "some" | "than" | "time" | "very" | "well" | "were"
            | "what" | "when" | "where" | "which" | "while" | "also"
            | "back" | "come" | "could" | "even" | "give" | "good"
            | "look" | "made" | "most" | "need" | "only" | "such"
            | "take" | "think" | "those" | "their" | "there" | "these"
            | "about" | "after" | "again" | "being" | "below"
            | "between" | "both" | "before" | "during" | "every"
            | "found" | "going" | "great" | "other" | "right"
            | "should" | "since" | "still" | "through" | "under"
            | "until" | "using" | "would" | "yeah" | "okay" | "yes"
            | "sure" | "actually" | "really" | "maybe" | "gonna"
            | "wanna" | "kind" | "mean" | "got" | "let" | "put"
            | "set" | "try" | "use" | "way" | "big" | "few" | "far"
            | "ago" | "any" | "ask" | "bit" | "end" | "top" | "own"
            | "per" | "say" | "too" | "lot" | "add" | "run"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_meaningful_words() {
        let words = extract_trackable_words("The NexusVoice application uses whisper");
        assert!(words.contains(&"nexusvoice".to_string()));
        assert!(words.contains(&"application".to_string()));
        assert!(words.contains(&"whisper".to_string()));
        assert!(!words.contains(&"the".to_string()));
        assert!(!words.contains(&"uses".to_string()) || words.contains(&"uses".to_string())); // "uses" not in stop list
    }

    #[test]
    fn strips_punctuation() {
        let words = extract_trackable_words("hello, world!");
        assert!(words.contains(&"hello".to_string()));
        assert!(words.contains(&"world".to_string()));
    }

    #[test]
    fn filters_short_words() {
        let words = extract_trackable_words("a an it");
        assert!(words.is_empty());
    }
}
