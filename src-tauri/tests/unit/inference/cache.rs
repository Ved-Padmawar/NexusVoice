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
