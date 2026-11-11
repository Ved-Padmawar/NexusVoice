use std::time::{Duration, Instant};

use super::{
    Landing, Step, Timeline, CHORD_DELAY, FAILED_CHORD_TIMEOUT, QUIET_PERIOD, READ_TIMEOUT,
};

const MS: Duration = Duration::from_millis(1);

/// The last instant before `at`.
fn before(at: Instant) -> Instant {
    at.checked_sub(MS).expect("instant after the epoch")
}

/// A paste whose chord went out at `t0 + CHORD_DELAY`.
fn sent(t0: Instant, ok: bool) -> (Timeline, Instant) {
    let mut timeline = Timeline::new(t0);
    let sent_at = t0 + CHORD_DELAY;
    timeline.chord_sent(sent_at, ok);
    (timeline, sent_at)
}

#[test]
fn the_chord_waits_for_the_hotkey_to_release() {
    let t0 = Instant::now();
    let timeline = Timeline::new(t0);
    assert_eq!(timeline.next_step(before(t0 + CHORD_DELAY)), Step::Wait);
    assert_eq!(timeline.next_step(t0 + CHORD_DELAY), Step::SendChord);
}

#[test]
fn it_settles_once_reads_go_quiet() {
    let (mut timeline, sent_at) = sent(Instant::now(), true);
    let read = sent_at + 20 * MS;
    timeline.record_read(read);
    assert_eq!(timeline.next_step(before(read + QUIET_PERIOD)), Step::Wait);
    assert_eq!(timeline.next_step(read + QUIET_PERIOD), Step::Settle);
}

#[test]
fn a_second_read_restarts_the_quiet_period() {
    let (mut timeline, sent_at) = sent(Instant::now(), true);
    let second = sent_at + 150 * MS;
    timeline.record_read(sent_at + 10 * MS);
    timeline.record_read(second);
    assert_eq!(
        timeline.next_step(before(second + QUIET_PERIOD)),
        Step::Wait
    );
}

#[test]
fn a_watcher_reading_before_the_chord_leaves_the_paste_unknown_not_failed() {
    let t0 = Instant::now();
    let mut timeline = Timeline::new(t0);
    timeline.record_read(t0 + MS);
    let sent_at = t0 + CHORD_DELAY;
    timeline.chord_sent(sent_at, true);
    assert_eq!(timeline.landing(), Landing::Unknown);
    // No later read will be reported, so it settles without the full timeout.
    assert_eq!(timeline.next_step(sent_at + QUIET_PERIOD), Step::Settle);
}

#[test]
fn without_a_read_it_settles_at_the_timeout() {
    let (timeline, sent_at) = sent(Instant::now(), true);
    assert_eq!(
        timeline.next_step(before(sent_at + READ_TIMEOUT)),
        Step::Wait
    );
    assert_eq!(timeline.next_step(sent_at + READ_TIMEOUT), Step::Settle);
}

#[test]
fn a_failed_chord_settles_quickly_as_not_pasted() {
    let (timeline, sent_at) = sent(Instant::now(), false);
    assert_eq!(
        timeline.next_step(sent_at + FAILED_CHORD_TIMEOUT),
        Step::Settle
    );
    assert_eq!(timeline.landing(), Landing::NotRead);

    // Even after a watcher's early read: the paste never went out.
    let t0 = Instant::now();
    let mut timeline = Timeline::new(t0);
    timeline.record_read(t0 + MS);
    timeline.chord_sent(t0 + CHORD_DELAY, false);
    assert_eq!(timeline.landing(), Landing::NotRead);
}

#[test]
fn a_user_copy_settles_at_once() {
    let t0 = Instant::now();
    let mut timeline = Timeline::new(t0);
    timeline.lose_ownership();
    assert!(timeline.ownership_lost());
    assert_eq!(timeline.next_step(t0), Step::Settle);
}

#[test]
fn a_user_copy_before_any_read_wins() {
    let (mut timeline, sent_at) = sent(Instant::now(), true);
    timeline.lose_ownership();
    assert_eq!(timeline.next_step(sent_at + MS), Step::Settle);
}
