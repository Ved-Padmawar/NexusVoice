//! A model change mid-recording must wait for that recording to finish.

use super::*;

#[test]
fn a_recording_holds_the_engine_until_its_lease_drops() {
    let cache = EngineCache::new(std::env::temp_dir());
    assert!(!cache.in_use());

    let first = cache.lease();
    let second = cache.lease();
    assert!(cache.in_use());

    // An unload requested now is deferred, not applied.
    assert!(!cache.unload());
    assert!(lock_recovering(&cache.slot).unload_pending);

    drop(first);
    assert!(cache.in_use());
    drop(second);
    assert!(!cache.in_use());
    assert!(!lock_recovering(&cache.slot).unload_pending);
}

#[test]
fn a_load_superseded_by_a_model_switch_is_never_installed() {
    let mut slot = Slot::<&str>::default();
    let old_load = slot.generation;

    // The user picks another model while the old one is still loading.
    slot.supersede();
    assert!(!slot.install("old model", old_load));
    assert!(slot.engine.is_none());

    // Still void when a recording defers the unload itself.
    let new_load = slot.generation;
    slot.leases = 1;
    slot.supersede();
    assert!(!slot.install("second model", new_load));
    assert!(slot.engine.is_none());

    // A load begun after the last switch installs.
    assert!(slot.install("current model", slot.generation));
    assert_eq!(slot.engine, Some("current model"));
}
