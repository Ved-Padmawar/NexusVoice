//! A model change mid-recording must wait for that recording to finish.

use super::*;

/// A slot with `engine` loaded and one recording per language in `leases`.
fn slot_with(engine: Option<&'static str>, leases: &[Option<&str>]) -> Slot<&'static str> {
    Slot {
        engine,
        leases: leases.iter().map(|l| l.map(str::to_string)).collect(),
        ..Default::default()
    }
}

#[test]
fn a_recording_holds_the_engine_until_its_lease_drops() {
    let mut slot = slot_with(Some("model"), &[None, None]);
    // An unload requested now is deferred, not applied.
    assert_eq!(slot.unload(), Unload::Deferred);
    assert!(slot.unload_pending);

    slot.release(None);
    assert_eq!(slot.engine, Some("model"));
    slot.release(None);
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
    slot.lease(None);
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
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::load(
        dir.path(),
        crate::settings::Secrets::new("nexusvoice-tests"),
    );
    let lease = cache.lease(&settings);
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
    let mut slot = slot_with(None, &[None]);
    slot.supersede();
    assert!(slot.install("new model", slot.generation));
    assert_eq!(slot.deferred(), Deferred::default());
    slot.release(None);
    assert_eq!(slot.engine, Some("new model"));

    // Recording on a loaded engine: it keeps that engine until it finishes.
    slot = slot_with(Some("old model"), &[None]);
    slot.supersede();
    assert!(slot.deferred().model);
    slot.release(None);
    assert!(slot.engine.is_none());
    assert_eq!(slot.deferred(), Deferred::default());
}

#[test]
fn a_language_saved_mid_recording_applies_from_the_next_recording() {
    let mut slot = slot_with(Some("engine"), &[]);
    slot.lease(Some("en".into()));
    slot.lease(Some("en".into()));
    // The recordings keep the language they started with.
    slot.language = Some("fr".into());
    assert!(slot.deferred().language);

    slot.release(Some("en"));
    assert!(slot.deferred().language);
    slot.release(Some("en"));
    assert_eq!(slot.deferred(), Deferred::default());

    // The next recording starts in the saved language, so nothing waits.
    slot.lease(Some("fr".into()));
    assert!(!slot.deferred().language);
}

#[test]
fn a_discarded_engine_takes_its_waiting_unload_with_it() {
    let mut slot = slot_with(Some("old model"), &[None]);
    slot.supersede();
    assert!(slot.unload_pending);

    // The engine panics mid-recording; finalize then loads a replacement.
    slot.clear();
    assert!(slot.install("replacement", slot.generation));
    slot.release(None);
    assert_eq!(slot.engine, Some("replacement"));
}

#[test]
fn a_recording_keeps_its_language_even_before_its_engine_loads() {
    let mut slot = Slot::<&str>::default();
    slot.lease(Some("en".into()));
    slot.language = Some("fr".into());
    assert!(slot.deferred().language);
}

#[test]
fn a_refused_unload_leaves_nothing_queued() {
    let mut slot = slot_with(Some("model"), &[None]);
    assert!(!slot.supersede_unless_leased());
    assert!(!slot.unload_pending);
    slot.release(None);
    assert_eq!(slot.engine, Some("model"));

    assert!(slot.supersede_unless_leased());
    assert!(slot.engine.is_none());
}
