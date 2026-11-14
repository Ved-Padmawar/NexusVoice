//! The state the windows mirror. An owner keeps it in a `Store`, whose one
//! write path saves the change, takes the next revision and sends the whole new
//! value to every window, in order. A window holds the newest revision of each
//! slice it was sent, so it never asks for state and never shows an older one.

use std::collections::HashMap;
use std::convert::Infallible;
use std::mem::{discriminant, Discriminant};
use std::sync::{LazyLock, Mutex};

use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;

use crate::bridge::commands::dto::DictionaryResponse;
use crate::jobs::JobSnapshot;
use crate::models::cuda::CudaStatus;
use crate::models::library::{Download, DownloadedModel};
use crate::session::recorder::RecordingSnapshot;
use crate::settings::{HotkeyBindings, SettingsView};
use crate::sync::lock_recovering;

/// One slice's whole value at a revision.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(tag = "slice", rename_all = "camelCase")]
pub enum Patch {
    Settings { rev: u64, value: SettingsView },
    /// The shortcuts the OS granted, which can be fewer than the saved ones.
    Hotkeys { rev: u64, value: HotkeyBindings },
    /// The models on disk.
    Models { rev: u64, value: Vec<DownloadedModel> },
    /// Model downloads queued, running or failed.
    Downloads { rev: u64, value: Vec<Download> },
    /// The CUDA pack; `None` until the GPU has been probed.
    Cuda { rev: u64, value: Option<CudaStatus> },
    /// The recording, and the saved changes it does not use.
    Recording { rev: u64, value: RecordingSnapshot },
    /// The running transcription job, then the queued ones.
    Jobs { rev: u64, value: Vec<JobSnapshot> },
    /// Dictionary entries and their use counts, sorted by term.
    Dictionary { rev: u64, value: Vec<DictionaryResponse> },
    /// Bytes the saved audio takes, pinned entries' included.
    SavedAudio { rev: u64, value: u64 },
}

/// The revision a write left its store at, so the window that made it knows
/// when its mirror holds the result. A write that changed nothing returns the
/// current revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct Committed {
    pub rev: u64,
}

/// State an owner keeps in a `Store`: what the windows see of it at `rev`.
pub trait Slice: Clone + PartialEq {
    fn patch(&self, rev: u64) -> Patch;
}

struct Versioned<T> {
    rev: u64,
    value: T,
}

pub struct Store<T> {
    current: Mutex<Versioned<T>>,
}

impl<T: Slice> Store<T> {
    /// Start at revision 0 and offer that value to every window from now on.
    pub fn new(value: T) -> Self {
        publish(value.patch(0));
        Self {
            current: Mutex::new(Versioned { rev: 0, value }),
        }
    }

    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&lock_recovering(&self.current).value)
    }

    /// The revision the store is at.
    pub fn committed(&self) -> Committed {
        Committed {
            rev: lock_recovering(&self.current).rev,
        }
    }

    /// The only write. Apply `change`; if it changed anything, `persist` the
    /// result, and only once that succeeds take the next revision and send it.
    /// All under the store's lock, so the windows receive revisions in order.
    pub fn update<E>(
        &self,
        change: impl FnOnce(&mut T),
        persist: impl FnOnce(&T) -> Result<(), E>,
    ) -> Result<Committed, E> {
        let mut current = lock_recovering(&self.current);
        let mut next = current.value.clone();
        change(&mut next);
        if next != current.value {
            persist(&next)?;
            current.rev += 1;
            current.value = next;
            publish(current.value.patch(current.rev));
        }
        Ok(Committed { rev: current.rev })
    }

    /// The write for state that lives only in memory.
    pub fn set(&self, change: impl FnOnce(&mut T)) -> Committed {
        let Ok(committed) = self.update(change, |_| Ok::<(), Infallible>(()));
        committed
    }
}

#[derive(Default)]
struct Hub {
    /// The newest patch of each slice, for a window that subscribes later.
    latest: HashMap<Discriminant<Patch>, Patch>,
    /// Each window's channel, by window label.
    windows: HashMap<String, Channel<Patch>>,
}

static HUB: LazyLock<Mutex<Hub>> = LazyLock::new(Mutex::default);

fn publish(patch: Patch) {
    let mut hub = lock_recovering(&HUB);
    hub.windows.retain(|window, channel| match channel.send(patch.clone()) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("stopped sending state to the {window} window: {e}");
            false
        }
    });
    hub.latest.insert(discriminant(&patch), patch);
}

/// Send `window` every change from now on, and return the newest patch of each
/// slice. Both happen under one lock, so no change falls between them. A
/// window that subscribes again, after a reload, replaces its old channel.
pub fn subscribe(window: &str, channel: Channel<Patch>) -> Vec<Patch> {
    let mut hub = lock_recovering(&HUB);
    hub.windows.insert(window.to_owned(), channel);
    hub.latest.values().cloned().collect()
}

#[cfg(test)]
#[path = "../tests/unit/mirror.rs"]
mod tests;
