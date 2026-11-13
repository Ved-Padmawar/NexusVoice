//! Clipboard paste for Windows and macOS: publish the transcript to the
//! clipboard, then send the paste chord.
//!
//! On Windows the chord follows the paste setting (see [`chord`]); macOS always
//! sends Cmd+V, since its terminals paste with it too.
//!
//! Windows also gives the user's clipboard back once the target has read the
//! transcript (see [`windows`]). macOS leaves the transcript on the clipboard.

#[cfg(target_os = "windows")]
mod caret;
#[cfg(any(target_os = "windows", test))]
mod chord;
#[cfg(any(target_os = "windows", test))]
mod settle;
#[cfg(any(target_os = "windows", test))]
mod spacing;
#[cfg(target_os = "windows")]
mod windows;

use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::AppHandle;

#[cfg(target_os = "windows")]
use chord::Chord;

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

/// Press the macOS paste chord. Returns whether the paste itself went out.
#[cfg(target_os = "macos")]
fn send_paste_chord() -> bool {
    press(&[Key::Meta], Key::Unicode('v'))
}

/// Press `chord`. Returns whether the paste itself went out.
#[cfg(target_os = "windows")]
fn send_paste_chord(chord: Chord) -> bool {
    match chord {
        Chord::CtrlV => press(&[Key::Control], Key::Unicode('v')),
        Chord::CtrlShiftV => press(&[Key::Control, Key::Shift], Key::Unicode('v')),
        Chord::ShiftInsert => press(&[Key::Shift], Key::Insert),
    }
}

/// Hold `modifiers`, click `key`, release the modifiers.
fn press(modifiers: &[Key], key: Key) -> bool {
    let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
        return false;
    };
    let mut held = Vec::with_capacity(modifiers.len());
    for &modifier in modifiers {
        if enigo.key(modifier, Direction::Press).is_err() {
            break;
        }
        held.push(modifier);
    }
    let sent = held.len() == modifiers.len() && enigo.key(key, Direction::Click).is_ok();
    // Always release, so a failed click never leaves a modifier stuck down.
    for modifier in held.into_iter().rev() {
        if enigo.key(modifier, Direction::Release).is_err() {
            log::warn!("paste modifier could not be released");
        }
    }
    sent
}
