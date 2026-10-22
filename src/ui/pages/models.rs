//! What the model screens share: the catalog's display order, size labels,
//! the filters on the Voice tab, and the small widgets drawn the same way in
//! the Voice tab, the first-run picker and About.

mod widgets;

use crate::api::CatalogModel;

pub use widgets::{button, icon_button, pill, progress_bar, ButtonKind, ButtonSize, Tone};

/// Family display order. Unlisted families sort after these, alphabetically.
const FAMILY_ORDER: [&str; 6] = [
    "whisper",
    "parakeet",
    "nemotron",
    "qwen3-asr",
    "canary",
    "moonshine",
];

/// Group the catalog by family, smallest model first within each.
pub fn sort_for_display(catalog: &[CatalogModel]) -> Vec<&CatalogModel> {
    let rank = |family: &str| {
        FAMILY_ORDER
            .iter()
            .position(|f| *f == family)
            .unwrap_or(FAMILY_ORDER.len())
    };
    let mut sorted: Vec<&CatalogModel> = catalog.iter().collect();
    sorted.sort_by(|a, b| {
        rank(&a.family)
            .cmp(&rank(&b.family))
            // Unlisted families share a rank; keep them deterministic.
            .then_with(|| a.family.cmp(&b.family))
            .then_with(|| a.size_bytes.cmp(&b.size_bytes))
    });
    sorted
}

/// Whether a model can be driven by its own streaming session.
pub fn is_streaming(model: &CatalogModel) -> bool {
    model.pipelines.iter().any(|p| p == "streaming")
}

/// Resolve a backend display name (from `get_model_info` or the hardware
/// profile) to a catalog id. Falls back to the first catalog entry when
/// nothing matches.
pub fn model_name_to_id<'a>(name: &str, catalog: &'a [CatalogModel]) -> Option<&'a str> {
    let name = name.to_lowercase();
    catalog
        .iter()
        .find(|m| m.display_name.to_lowercase() == name)
        .or_else(|| catalog.first())
        .map(|m| m.id.as_str())
}

/// `bytes / unit` to `decimals` places, rounded half up. Integer arithmetic,
/// so no float casts and no drift at the half.
fn fixed(bytes: u64, unit: u64, decimals: u32) -> String {
    let scale = 10u128.pow(decimals);
    let scaled = (u128::from(bytes) * scale + u128::from(unit) / 2) / u128::from(unit);
    if decimals == 0 {
        return scaled.to_string();
    }
    format!(
        "{}.{:0width$}",
        scaled / scale,
        scaled % scale,
        width = decimals as usize
    )
}

const GB: u64 = 1_000_000_000;
const MB: u64 = 1_000_000;
const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;

/// Download size as the catalog shows it, e.g. "886 MB" or "1.7 GB".
pub fn format_model_size(bytes: u64) -> String {
    if bytes >= GB {
        format!("{} GB", fixed(bytes, GB, 1))
    } else {
        format!("{} MB", fixed(bytes, MB, 0))
    }
}

/// Total on-disk size in the Voice tab's status line: decimal GB, two places.
pub fn format_disk_total(bytes: u64) -> String {
    format!("{} GB", fixed(bytes, GB, 2))
}

/// A file's size in the Model Manager: binary units, two places for GB.
pub fn format_manager_size(bytes: u64) -> String {
    if bytes == 0 {
        "0 B".into()
    } else if bytes >= GIB {
        format!("{} GB", fixed(bytes, GIB, 2))
    } else {
        format!("{} MB", fixed(bytes, MIB, 0))
    }
}

/// Total model size on About: binary units, one place for GB.
pub fn format_about_size(bytes: u64) -> String {
    if bytes >= GIB {
        format!("{} GB", fixed(bytes, GIB, 1))
    } else if bytes >= MIB {
        format!("{} MB", fixed(bytes, MIB, 0))
    } else {
        format!("{} KB", ((bytes + 512) / 1024).max(1))
    }
}

/// How heavy a download is, quantised: a proportional bar made small models
/// invisible slivers.
pub fn weight_tier(bytes: u64) -> usize {
    match bytes {
        1_000_000_000.. => 3,
        600_000_000.. => 2,
        180_000_000.. => 1,
        _ => 0,
    }
}

pub const TIER_LABELS: [&str; 4] = ["Light", "Medium", "Heavy", "Max"];

/// The Voice tab's narrowing of the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFilter {
    All,
    OnDisk,
    Streaming,
    Multilingual,
}

impl ModelFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::OnDisk, Self::Streaming, Self::Multilingual];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::OnDisk => "On disk",
            Self::Streaming => "Streaming",
            Self::Multilingual => "Multilingual",
        }
    }

    pub fn admits(self, model: &CatalogModel, on_disk: bool) -> bool {
        match self {
            Self::All => true,
            Self::OnDisk => on_disk,
            Self::Streaming => is_streaming(model),
            Self::Multilingual => model.multilingual,
        }
    }
}

/// Whether the search text appears in a model's name or description.
pub fn matches_query(model: &CatalogModel, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || model.display_name.to_lowercase().contains(&query)
        || model.description.to_lowercase().contains(&query)
}

#[cfg(test)]
#[path = "../../../tests/unit/ui/pages/models.rs"]
mod tests;
