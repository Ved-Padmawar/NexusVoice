//! External dictation triggers for sessions where a global hotkey cannot work.
//!
//! Wayland compositors own key grabs, so a global hotkey cannot be registered
//! there. Instead the user binds a command in their desktop's keyboard
//! settings, and that command reaches the running instance either as CLI
//! arguments (forwarded by [`crate::platform::single_instance`]) or as a Unix
//! signal. Both send the controller the same input the real hotkey does, so
//! there is one downstream path regardless of how dictation was triggered.

use crate::transcription::controller::{self, Input};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Toggle,
    Commit,
    Cancel,
}

impl Trigger {
    /// The same input the equivalent global hotkey sends.
    fn input(self) -> Input {
        match self {
            Self::Toggle => Input::DictationToggle,
            Self::Commit => Input::DictationCommit,
            Self::Cancel => Input::Cancel,
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

fn trigger(trigger: Trigger, source: &str) {
    log::info!("dictation {trigger:?} requested via {source}");
    controller::send(trigger.input());
}

/// Handles the arguments a second launch passed to the running instance.
/// Returns `true` when one was a dictation trigger, in which case the caller
/// should not also focus the window — the user pressed a shortcut, they are
/// working in another application.
pub fn handle_args(args: &[String]) -> bool {
    let mut handled = false;
    for parsed in args.iter().filter_map(|arg| Trigger::from_arg(arg)) {
        trigger(parsed, "cli");
        handled = true;
    }
    handled
}

/// `SIGUSR2` rather than `SIGUSR1`, which the webview build reserved; kept so
/// existing desktop shortcuts carry on working.
#[cfg(target_os = "linux")]
pub fn listen_for_signals() {
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
            trigger(Trigger::Toggle, "SIGUSR2");
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
    fn triggers_match_hotkey_inputs() {
        use crate::transcription::controller::Input;

        assert_eq!(Trigger::Toggle.input(), Input::DictationToggle);
        assert_eq!(Trigger::Commit.input(), Input::DictationCommit);
        assert_eq!(Trigger::Cancel.input(), Input::Cancel);
    }
}
