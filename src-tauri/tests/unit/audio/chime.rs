//! The start chime: short, quiet, and clean at both ends.

use super::{sample_at, Playhead, AMPLITUDE, DURATION};

#[test]
fn the_chime_is_quiet_and_silent_once_it_ends() {
    // 0.145 s at 48 kHz.
    let peak = (0..6_960_u16)
        .map(|n| sample_at(f32::from(n) / 48_000.0).abs())
        .fold(0.0_f32, f32::max);
    assert!(peak > 0.0, "the chime is silent");
    // The notes overlap briefly, so the peak may exceed one note's amplitude.
    assert!(peak <= AMPLITUDE * 2.0, "the chime peaks at {peak}");
    for t in [DURATION + 0.001, DURATION + 0.01, 1.0] {
        assert!(sample_at(t).abs() < f32::EPSILON, "still sounding at {t}s");
    }
}

#[test]
fn the_chime_starts_without_a_click() {
    assert!(sample_at(0.0).abs() < f32::EPSILON);
    assert!(sample_at(1.0 / 48_000.0).abs() < AMPLITUDE * 0.1);
}

#[test]
fn an_idle_output_stays_silent() {
    let mut playhead = Playhead::default();
    assert!((0..4_800).all(|_| playhead.next(false, 48_000.0).abs() < f32::EPSILON));
}

#[test]
fn a_triggered_chime_sounds_once_then_goes_silent() {
    let mut playhead = Playhead::default();
    let rate = 48_000.0;
    let first: Vec<f32> = (0..9_600).map(|n| playhead.next(n == 0, rate)).collect();

    assert!(first.iter().any(|s| s.abs() > AMPLITUDE * 0.5), "no chime");
    // 0.145 s is 6_960 frames at 48 kHz; everything after is silence.
    assert!(first[6_961..].iter().all(|s| s.abs() < f32::EPSILON));
    assert!(playhead.next(false, rate).abs() < f32::EPSILON);
}
