//! A store's one write path, and what a subscribed window receives from it.

use std::sync::{Arc, Mutex};

use tauri::ipc::InvokeResponseBody;

use super::*;
use crate::settings::{PillSettings, PillTheme};

#[derive(Clone, PartialEq)]
struct Pill(PillSettings);

impl Slice for Pill {
    fn patch(&self, rev: u64) -> Patch {
        Patch::Settings {
            rev,
            value: SettingsView { pill: self.0 },
        }
    }
}

fn theme(theme: PillTheme) -> impl FnOnce(&mut Pill) {
    move |pill| pill.0.theme = theme
}

/// A window's channel, and the revisions it has received.
fn window() -> (Channel<Patch>, Arc<Mutex<Vec<u64>>>) {
    let revs = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&revs);
    let channel = Channel::new(move |body: InvokeResponseBody| {
        let patch: serde_json::Value = body.deserialize().unwrap();
        received.lock().unwrap().push(patch["rev"].as_u64().unwrap());
        Ok(())
    });
    (channel, revs)
}

#[test]
fn a_window_gets_the_current_value_then_each_saved_change_in_order() {
    let store = Store::new(Pill(PillSettings::default()));
    let (channel, received) = window();

    let snapshot = subscribe("main", channel);
    assert_eq!(snapshot, vec![Pill(PillSettings::default()).patch(0)]);

    store.update(theme(PillTheme::Aurora), |_| std::io::Result::Ok(())).unwrap();
    store.update(theme(PillTheme::Dawn), |_| std::io::Result::Ok(())).unwrap();
    // Changing nothing saves nothing and sends nothing; the caller still
    // learns the revision its mirror already holds.
    let unchanged = store
        .update(theme(PillTheme::Dawn), |_| -> std::io::Result<()> {
            panic!("an unchanged value was saved")
        })
        .unwrap();

    assert_eq!(unchanged, Committed { rev: 2 });
    assert_eq!(*received.lock().unwrap(), vec![1, 2]);
}

#[test]
fn a_failed_save_leaves_the_value_and_the_windows_as_they_were() {
    let store = Store::new(Pill(PillSettings::default()));
    let (channel, received) = window();
    subscribe("main", channel);

    let failed = store.update(theme(PillTheme::Aurora), |_| {
        Err(std::io::Error::other("disk full"))
    });

    assert!(failed.is_err());
    assert_eq!(store.read(|pill| pill.0.theme), PillTheme::default());
    assert!(received.lock().unwrap().is_empty());
    // The next write takes the revision the failed one did not.
    let next = store.update(theme(PillTheme::Dawn), |_| std::io::Result::Ok(())).unwrap();
    assert_eq!(next, Committed { rev: 1 });
}
