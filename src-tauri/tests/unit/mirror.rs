//! A store's one write path, and what a subscribed window receives from it.

use std::sync::{Arc, Mutex};

use tauri::ipc::InvokeResponseBody;

use super::*;

#[derive(Clone, PartialEq)]
struct Count(u64);

impl Slice for Count {
    fn patch(&self) -> Patch {
        Patch::History(self.0)
    }
}

#[derive(Clone, PartialEq)]
struct Totals(u64);

impl Slice for Totals {
    fn patch(&self) -> Patch {
        Patch::HistoryTotals(self.0)
    }
}

/// Each revision a window received, with the slices that came with it.
type Received = Arc<Mutex<Vec<(u64, Vec<String>)>>>;

/// A window mirroring `slices`, and what it receives.
fn window(label: &str, slices: &[SliceName]) -> Received {
    let received = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&received);
    let channel = Channel::new(move |body: InvokeResponseBody| {
        let change: serde_json::Value = body.deserialize().unwrap();
        let slices = change["patches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|patch| patch["slice"].as_str().unwrap().to_owned())
            .collect();
        log.lock()
            .unwrap()
            .push((change["rev"].as_u64().unwrap(), slices));
        Ok(())
    });
    subscribe(label, slices.to_vec(), channel);
    received
}

#[test]
fn a_window_gets_each_saved_change_in_order_and_nothing_for_a_write_that_changed_nothing() {
    let store = Store::new(Count(0));
    let received = window("main", &[SliceName::History]);

    let first = store
        .update(|c| c.0 = 1, |_| std::io::Result::Ok(()))
        .unwrap();
    let second = store
        .update(|c| c.0 = 2, |_| std::io::Result::Ok(()))
        .unwrap();
    // The caller still learns the revision its mirror already holds.
    let unchanged = store
        .update(
            |c| c.0 = 2,
            |_| -> std::io::Result<()> { panic!("an unchanged value was saved") },
        )
        .unwrap();

    assert_eq!(unchanged, second);
    assert_eq!(
        *received.lock().unwrap(),
        vec![
            (first.rev, vec!["history".to_owned()]),
            (second.rev, vec!["history".to_owned()]),
        ]
    );
}

#[test]
fn a_failed_save_leaves_the_value_and_the_windows_as_they_were() {
    let store = Store::new(Count(0));
    let before = store.committed();
    let received = window("main", &[SliceName::History]);

    let failed = store.update(|c| c.0 = 1, |_| Err(std::io::Error::other("disk full")));

    assert!(failed.is_err());
    assert_eq!(store.read(|c| c.0), 0);
    assert_eq!(store.committed(), before);
    assert!(received.lock().unwrap().is_empty());
}

#[test]
fn stores_changed_together_reach_each_window_at_one_revision_and_only_its_slices() {
    let (rows, totals) = (Store::new(Count(0)), Store::new(Totals(0)));
    let main = window("main", &[SliceName::History, SliceName::HistoryTotals]);
    let pill = window("pill", &[SliceName::Recording]);

    let committed = atomic(|| {
        totals.set(|t| t.0 += 1);
        rows.set(|c| c.0 += 1)
    });

    assert_eq!(totals.committed(), committed);
    assert_eq!(
        *main.lock().unwrap(),
        vec![(
            committed.rev,
            vec!["historyTotals".to_owned(), "history".to_owned()]
        )]
    );
    // The pill mirrors neither, yet still learns the revision.
    assert_eq!(*pill.lock().unwrap(), vec![(committed.rev, Vec::new())]);
}
