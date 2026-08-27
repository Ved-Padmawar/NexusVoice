//! Dictation language for the picker.
//!
//! Auto-detect re-runs per decode, so on the streaming path one utterance can
//! be labelled differently across chunks — a sentence that mixes languages.
//! [`AUTO`] is therefore offered but is not the default.

/// Sentinel for "detect per decode".
pub const AUTO: &str = "auto";

/// Language used when nothing is saved.
pub const DEFAULT: &str = "en";

pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
}

/// Display names for the codes a model may advertise. Not the offered list —
/// that comes from the model itself.
#[rustfmt::skip]
pub const LANGUAGES: &[Language] = &[
    Language { code: "ar", name: "Arabic" },
    Language { code: "bg", name: "Bulgarian" },
    Language { code: "cs", name: "Czech" },
    Language { code: "da", name: "Danish" },
    Language { code: "nl", name: "Dutch" },
    Language { code: "en", name: "English" },
    Language { code: "et", name: "Estonian" },
    Language { code: "fi", name: "Finnish" },
    Language { code: "fr", name: "French" },
    Language { code: "de", name: "German" },
    Language { code: "el", name: "Greek" },
    Language { code: "he", name: "Hebrew" },
    Language { code: "hi", name: "Hindi" },
    Language { code: "hu", name: "Hungarian" },
    Language { code: "id", name: "Indonesian" },
    Language { code: "it", name: "Italian" },
    Language { code: "ja", name: "Japanese" },
    Language { code: "ko", name: "Korean" },
    Language { code: "lv", name: "Latvian" },
    Language { code: "lt", name: "Lithuanian" },
    Language { code: "mt", name: "Maltese" },
    Language { code: "no", name: "Norwegian" },
    Language { code: "pl", name: "Polish" },
    Language { code: "pt", name: "Portuguese" },
    Language { code: "ro", name: "Romanian" },
    Language { code: "ru", name: "Russian" },
    Language { code: "sk", name: "Slovak" },
    Language { code: "sl", name: "Slovenian" },
    Language { code: "es", name: "Spanish" },
    Language { code: "sv", name: "Swedish" },
    Language { code: "tr", name: "Turkish" },
    Language { code: "uk", name: "Ukrainian" },
    Language { code: "vi", name: "Vietnamese" },
    Language { code: "zh", name: "Chinese" },
];

/// The primary subtag of a code: `de-DE` → `de`.
pub fn primary_of(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

/// Whether `code` is a real language — [`AUTO`] is not.
pub fn is_supported(code: &str) -> bool {
    LANGUAGES.iter().any(|l| l.code == primary_of(code))
}

/// Display name for `code`, falling back to the code itself. Models advertise
/// BCP-47 locales, so the region is appended only when `regioned`.
pub fn display_name(code: &str, regioned: bool) -> String {
    let Some(entry) = LANGUAGES.iter().find(|l| l.code == primary_of(code)) else {
        return code.to_string();
    };
    match code.split_once(['-', '_']) {
        Some((_, region)) if regioned => format!("{} ({})", entry.name, region.to_uppercase()),
        _ => entry.name.to_string(),
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
