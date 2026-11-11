//! A model change mid-recording must wait for that recording to finish.

use super::*;

#[test]
fn a_recording_holds_the_engine_until_its_lease_drops() {
    let mut slot = Slot::<&str> {
        engine: Some("model"),
        leases: 2,
        ..Default::default()
    };
    // An unload requested now is deferred, not applied.
    assert_eq!(slot.unload(), Unload::Deferred);
    assert!(slot.unload_pending);

    slot.release();
    assert_eq!(slot.engine, Some("model"));
    slot.release();
    assert!(slot.engine.is_none());
    assert!(!slot.unload_pending);
}

#[test]
fn a_load_superseded_by_a_model_switch_is_never_installed() {
    let mut slot = Slot::<&str>::default();
    let old_load = slot.generation;

    // The user picks another model while the old one is still loading.
    slot.supersede();
    assert!(!slot.install("old model", old_load));
    assert!(slot.engine.is_none());

    // Still void while a recording waits on that load.
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
    lock_recovering(&cache.slot).unload_pending = true;

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
fn a_switch_is_deferred_only_while_a_recording_holds_a_loaded_engine() {
    let cache = EngineCache::new(std::env::temp_dir());
    // Idle: nothing holds the engine, so the switch applies now.
    cache.unload();
    assert_eq!(cache.deferred(), Deferred::default());

    // Recording on a cold load: it loads the new model and keeps it.
    let mut slot = Slot::<&str> {
        leases: 1,
        ..Default::default()
    };
    slot.supersede();
    assert!(slot.install("new model", slot.generation));
    assert_eq!(slot.deferred(), Deferred::default());
    slot.release();
    assert_eq!(slot.engine, Some("new model"));

    // Recording on a loaded engine: it keeps that engine until it finishes.
    slot = Slot::<&str> {
        engine: Some("old model"),
        leases: 1,
        ..Default::default()
    };
    slot.supersede();
    assert!(slot.deferred().model);
    assert_eq!(slot.release(), None);
    assert!(slot.engine.is_none());
    assert_eq!(slot.deferred(), Deferred::default());
}

#[test]
fn a_language_saved_mid_recording_waits_for_the_last_recording_to_finish() {
    let mut slot = Slot::<&str> {
        engine: Some("engine"),
        leases: 2,
        ..Default::default()
    };
    // The recordings keep the language they started with.
    assert_eq!(slot.want_language(Some("fr".into())), None);
    assert!(slot.deferred().language);
    assert_eq!(slot.language.as_deref(), Some("fr"));

    // Only the last one to finish hands the engine the waiting language.
    assert_eq!(slot.release(), None);
    assert!(slot.deferred().language);
    assert_eq!(slot.release(), Some("engine"));
    assert_eq!(slot.deferred(), Deferred::default());

    // Idle: the engine is repointed at once, nothing waits.
    assert_eq!(slot.want_language(Some("de".into())), Some("engine"));
    assert!(!slot.deferred().language);
}

#[test]
fn an_unload_drops_a_waiting_language_with_the_engine() {
    let mut slot = Slot::<&str> {
        engine: Some("old model"),
        leases: 1,
        ..Default::default()
    };
    slot.want_language(Some("fr".into()));
    slot.supersede();
    assert_eq!(
        slot.deferred(),
        Deferred {
            model: true,
            language: true
        }
    );
    // The next engine is built with the saved language, so nothing is repointed.
    assert_eq!(slot.release(), None);
    assert_eq!(slot.deferred(), Deferred::default());
}

#[test]
fn a_language_saved_with_no_engine_loaded_waits_for_nothing() {
    let mut slot = Slot::<&str> {
        leases: 1,
        ..Default::default()
    };
    // The recording's own load reads the saved language.
    assert_eq!(slot.want_language(Some("fr".into())), None);
    assert!(!slot.deferred().language);
}

#[test]
fn a_refused_unload_leaves_nothing_queued() {
    let mut slot = Slot::<&str> {
        engine: Some("model"),
        leases: 1,
        ..Default::default()
    };
    assert!(!slot.supersede_unless_leased());
    assert!(!slot.unload_pending);
    slot.release();
    assert_eq!(slot.engine, Some("model"));

    assert!(slot.supersede_unless_leased());
    assert!(slot.engine.is_none());
}
