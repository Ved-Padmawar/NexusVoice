//! Dictation language for the picker.
//!
//! Auto-detect re-runs per decode, so on the streaming path one utterance can
//! be labelled differently across chunks — a sentence that mixes languages.
//! [`AUTO`] is therefore offered but is not the default.

use serde::Serialize;

use super::catalog::ModelEntry;

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
    let Some(name) = crate::models::catalog::language_name(primary_of(code)) else {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageOption {
    /// ISO code, or `auto` for the detect-per-decode sentinel.
    pub code: String,
    pub name: String,
    pub is_selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSettings {
    /// Whether the model handles anything but English.
    pub supported: bool,
    /// Empty when `supported` is false.
    pub options: Vec<LanguageOption>,
}

/// `model`'s picker list with the `saved` choice marked, `auto` first. Codes
/// come from the catalog, so the list is complete whether or not a model has
/// loaded.
pub fn options(model: &ModelEntry, saved: Option<&str>) -> LanguageSettings {
    if !model.multilingual {
        return LanguageSettings {
            supported: false,
            options: Vec::new(),
        };
    }

    let advertised = &model.languages;
    let active = resolve(saved);

    // Region shown only where a language has several locales.
    let mut named: Vec<(String, String)> = advertised
        .iter()
        .map(|code| {
            let base = primary_of(code);
            let regioned = advertised
                .iter()
                .filter(|c| primary_of(c) == base)
                .count()
                > 1;
            (code.clone(), display_name(code, regioned))
        })
        .collect();
    named.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });

    let codes: Vec<&str> = named.iter().map(|(code, _)| code.as_str()).collect();
    let selected = active
        .and_then(|want| offered_for(&codes, want))
        .map(str::to_owned);

    let options = std::iter::once(LanguageOption {
        code: AUTO.to_string(),
        name: "Auto-detect".to_string(),
        is_selected: active.is_none(),
    })
    .chain(named.into_iter().map(|(code, name)| LanguageOption {
        is_selected: selected.as_deref() == Some(code.as_str()),
        code,
        name,
    }))
    .collect();

    LanguageSettings {
        supported: true,
        options,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/models/language.rs"]
mod tests;
