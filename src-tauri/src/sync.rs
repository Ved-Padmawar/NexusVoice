//! Lock helpers for state shared with the audio and decode threads, and the
//! one way an owner tells the windows its data changed.

use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use serde::Serialize;
use specta::Type;

/// Lock a mutex, recovering the guard if a previous holder panicked.
/// Re-panicking here would take down the stop path and strand the pill on
/// screen.
pub fn lock_recovering<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whose data changed. The windows read again every query that reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Topic {
    /// `settings.json`: every preference, the active model among them.
    Settings,
    /// The shortcuts the OS granted.
    Hotkeys,
    /// Which model files are on disk.
    Models,
    /// Model downloads queued, running or failed.
    Downloads,
    /// The CUDA pack and its download.
    Cuda,
    /// Dictionary entries and their use counts.
    Dictionary,
    /// The transcripts table: History, search and the totals.
    History,
    /// The saved audio files.
    SavedAudio,
    /// The transcription job queue.
    Jobs,
}

type Sink = Box<dyn Fn(Topic) + Send + Sync>;

/// Set once at startup. Owners without an `AppHandle` (Settings, the
/// repositories) announce from their write path through it.
static SINK: OnceLock<Sink> = OnceLock::new();

/// Where announcements go; the app sends them to the windows.
pub fn announce_to(sink: impl Fn(Topic) + Send + Sync + 'static) {
    if SINK.set(Box::new(sink)).is_err() {
        log::error!("change announcements were already wired");
    }
}

/// Tell the windows `topic` changed. Called by an owner's write path once the
/// change has landed (file renamed, row committed), so a read it triggers
/// sees the change.
pub fn announce(topic: Topic) {
    #[cfg(test)]
    ANNOUNCED.with_borrow_mut(|announced| announced.push(topic));
    if let Some(sink) = SINK.get() {
        sink(topic);
    }
}

#[cfg(test)]
thread_local! {
    static ANNOUNCED: std::cell::RefCell<Vec<Topic>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// What this test thread announced since the last call.
#[cfg(test)]
pub fn take_announced() -> Vec<Topic> {
    ANNOUNCED.take()
}
