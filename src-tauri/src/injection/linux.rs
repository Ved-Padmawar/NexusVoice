//! Linux text injection.
//!
//! Wayland grants no protocol for one application to synthesize input into
//! another's window, so the `XTEST` path enigo uses is inert there. Typing goes
//! through xdg-desktop-portal where the compositor supports it, and otherwise
//! through an external helper holding the necessary privilege.
//!
//! Pasting is preferred — one chord instead of a keystroke per character.
//! Where nothing can send the chord, the transcript is typed out instead.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use super::portal;
use super::session::{self, Desktop, Server, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Wtype,
    Kwtype,
    Dotool,
    Ydotool,
    Xdotool,
}

impl Tool {
    pub const ALL: [Self; 5] = [
        Self::Wtype,
        Self::Kwtype,
        Self::Dotool,
        Self::Ydotool,
        Self::Xdotool,
    ];

    #[must_use]
    pub fn binary(self) -> &'static str {
        match self {
            Self::Wtype => "wtype",
            Self::Kwtype => "kwtype",
            Self::Dotool => "dotool",
            Self::Ydotool => "ydotool",
            Self::Xdotool => "xdotool",
        }
    }

    #[must_use]
    pub fn install_hint(self) -> &'static str {
        match self {
            Self::Wtype => "apt install wtype | pacman -S wtype | dnf install wtype",
            Self::Kwtype => "available for KDE Plasma via your distribution",
            Self::Dotool => "https://sr.ht/~geb/dotool/ (needs access to /dev/uinput)",
            Self::Ydotool => {
                "apt install ydotool | pacman -S ydotool, then enable the ydotoold service"
            }
            Self::Xdotool => "apt install xdotool | pacman -S xdotool | dnf install xdotool",
        }
    }

    /// Cached per tool, because a paste runs on every dictation and probing
    /// spawns a process. Only positive results are cached, so a tool installed
    /// while the app is running is picked up on the next attempt.
    #[must_use]
    pub fn available(self) -> bool {
        static CACHE: OnceLock<[OnceLock<bool>; Tool::ALL.len()]> = OnceLock::new();
        let slot = &CACHE.get_or_init(Default::default)[self as usize];

        if slot.get() == Some(&true) {
            return true;
        }
        let found = which(self.binary());
        if found {
            let _ = slot.set(true);
        }
        found
    }

    fn type_text(self, text: &str) -> Result<(), String> {
        match self {
            Self::Wtype | Self::Kwtype => self.run(&["--", text]),
            Self::Ydotool => self.run(&["type", "--", text]),
            Self::Xdotool => self.type_via_xdotool(text),
            Self::Dotool => Self::type_via_dotool(text),
        }
    }

    /// `kwtype` has no key-combination mode, so it is typing-only.
    fn send_paste_chord(self) -> Result<(), String> {
        match self {
            Self::Wtype => self.run(&["-M", "ctrl", "-k", "v", "-m", "ctrl"]),
            // 29 = KEY_LEFTCTRL, 47 = KEY_V.
            Self::Ydotool => self.run(&["key", "29:1", "47:1", "47:0", "29:0"]),
            Self::Xdotool => {
                let result = self.run(&["key", "--clearmodifiers", "ctrl+v"]);
                release_xdotool_modifiers();
                result
            }
            Self::Dotool => Self::chord_via_dotool(),
            Self::Kwtype => Err("kwtype cannot send key combinations".to_string()),
        }
    }

    fn chord_via_dotool() -> Result<(), String> {
        let mut child = Command::new(Self::Dotool.binary())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to run dotool: {e}"))?;

        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "dotool stdin unavailable".to_string())?;
        writeln!(stdin, "key ctrl+v").map_err(|e| format!("failed to write to dotool: {e}"))?;

        let output = child
            .wait_with_output()
            .map_err(|e| format!("failed to wait for dotool: {e}"))?;

        if output.status.success() {
            return Ok(());
        }
        Err(format!(
            "dotool failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }

    fn run(self, args: &[&str]) -> Result<(), String> {
        let output = Command::new(self.binary())
            .args(args)
            .output()
            .map_err(|e| format!("failed to run {}: {e}", self.binary()))?;

        if output.status.success() {
            return Ok(());
        }
        Err(format!(
            "{} failed: {}",
            self.binary(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }

    /// `dotool` reads commands from stdin rather than argv.
    fn type_via_dotool(text: &str) -> Result<(), String> {
        let mut child = Command::new(Self::Dotool.binary())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to run dotool: {e}"))?;

        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "dotool stdin unavailable".to_string())?;
        writeln!(stdin, "type {text}").map_err(|e| format!("failed to write to dotool: {e}"))?;

        let output = child
            .wait_with_output()
            .map_err(|e| format!("failed to wait for dotool: {e}"))?;

        if output.status.success() {
            return Ok(());
        }
        Err(format!(
            "dotool failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }

    fn type_via_xdotool(self, text: &str) -> Result<(), String> {
        let result = self.run(&["type", "--clearmodifiers", "--", text]);
        release_xdotool_modifiers();
        result
    }
}

/// `xdotool --clearmodifiers` restores whatever modifiers were held when it
/// started. If the user releases the hotkey while it types, that restore leaves
/// the modifier latched on the XTEST device for every application. Releasing
/// both sides unconditionally can make a still-held modifier read as up until
/// the next physical event, which is far preferable to a system-wide latch.
/// Lock keys are excluded because key events toggle rather than hold them.
fn release_xdotool_modifiers() {
    let result = Command::new(Tool::Xdotool.binary())
        .arg("keyup")
        .args([
            "Control_L",
            "Control_R",
            "Shift_L",
            "Shift_R",
            "Alt_L",
            "Alt_R",
            "Super_L",
            "Super_R",
        ])
        .output();

    match result {
        Ok(output) if !output.status.success() => log::warn!(
            "xdotool modifier cleanup failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => log::warn!("xdotool modifier cleanup could not run: {error}"),
        Ok(_) => {}
    }
}

fn which(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Candidates for `session`, best first.
///
/// `wtype` is omitted on GNOME and KDE: neither compositor implements
/// `zwp_virtual_keyboard_manager_v1`, so it exits without typing anything.
/// `ydotool` is last everywhere: it writes through `/dev/uinput`, below the
/// compositor, which makes it the most broadly capable and the most likely to
/// need setup (a running `ydotoold`).
#[must_use]
pub fn candidates(session: Session) -> &'static [Tool] {
    match (session.server, session.desktop) {
        (Server::X11, _) => &[Tool::Xdotool, Tool::Ydotool],
        (Server::Wayland, Desktop::Kde) => &[Tool::Kwtype, Tool::Dotool, Tool::Ydotool],
        (Server::Wayland, Desktop::Gnome) => &[Tool::Dotool, Tool::Ydotool],
        (Server::Wayland, Desktop::Other) => &[Tool::Wtype, Tool::Dotool, Tool::Ydotool],
    }
}

/// Errors when nothing can send the chord, so the caller falls back to typing.
async fn paste_via_clipboard(
    app: &tauri::AppHandle,
    text: &str,
    session: Session,
    candidates: &'static [Tool],
) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard()
        .write_text(text.to_string())
        .map_err(|e| format!("clipboard write failed: {e}"))?;

    if session.server == Server::Wayland && portal::available().await {
        return portal::send_paste_chord().await;
    }

    tokio::task::spawn_blocking(move || {
        let mut last = Err("no tool can send a paste chord".to_string());
        for tool in candidates.iter().copied().filter(|t| t.available()) {
            last = tool.send_paste_chord();
            if last.is_ok() {
                return last;
            }
        }
        last
    })
    .await
    .map_err(|e| format!("paste task failed: {e}"))?
}

/// Wayland only; connecting also warms the session so the first dictation does
/// not pay for the permission round-trip.
pub async fn portal_available() -> bool {
    session::detect().is_wayland() && portal::available().await
}

#[must_use]
pub fn selected() -> Option<Tool> {
    candidates(session::detect())
        .iter()
        .copied()
        .find(|tool| tool.available())
}

/// Tries the portal first on Wayland, then each installed candidate in turn:
/// one that is present but fails (missing daemon, unsupported protocol) falls
/// through to the next rather than ending the attempt.
///
/// Unlike the clipboard platforms this waits until the text is typed, so the
/// caller learns whether injection actually succeeded.
///
/// # Errors
/// Returns the combined failures, or an install hint when the session has no
/// injection tool available at all.
pub async fn type_text(app: &tauri::AppHandle, text: &str) -> Result<(), String> {
    let session = session::detect();
    let candidates = candidates(session);
    let mut failures = Vec::new();

    // One chord beats a keystroke per character.
    match paste_via_clipboard(app, text, session, candidates).await {
        Ok(()) => return Ok(()),
        Err(error) => {
            log::warn!("{error}");
            failures.push(error);
        }
    }

    if session.server == Server::Wayland {
        match portal::type_text(text).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                log::warn!("{error}");
                failures.push(error);
            }
        }
    }

    // Each tool spawns a process and waits, so keep it off the async runtime.
    let owned = text.to_string();
    let spawned = tokio::task::spawn_blocking(move || {
        let mut failures = Vec::new();
        for tool in candidates.iter().copied().filter(|t| t.available()) {
            match tool.type_text(&owned) {
                Ok(()) => {
                    log::debug!("typed transcript with {}", tool.binary());
                    return (true, failures);
                }
                Err(error) => {
                    log::warn!("{error}");
                    failures.push(error);
                }
            }
        }
        (false, failures)
    })
    .await
    .map_err(|e| format!("injection task failed: {e}"))?;

    if spawned.0 {
        return Ok(());
    }
    failures.extend(spawned.1);

    Err(if failures.is_empty() {
        let names: Vec<_> = candidates.iter().map(|t| t.binary()).collect();
        format!(
            "Nothing available to type with on {}. Install one of: {}.",
            session.describe(),
            names.join(", ")
        )
    } else {
        format!(
            "Every text-injection tool failed on {}: {}",
            session.describe(),
            failures.join("; ")
        )
    })
}
