//! How a window follows the state Rust pushes.

use tauri::ipc::Channel;
use tauri::WebviewWindow;

use crate::mirror::{self, Change, SliceName};

/// The newest value of each of `slices`, then every commit on `changes`, in
/// order.
#[tauri::command]
#[specta::specta]
pub fn subscribe(
    window: WebviewWindow,
    slices: Vec<SliceName>,
    changes: Channel<Change>,
) -> Change {
    mirror::subscribe(window.label(), slices, changes)
}
