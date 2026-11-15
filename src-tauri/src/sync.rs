//! Lock helpers for state shared with the audio and decode threads.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Lock a mutex, recovering the guard if a previous holder panicked.
/// Re-panicking here would take down the stop path and strand the pill on
/// screen.
pub fn lock_recovering<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
