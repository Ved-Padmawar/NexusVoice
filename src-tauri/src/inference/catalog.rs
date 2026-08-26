//! Model catalog: the single source of truth for which models exist.
//!
//! Entries live in `models.json`, baked in at compile time. Adding a model is a
//! JSON edit; only the metadata ships, models download on demand.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Which decode path a model runs through: [`Self::SingleShot`] decodes a whole
/// buffer per call, [`Self::Streaming`] is fed incrementally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Pipeline {
    SingleShot,
    Streaming,
}

impl Pipeline {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleShot => "single-shot",
            Self::Streaming => "streaming",
        }
    }
}

/// One catalog entry, deserialized straight from `models.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelEntry {
    /// Stable identifier, also the persisted override value.
    pub id: String,
    pub display_name: String,
    /// Architecture family, which gates family-specific run options.
    pub family: String,
    /// Every path this model supports; some support more than one.
    pub pipelines: Vec<Pipeline>,
    /// Always present in `pipelines`.
    pub default_pipeline: Pipeline,
    pub filename: String,
    pub url: String,
    pub size_bytes: u64,
    pub multilingual: bool,
    /// Capability ordering, ascending. Drives fallback and recommendation.
    pub tier: u32,
    pub description: String,
    pub detail: String,
}

impl ModelEntry {
    /// Whether the whisper run extension may be attached; others reject it.
    pub fn is_whisper(&self) -> bool {
        self.family == "whisper"
    }
}

#[derive(Debug, Deserialize)]
struct Catalog {
    models: Vec<ModelEntry>,
}

static CATALOG: OnceLock<Vec<ModelEntry>> = OnceLock::new();

/// Every model in the catalog, ascending by [`ModelEntry::tier`].
///
/// # Panics
/// Panics if `models.json` is malformed — it is compiled in, so that is an
/// authoring error, not a runtime condition a caller could handle.
pub fn all() -> &'static [ModelEntry] {
    CATALOG.get_or_init(|| {
        let raw = include_str!("models.json");
        let mut catalog: Catalog =
            serde_json::from_str(raw).expect("models.json is malformed — fix the catalog");
        assert!(!catalog.models.is_empty(), "models.json has no entries");
        for m in &catalog.models {
            assert!(
                m.pipelines.contains(&m.default_pipeline),
                "{}: default_pipeline missing from pipelines",
                m.id
            );
        }
        catalog.models.sort_by_key(|m| m.tier);
        catalog.models
    })
}

/// Look up an entry by its stable id.
pub fn find(id: &str) -> Option<&'static ModelEntry> {
    all().iter().find(|m| m.id == id)
}

#[cfg(test)]
#[path = "../../tests/unit/inference/catalog.rs"]
mod tests;
