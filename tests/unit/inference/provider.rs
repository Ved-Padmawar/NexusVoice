//! Model selection: hardware recommendation and override resolution.

use super::*;

#[test]
fn legacy_size_keywords_map_to_catalog_ids() {
    assert_eq!(canonical_override("tiny"), Some("whisper-tiny"));
    assert_eq!(canonical_override("large"), Some("whisper-large-turbo"));
    assert_eq!(canonical_override("large-full"), Some("whisper-large"));
}

#[test]
fn canonical_override_accepts_current_ids_and_trims() {
    assert_eq!(canonical_override("whisper-medium"), Some("whisper-medium"));
    assert_eq!(
        canonical_override("  whisper-medium\n"),
        Some("whisper-medium")
    );
}

#[test]
fn canonical_override_rejects_unknown() {
    assert_eq!(canonical_override("bogus"), None);
    assert_eq!(canonical_override(""), None);
}

#[test]
fn every_legacy_keyword_maps_to_a_real_model() {
    for (legacy, id) in LEGACY_OVERRIDES {
        assert!(
            catalog::find(id).is_some(),
            "legacy '{legacy}' maps to missing model '{id}'"
        );
    }
}

#[test]
fn more_vram_never_recommends_a_weaker_model() {
    let low = recommend_from_profile("cuda", 1.0, 16.0);
    let mid = recommend_from_profile("cuda", 3.0, 16.0);
    let high = recommend_from_profile("cuda", 8.0, 16.0);
    assert!(low.tier <= mid.tier);
    assert!(mid.tier <= high.tier);
}

#[test]
fn cpu_recommendation_scales_with_ram() {
    let low = recommend_from_profile("cpu", 0.0, 4.0);
    let high = recommend_from_profile("cpu", 0.0, 16.0);
    assert!(low.tier <= high.tier);
}

#[test]
fn igpu_with_no_vram_reading_falls_back_to_ram() {
    // DXGI reports ~0 VRAM on integrated GPUs; must not collapse to the floor.
    let igpu = recommend_from_profile("vulkan", 0.0, 16.0);
    let weak = recommend_from_profile("vulkan", 0.0, 4.0);
    assert!(igpu.tier > weak.tier);
}

#[test]
fn override_wins_over_hardware() {
    let picked = select_model(Backend::Cpu, Some("whisper-tiny"));
    assert_eq!(picked.id, "whisper-tiny");
}

#[test]
fn unknown_override_falls_back_to_recommendation() {
    // Both resolve through the hardware path, so they must agree on this host.
    let picked = select_model(Backend::Cpu, Some("bogus"));
    assert_eq!(picked.id, recommend_model().id);
}

#[test]
fn absent_override_matches_the_recommendation() {
    assert_eq!(select_model(Backend::Cpu, None).id, recommend_model().id);
}

#[test]
fn backend_names_match_the_hardware_profile_strings() {
    // `select_model` routes on these, so they must match what the detector emits.
    assert_eq!(Backend::Cuda.as_str(), "cuda");
    assert_eq!(Backend::Vulkan.as_str(), "vulkan");
    assert_eq!(Backend::Cpu.as_str(), "cpu");
}
