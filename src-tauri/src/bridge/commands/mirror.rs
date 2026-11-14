//! How a window follows the state Rust pushes.

use tauri::ipc::Channel;
use tauri::WebviewWindow;

use crate::mirror::{self, Patch};

/// The newest patch of every slice, then each change on `patches`, in order.
#[tauri::command]
#[specta::specta]
pub fn subscribe(window: WebviewWindow, patches: Channel<Patch>) -> Vec<Patch> {
    mirror::subscribe(window.label(), patches)
}
