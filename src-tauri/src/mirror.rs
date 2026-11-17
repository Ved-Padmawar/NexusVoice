//! The state the windows mirror. An owner keeps its part in a `Store`, whose
//! one write path saves the change and adds the store's new value to a
//! commit. A commit takes the app's next revision and sends each window, in
//! order and in one message, the changed slices it mirrors. `atomic` groups
//! the writes of several stores into one commit, so a window never shows half
//! of a change.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;

use crate::bridge::commands::dto::DictionaryResponse;
use crate::jobs::JobSnapshot;
use crate::models::cuda::CudaStatus;
use crate::models::library::{Download, DownloadedModel};
use crate::recordings::SavedAudioStatus;
use crate::session::recorder::RecordingSnapshot;
use crate::settings::{HotkeyBindings, SettingsView};
use crate::sync::lock_recovering;

/// One slice's whole value.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(tag = "slice", content = "value", rename_all = "camelCase")]
pub enum Patch {
    Settings(SettingsView),
    /// The shortcuts the OS granted, which can be fewer than the saved ones.
    Hotkeys(HotkeyBindings),
    /// The models on disk.
    Models(Vec<DownloadedModel>),
    /// Model downloads queued, running or failed.
    Downloads(Vec<Download>),
    /// The CUDA pack; `None` until the GPU has been probed.
    Cuda(Option<CudaStatus>),
    /// The recording, and the saved changes it does not use.
    Recording(RecordingSnapshot),
    /// The running transcription job, then the queued ones.
    Jobs(Vec<JobSnapshot>),
    /// Dictionary entries and their use counts, sorted by term.
    Dictionary(Vec<DictionaryResponse>),
    /// Bytes the saved audio takes and whether retention needs a retry.
    SavedAudio(SavedAudioStatus),
    /// Writes to History rows; a window reads the rows it shows again at each.
    History(u64),
    /// Writes that change History's totals and daily words.
    HistoryTotals(u64),
}

/// The name of a slice, for a window to say which ones it mirrors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SliceName {
    Settings,
    Hotkeys,
    Models,
    Downloads,
    Cuda,
    Recording,
    Jobs,
    Dictionary,
    SavedAudio,
    History,
    HistoryTotals,
}

impl Patch {
    fn name(&self) -> SliceName {
        match self {
            Self::Settings(_) => SliceName::Settings,
            Self::Hotkeys(_) => SliceName::Hotkeys,
            Self::Models(_) => SliceName::Models,
            Self::Downloads(_) => SliceName::Downloads,
            Self::Cuda(_) => SliceName::Cuda,
            Self::Recording(_) => SliceName::Recording,
            Self::Jobs(_) => SliceName::Jobs,
            Self::Dictionary(_) => SliceName::Dictionary,
            Self::SavedAudio(_) => SliceName::SavedAudio,
            Self::History(_) => SliceName::History,
            Self::HistoryTotals(_) => SliceName::HistoryTotals,
        }
    }
}

/// What a window receives: the slices one commit changed, at its revision.
/// A window gets every revision, with no patches when none of its slices
/// changed, so it can always tell when it holds a write.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Change {
    pub rev: u64,
    pub patches: Vec<Patch>,
}

/// The revision a write left its store at, so the window that made it knows
/// when its mirror holds the result. A write that changed nothing returns the
/// revision the store last changed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct Committed {
    pub rev: u64,
}

/// State an owner keeps in a `Store`: what the windows see of it.
pub trait Slice: Clone + PartialEq {
    fn patch(&self) -> Patch;
}

struct Window {
    slices: HashSet<SliceName>,
    channel: Channel<Change>,
}

#[derive(Default)]
struct Hub {
    rev: u64,
    /// The newest patch of each slice, for a window that subscribes later.
    latest: HashMap<SliceName, Patch>,
    /// Each window, by label.
    windows: HashMap<String, Window>,
}

/// The app's hub. Each test thread gets its own, so tests sharing a process
/// never see each other's commits.
fn hub() -> &'static Mutex<Hub> {
    #[cfg(not(test))]
    {
        static HUB: std::sync::LazyLock<Mutex<Hub>> = std::sync::LazyLock::new(Mutex::default);
        &HUB
    }
    #[cfg(test)]
    {
        thread_local! {
            static HUB: &'static Mutex<Hub> = Box::leak(Box::default());
        }
        HUB.with(|hub| *hub)
    }
}

