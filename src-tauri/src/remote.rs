//! External dictation triggers for sessions where a global hotkey cannot work.
//!
//! Wayland compositors own key grabs, so `tauri-plugin-global-shortcut` cannot
//! register a system-wide shortcut there. Instead the user binds a command in
//! their desktop's keyboard settings, and that command reaches the running
//! instance either as CLI arguments (via `tauri-plugin-single-instance`) or as
//! a Unix signal. Both emit the same events the real hotkey does, so there is
//! one downstream path regardless of how dictation was triggered.

use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Toggle,
    Commit,
    Cancel,
}

impl Trigger {
    /// Matches the event the equivalent global shortcut emits.
    fn event(self) -> &'static str {
        match self {
            Self::Toggle => "dictation-hotkey-pressed",
            Self::Commit => "dictation-commit-hotkey-pressed",
            Self::Cancel => "dictation-cancel-requested",
        }
    }

    fn from_arg(arg: &str) -> Option<Self> {
        match arg {
            "--toggle-dictation" => Some(Self::Toggle),
            "--commit-dictation" => Some(Self::Commit),
            "--cancel-dictation" => Some(Self::Cancel),
            _ => None,
        }
    }
}

pub fn emit(app: &AppHandle, trigger: Trigger, source: &str) {
    log::info!("dictation {trigger:?} requested via {source}");
    if let Err(error) = app.emit(trigger.event(), ()) {
        log::warn!("failed to emit {}: {error}", trigger.event());
    }
}

/// Handles the arguments a second launch passed to the running instance.
/// Returns `true` when one was a dictation trigger, in which case the caller
/// should not also focus the window — the user pressed a shortcut, they are
/// working in another application.
pub fn handle_args(app: &AppHandle, args: &[String]) -> bool {
    let mut handled = false;
    for trigger in args.iter().filter_map(|arg| Trigger::from_arg(arg)) {
        emit(app, trigger, "cli");
        handled = true;
    }
    handled
}

/// Deliberately not `SIGUSR1`: `WebKitGTK`, the webview engine on Linux, sends
/// it to its own threads to suspend them for JavaScript garbage collection.
/// Handling it turns every GC cycle into a phantom dictation.
#[cfg(target_os = "linux")]
pub fn listen_for_signals(app: AppHandle) {
    use signal_hook::consts::SIGUSR2;
    use signal_hook::iterator::Signals;

    let mut signals = match Signals::new([SIGUSR2]) {
        Ok(signals) => signals,
        Err(error) => {
            log::warn!("could not register SIGUSR2 handler: {error}");
            return;
        }
    };

    std::thread::spawn(move || {
        for _ in &mut signals {
            emit(&app, Trigger::Toggle, "SIGUSR2");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::Trigger;

    #[test]
    fn parses_known_flags() {
        assert_eq!(
            Trigger::from_arg("--toggle-dictation"),
            Some(Trigger::Toggle)
        );
        assert_eq!(
            Trigger::from_arg("--commit-dictation"),
            Some(Trigger::Commit)
        );
        assert_eq!(
            Trigger::from_arg("--cancel-dictation"),
            Some(Trigger::Cancel)
        );
    }

    #[test]
    fn ignores_unknown_flags() {
        assert_eq!(Trigger::from_arg("--nope"), None);
        assert_eq!(Trigger::from_arg(""), None);
    }

    #[test]
    fn triggers_match_hotkey_events() {
        assert_eq!(Trigger::Toggle.event(), "dictation-hotkey-pressed");
        assert_eq!(Trigger::Commit.event(), "dictation-commit-hotkey-pressed");
        assert_eq!(Trigger::Cancel.event(), "dictation-cancel-requested");
    }

    /// A shared event would make one flag silently do another's job.
    #[test]
    fn every_trigger_has_a_distinct_event() {
        let events = [Trigger::Toggle, Trigger::Commit, Trigger::Cancel].map(Trigger::event);
        let unique: std::collections::HashSet<_> = events.iter().collect();
        assert_eq!(unique.len(), events.len());
    }
}
