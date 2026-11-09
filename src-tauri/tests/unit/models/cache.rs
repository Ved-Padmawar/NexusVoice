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

#[test]
fn a_warmup_after_a_mid_recording_switch_waits_for_the_recording_to_let_go() {
    let cache = Arc::new(EngineCache::new(std::env::temp_dir()));
    let lease = cache.lease();
    // A switch mid-recording: the unload is deferred, the old engine stays.
    cache.unload();

    tauri::async_runtime::block_on(async {
        let waiting = Arc::clone(&cache);
        let warmup = tokio::spawn(async move { waiting.superseded_released().await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !warmup.is_finished(),
            "warmed while the recording held the old engine"
        );

        drop(lease);
        tokio::time::timeout(Duration::from_secs(1), warmup)
            .await
            .expect("the released lease must wake the warmup")
            .unwrap();
    });
    assert!(!lock_recovering(&cache.slot).unload_pending);
}

#[test]
fn a_warmup_with_nothing_pending_proceeds_at_once() {
    let cache = EngineCache::new(std::env::temp_dir());
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(Duration::from_millis(100), cache.superseded_released())
            .await
            .expect("nothing holds the engine");
    });
}

#[test]
fn a_switch_reports_whether_a_recording_keeps_the_old_model() {
    let cache = EngineCache::new(std::env::temp_dir());
    // Idle: nothing holds the engine, so the switch applies now.
    assert!(!cache.switch_model());

    // Recording with no engine loaded yet: its own load picks up the new model.
    let lease = cache.lease();
    assert!(!cache.switch_model());
    drop(lease);

    // Recording on a loaded engine: it keeps that engine until it finishes.
    let mut slot = Slot::<&str> {
        engine: Some("old model"),
        leases: 1,
        ..Default::default()
    };
    slot.supersede();
    assert!(slot.unload_pending && slot.engine.is_some());
}
