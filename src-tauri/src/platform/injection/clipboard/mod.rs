//! Clipboard paste for Windows and macOS: publish the transcript to the
//! clipboard, then send the paste chord.
//!
//! Windows also gives the user's clipboard back once the target has read the
//! transcript (see [`windows`]). macOS leaves the transcript on the clipboard.

#[cfg(target_os = "windows")]
mod caret;
#[cfg(any(target_os = "windows", test))]
mod settle;
#[cfg(any(target_os = "windows", test))]
mod spacing;
#[cfg(target_os = "windows")]
mod windows;

use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::AppHandle;

#[cfg(target_os = "macos")]
const PASTE_MODIFIER: Key = Key::Meta;
#[cfg(not(target_os = "macos"))]
const PASTE_MODIFIER: Key = Key::Control;

/// Wait before the chord, so the hotkey's own modifiers are released first —
/// otherwise Ctrl+V lands as e.g. Ctrl+Alt+V.
const CHORD_DELAY: Duration = Duration::from_millis(150);

/// # Errors
/// Returns an error when the transcript cannot be published to the clipboard.
/// The paste itself completes on a worker thread and is not awaited.
#[cfg(target_os = "windows")]
pub async fn type_text(app: &AppHandle, text: &str) -> Result<(), String> {
    windows::paste(app, text).await
}

/// # Errors
/// Returns an error when the transcript cannot be written to the clipboard.
/// The paste chord itself is sent from a worker thread and is not awaited.
#[cfg(target_os = "macos")]
pub async fn type_text(app: &AppHandle, text: &str) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard()
        .write_text(text.to_string())
        .map_err(|e| e.to_string())?;

    std::thread::spawn(|| {
        std::thread::sleep(CHORD_DELAY);
        send_paste_chord();
    });

    Ok(())
}

/// Press the platform paste chord. Returns whether the paste itself went out.
fn send_paste_chord() -> bool {
    let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
        return false;
    };
    let pressed = enigo.key(PASTE_MODIFIER, Direction::Press).is_ok();
    let sent = pressed && enigo.key(Key::Unicode('v'), Direction::Click).is_ok();
    // Always release, so a failed click never leaves the modifier stuck down.
    if enigo.key(PASTE_MODIFIER, Direction::Release).is_err() {
        log::warn!("paste modifier could not be released");
    }
    sent
}
