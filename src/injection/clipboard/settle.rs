//! When a paste sends its chord and when it hands the clipboard back. Pure, so
//! the timing rules are tested without a clipboard.

use std::time::{Duration, Instant};

use super::CHORD_DELAY;

/// After the target's last read, how long before the clipboard is restored.
/// Some apps read more than once per paste: a probe, then the real read.
const QUIET_PERIOD: Duration = Duration::from_millis(200);

/// Restore even without a read. Long enough for a busy target to get there;
/// a paste into a window with no text field never reads at all.
const READ_TIMEOUT: Duration = Duration::from_secs(8);

/// The chord never went out, so no read can follow: restore quickly.
const FAILED_CHORD_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Wait,
    SendChord,
    /// Done: restore the user's clipboard if it is still ours.
    Settle,
}

/// One paste, from publishing the transcript to restoring the clipboard.
#[derive(Debug)]
pub struct Timeline {
    published_at: Instant,
    chord_sent_at: Option<Instant>,
    chord_failed: bool,
    /// Latest read after the chord. Earlier reads are clipboard watchers
    /// reacting to the change itself, not the paste.
    last_read: Option<Instant>,
    ownership_lost: bool,
}

impl Timeline {
    pub const fn new(published_at: Instant) -> Self {
        Self {
            published_at,
            chord_sent_at: None,
            chord_failed: false,
            last_read: None,
            ownership_lost: false,
        }
    }

    pub const fn chord_sent(&mut self, at: Instant, ok: bool) {
        self.chord_sent_at = Some(at);
        self.chord_failed = !ok;
    }

    pub fn record_read(&mut self, at: Instant) {
        if self.chord_sent_at.is_some_and(|sent| at >= sent) {
            self.last_read = Some(at);
        }
    }

    /// Someone else emptied the clipboard: the user copied something, and
    /// their copy wins.
    pub const fn lose_ownership(&mut self) {
        self.ownership_lost = true;
    }

    pub const fn ownership_lost(&self) -> bool {
        self.ownership_lost
    }

    pub fn next_step(&self, now: Instant) -> Step {
        if self.ownership_lost {
            return Step::Settle;
        }
        let Some(sent) = self.chord_sent_at else {
            return if now.duration_since(self.published_at) >= CHORD_DELAY {
                Step::SendChord
            } else {
                Step::Wait
            };
        };
        if self
            .last_read
            .is_some_and(|read| now.duration_since(read) >= QUIET_PERIOD)
        {
            return Step::Settle;
        }
        let timeout = if self.chord_failed {
            FAILED_CHORD_TIMEOUT
        } else {
            READ_TIMEOUT
        };
        if now.duration_since(sent) >= timeout {
            Step::Settle
        } else {
            Step::Wait
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/injection/settle.rs"]
mod tests;
