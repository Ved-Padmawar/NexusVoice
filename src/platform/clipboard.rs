//! The system clipboard. One handle lives for the whole run: on X11 and Wayland
//! dropping it would empty the clipboard.

use std::sync::{Mutex, OnceLock};

fn handle() -> Result<&'static Mutex<arboard::Clipboard>, String> {
    static CLIPBOARD: OnceLock<Result<Mutex<arboard::Clipboard>, String>> = OnceLock::new();
    CLIPBOARD
        .get_or_init(|| {
            arboard::Clipboard::new()
                .map(Mutex::new)
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// # Errors
/// When the clipboard cannot be opened or written.
pub fn write_text(text: &str) -> Result<(), String> {
    handle()?
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .set_text(text)
        .map_err(|e| e.to_string())
}
