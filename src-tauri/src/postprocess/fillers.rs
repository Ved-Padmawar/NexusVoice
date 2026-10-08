//! Filler-word removal: hesitation sounds dropped from the transcript without
//! an LLM, so it applies whether or not smart formatting is on.
//!
//! Two tiers. Sounds that are a word in no language ("uh", "umm", "hmm") always
//! go. Sounds that are a real word somewhere ("um" is Portuguese for "a", "er"
//! German for "he") go only when the transcript is known to be English.

use crate::inference::language::primary_of;

/// Filler only in English; elsewhere a real word.
const ENGLISH_FILLERS: &[&str] = &["um", "er", "erm", "ah", "ahh", "eh"];

/// Ends a sentence, so it survives the filler it was attached to.
const SENTENCE_END: [char; 3] = ['.', '!', '?'];

/// Remove filler words. `language` is the transcript's known language, `None`
/// when unknown — only the universal tier applies then. Everything else,
/// spacing and line breaks included, is kept as it was.
pub fn remove_fillers(text: &str, language: Option<&str>) -> String {
    let english = language.is_some_and(|code| primary_of(code) == "en");
    let mut out = String::with_capacity(text.len());
    let mut capitalize_next = false;
    let mut removed = false;
    // Whitespace around removed fillers. Of all the gaps that collapse into
    // one, the one with the most line breaks survives, so paragraphs stay.
    let mut held_gap: Option<&str> = None;

    let mut rest = text;
    while !rest.is_empty() {
        let gap_len = rest
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(rest.len());
        let (gap, after) = rest.split_at(gap_len);
        let token_len = after.find(char::is_whitespace).unwrap_or(after.len());
        let (token, remainder) = after.split_at(token_len);
        rest = remainder;

        let core = token.trim_end_matches(|c: char| c.is_ascii_punctuation());
        if is_filler(core, english) {
            removed = true;
            if held_gap.is_none_or(|held| line_breaks(gap) > line_breaks(held)) {
                held_gap = Some(gap);
            }
            // A capitalized filler opened its sentence; the next word now does.
            capitalize_next |= core.starts_with(char::is_uppercase);
            // The ending belongs to the sentence: "it works, uh." keeps its stop.
            let ending = token[core.len()..].trim_start_matches(|c| !SENTENCE_END.contains(&c));
            if !ending.is_empty() {
                out.truncate(out.trim_end_matches([',', ';', ':']).len());
                if !out.is_empty() && !out.ends_with(SENTENCE_END) {
                    out.push_str(ending);
                }
            }
            continue;
        }

        let gap = match held_gap.take() {
            Some(held) if line_breaks(held) > line_breaks(gap) => held,
            _ => gap,
        };
        // Fillers removed from the very start leave no leading space behind.
        if !(out.is_empty() && removed) {
            out.push_str(gap);
        }
        let mut chars = token.chars();
        if capitalize_next {
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
            }
            capitalize_next = false;
        }
        out.push_str(chars.as_str());
    }
    out
}

fn line_breaks(gap: &str) -> usize {
    gap.matches('\n').count()
}

fn is_filler(word: &str, english: bool) -> bool {
    // Every filler starts with one of these: a cheap first reject.
    if !word
        .chars()
        .next()
        .is_some_and(|c| matches!(c.to_ascii_lowercase(), 'u' | 'h' | 'm' | 'e' | 'a'))
    {
        return false;
    }
    let lower = word.to_lowercase();
    let mut letters: Vec<char> = lower.chars().collect();
    let len = letters.len();
    letters.dedup();
    let collapsed: String = letters.into_iter().collect();

    let universal = match collapsed.as_str() {
        "uh" | "uhm" | "hm" | "ehm" | "ahm" => true,
        // Only drawn out: "um" is a real word somewhere, "mm" is millimetres.
        "um" | "m" => len >= 3,
        _ => false,
    };
    universal || (english && ENGLISH_FILLERS.contains(&lower.as_str()))
}

#[cfg(test)]
#[path = "../../tests/unit/postprocess/fillers.rs"]
mod tests;
