//! Backend detection and model selection.
//!
//! Model metadata lives in [`crate::inference::catalog`]; this module decides
//! *which* catalog entry to use for the current hardware and user override.

use super::catalog::{self, ModelEntry};

/// Which transcribe.cpp backend to use for inference (auto-detected, not
/// user-configurable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cuda,
    Vulkan,
    Cpu,
}

impl Backend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
            Self::Cpu => "cpu",
        }
    }
}

/// Overrides written by versions that stored a size keyword rather than a model
/// id. Mapped on read so an upgrade keeps the user's existing choice.
const LEGACY_OVERRIDES: &[(&str, &str)] = &[
    ("large-full", "whisper-large"),
    ("large", "whisper-large-turbo"),
    ("medium", "whisper-medium"),
    ("small", "whisper-small"),
    ("base", "whisper-base"),
    ("tiny", "whisper-tiny"),
];

/// Resolve a persisted override to a catalog id, translating legacy keywords.
pub fn canonical_override(value: &str) -> Option<&'static str> {
    let trimmed = value.trim();
    if let Some(entry) = catalog::find(trimmed) {
        return Some(entry.id.as_str());
    }
    LEGACY_OVERRIDES
        .iter()
        .find(|(legacy, _)| *legacy == trimmed)
        .map(|(_, id)| *id)
}

/// Detect backend from hardware.
pub fn detect_backend() -> Backend {
    let profile = crate::hardware::cached_profile();
    match profile.execution_provider.as_str() {
        "cuda" => Backend::Cuda,
        "vulkan" => Backend::Vulkan,
        _ => Backend::Cpu,
    }
}

/// Highest-tier model at or below `max_tier`, falling back to the lowest tier
/// in the catalog. The catalog is non-empty, so this always resolves.
fn best_at_or_below(max_tier: u32) -> &'static ModelEntry {
    let models = catalog::all();
    models
        .iter()
        .rfind(|m| m.tier <= max_tier)
        .unwrap_or_else(|| models.first().expect("catalog is non-empty"))
}

/// Recommended model for the detected hardware.
pub fn recommend_model() -> &'static ModelEntry {
    let profile = crate::hardware::cached_profile();
    recommend_from_profile(
        profile.execution_provider.as_str(),
        profile.vram_gb,
        profile.ram_gb,
    )
}

/// Recommend a model from a hardware profile.
///
/// Thresholds mirror the tiers in `models.json` — GPU: ≥4 GB → large-turbo tier,
/// ≥2 GB → medium tier, else small. CPU: ≥12/≥6 GB RAM.
pub fn recommend_from_profile(
    execution_provider: &str,
    vram_gb: f32,
    ram_gb: f32,
) -> &'static ModelEntry {
    /// Tier ceilings, named for the model each targets on a stock catalog.
    const LARGE_TURBO: u32 = 50;
    const MEDIUM: u32 = 40;
    const SMALL: u32 = 30;

    let ceiling = match execution_provider {
        "cuda" => vram_ceiling(vram_gb),
        "vulkan" => {
            if vram_gb >= 1.0 {
                // Discrete GPU with a valid VRAM reading.
                vram_ceiling(vram_gb)
            } else if ram_gb >= 12.0 {
                // iGPU — DXGI reports near-zero VRAM; fall back to RAM.
                LARGE_TURBO
            } else {
                MEDIUM
            }
        }
        _ => {
            if ram_gb >= 12.0 {
                LARGE_TURBO
            } else if ram_gb >= 6.0 {
                MEDIUM
            } else {
                SMALL
            }
        }
    };
    best_at_or_below(ceiling)
}

fn vram_ceiling(vram_gb: f32) -> u32 {
    if vram_gb >= 4.0 {
        50
    } else if vram_gb >= 2.0 {
        40
    } else {
        30
    }
}

/// Resolve the active model: the user's override when it names a known model,
/// otherwise the hardware recommendation.
pub fn select_model(backend: Backend, override_id: Option<&str>) -> &'static ModelEntry {
    if let Some(entry) = override_id
        .and_then(canonical_override)
        .and_then(catalog::find)
    {
        return entry;
    }
    let profile = crate::hardware::cached_profile();
    recommend_from_profile(backend.as_str(), profile.vram_gb, profile.ram_gb)
}

#[cfg(test)]
#[path = "../../tests/unit/inference/provider.rs"]
mod tests;