impl Hub {
    fn publish(&mut self, rev: u64, patches: Vec<Patch>) {
        self.rev = rev;
        self.windows.retain(|label, window| {
            let change = Change {
                rev,
                patches: patches
                    .iter()
                    .filter(|patch| window.slices.contains(&patch.name()))
                    .cloned()
                    .collect(),
            };
            match window.channel.send(change) {
                Ok(()) => true,
                Err(e) => {
                    log::warn!("stopped sending state to the {label} window: {e}");
                    false
                }
            }
        });
        for patch in patches {
            self.latest.insert(patch.name(), patch);
        }
    }
}

/// A commit in progress on this thread, holding the hub until it publishes.
struct Open {
    hub: MutexGuard<'static, Hub>,
    rev: u64,
    patches: Vec<Patch>,
}

thread_local! {
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
}

/// Publishes the open commit when `atomic` returns or unwinds: a store that
/// changed before a panic still reaches the windows.
struct Publish;

impl Drop for Publish {
    fn drop(&mut self) {
        if let Some(mut open) = OPEN.take() {
            if !open.patches.is_empty() {
                let patches = std::mem::take(&mut open.patches);
                open.hub.publish(open.rev, patches);
            }
        }
    }
}

/// Run `writes` as one commit: every store they change is sent at one
/// revision, in one message. The hub is held throughout, so `writes` must not
/// take a lock that is held around a store write elsewhere (the job queue,
/// the recorder's phase). A nested call joins the commit already open.
pub fn atomic<R>(writes: impl FnOnce() -> R) -> R {
    if OPEN.with_borrow(Option::is_some) {
        return writes();
    }
    let hub = lock_recovering(hub());
    let rev = hub.rev + 1;
    OPEN.set(Some(Open {
        hub,
        rev,
        patches: Vec::new(),
    }));
    let _publish = Publish;
    writes()
}

struct Versioned<T> {
    rev: u64,
    value: T,
    /// Changed since its last commit.
    dirty: bool,
}

pub struct Store<T> {
    current: Mutex<Versioned<T>>,
}

impl<T: Slice> Store<T> {
    /// Commit `value`, for every window.
    pub fn new(value: T) -> Self {
        let store = Self {
            current: Mutex::new(Versioned {
                rev: 0,
                value,
                dirty: true,
            }),
        };
        store.committed();
        store
    }

    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&lock_recovering(&self.current).value)
    }

    /// `read`, with the revision the store last committed: a change undone
    /// between two reads still moves it.
    pub fn read_at<R>(&self, read: impl FnOnce(&T) -> R) -> (R, u64) {
        let current = lock_recovering(&self.current);
        (read(&current.value), current.rev)
    }

    /// Apply `change` under the store's lock; if it changed anything, `persist`
    /// the result, and only once that succeeds keep it and commit it.
    pub fn update<E>(
        &self,
        change: impl FnOnce(&mut T),
        persist: impl FnOnce(&T) -> Result<(), E>,
    ) -> Result<Committed, E> {
        {
            let mut current = lock_recovering(&self.current);
            let mut next = current.value.clone();
            change(&mut next);
            if next != current.value {
                persist(&next)?;
                current.value = next;
                current.dirty = true;
            }
        }
        Ok(self.committed())
    }

    /// The write for state that lives only in memory.
    pub fn set(&self, change: impl FnOnce(&mut T)) -> Committed {
        let Ok(committed) = self.update(change, |_| Ok::<(), Infallible>(()));
        committed
    }

    /// The revision the store is at, committing its value first if it changed
    /// since the last commit. Sends the value as it is now, under the hub, so
    /// a commit that ran late never sends an older one.
    pub fn committed(&self) -> Committed {
        atomic(|| {
            let mut current = lock_recovering(&self.current);
            if current.dirty {
                current.dirty = false;
                OPEN.with_borrow_mut(|open| {
                    let open = open.as_mut().expect("atomic opened a commit");
                    current.rev = open.rev;
                    open.patches.push(current.value.patch());
                });
            }
            Committed { rev: current.rev }
        })
    }
}

/// Send `window` the `slices` it mirrors at every commit from now on, and
/// return their newest values. Both happen under the hub, so no commit falls
/// between them. A window that subscribes again, after a reload, replaces its
/// old channel.
pub fn subscribe(window: &str, slices: Vec<SliceName>, channel: Channel<Change>) -> Change {
    let mut hub = lock_recovering(hub());
    let slices: HashSet<SliceName> = slices.into_iter().collect();
    let patches = hub
        .latest
        .values()
        .filter(|patch| slices.contains(&patch.name()))
        .cloned()
        .collect();
    hub.windows
        .insert(window.to_owned(), Window { slices, channel });
    Change {
        rev: hub.rev,
        patches,
    }
}

#[cfg(test)]
#[path = "../tests/unit/mirror.rs"]
mod tests;
