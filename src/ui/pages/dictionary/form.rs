//! What the add and edit forms send: both fields trimmed and non-empty.

/// A term and its replacement ready to save, or `None` while either is blank.
pub fn entry_from_fields(term: &str, replacement: &str) -> Option<(String, String)> {
    let (term, replacement) = (term.trim(), replacement.trim());
    (!term.is_empty() && !replacement.is_empty()).then(|| (term.to_string(), replacement.to_string()))
}

#[cfg(test)]
#[path = "../../../../tests/unit/ui/pages/dictionary/form.rs"]
mod tests;
