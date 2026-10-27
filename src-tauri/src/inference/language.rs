//! Dictation language for the picker.
//!
//! Auto-detect re-runs per decode, so on the streaming path one utterance can
//! be labelled differently across chunks — a sentence that mixes languages.
//! [`AUTO`] is therefore offered but is not the default.

/// Sentinel for "detect per decode".
pub const AUTO: &str = "auto";

/// Language used when nothing is saved.
pub const DEFAULT: &str = "en";

/// The primary subtag of a code: `de-DE` → `de`.
pub fn primary_of(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

/// The locale shown for [`DEFAULT`] when a model offers regions only.
const DEFAULT_LOCALE: &str = "en-US";

/// The offered code for `want`: exact, else the default's locale, else the
/// first locale of its language.
pub fn offered_for<'a>(offered: &[&'a str], want: &str) -> Option<&'a str> {
    let find = |matches: &dyn Fn(&str) -> bool| offered.iter().copied().find(|code| matches(code));
    find(&|code| code == want)
        .or_else(|| find(&|code| want == DEFAULT && code == DEFAULT_LOCALE))
        .or_else(|| find(&|code| primary_of(code) == primary_of(want)))
}

/// Display name for `code` from the catalog, else the code itself. Locales get
/// their region appended only when `regioned`.
pub fn display_name(code: &str, regioned: bool) -> String {
    let Some(name) = crate::inference::catalog::language_name(primary_of(code)) else {
        return code.to_string();
    };
    match code.split_once(['-', '_']) {
        Some((_, region)) if regioned => format!("{name} ({})", region.to_uppercase()),
        _ => name.to_string(),
    }
}

/// The hint the engine should use; `None` is auto-detect. Codes pass through
/// unchanged — the engine validates them against the loaded model.
pub fn resolve(saved: Option<&str>) -> Option<&str> {
    match saved {
        None => Some(DEFAULT),
        Some(AUTO) => None,
        Some(code) => Some(code),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/inference/language.rs"]
mod tests;
