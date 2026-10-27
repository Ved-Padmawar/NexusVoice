//! The start chime: short, quiet, and clean at both ends.

use super::{sample_at, AMPLITUDE, DURATION};

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
