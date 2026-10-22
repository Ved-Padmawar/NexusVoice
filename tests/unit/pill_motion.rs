use std::time::{Duration, Instant};

use super::{Curve, Transition, Tween};

#[test]
fn a_curve_runs_from_zero_to_one_without_overshooting() {
    assert!(Curve::SHAPE.at(0.0).abs() < 1e-4);
    assert!((Curve::SHAPE.at(1.0) - 1.0).abs() < 1e-4);
    let mut last = 0.0;
    for step in 1..=100_u8 {
        let eased = Curve::SHAPE.at(f32::from(step) / 100.0);
        assert!(eased >= last && eased <= 1.0 + 1e-4, "{eased} after {last}");
        last = eased;
    }
}

#[test]
fn a_tween_arrives_and_stops_asking_for_frames() {
    let start = Instant::now();
    let mut tween = Tween::new(0.0);
    tween.retarget(10.0, start, Transition::new(0.28, Curve::SHAPE));
    assert!(tween.is_active(start));
    let end = start + Duration::from_millis(300);
    assert!(!tween.is_active(end));
    assert!((tween.value(end) - 10.0).abs() < 1e-4);
}

#[test]
fn a_retarget_starts_from_where_the_value_is() {
    let start = Instant::now();
    let mut tween = Tween::new(0.0);
    tween.retarget(10.0, start, Transition::new(1.0, Curve::EASE));
    let middle = start + Duration::from_millis(400);
    let here = tween.value(middle);
    tween.retarget(0.0, middle, Transition::new(1.0, Curve::EASE));
    assert!((tween.value(middle) - here).abs() < 1e-4);
}

#[test]
fn aiming_where_it_already_aims_does_not_restart() {
    let start = Instant::now();
    let mut tween = Tween::new(5.0);
    tween.retarget(5.0, start, Transition::new(1.0, Curve::EASE));
    assert!(!tween.is_active(start));
}

#[test]
fn a_snap_leaves_nothing_running() {
    let start = Instant::now();
    let mut tween = Tween::new(0.0);
    tween.retarget(1.0, start, Transition::new(1.0, Curve::EASE).delayed(0.5));
    tween.snap(1.0, start);
    assert!(!tween.is_active(start));
}

#[test]
fn a_delay_holds_the_start_value() {
    let start = Instant::now();
    let mut tween = Tween::new(0.0);
    tween.retarget(1.0, start, Transition::new(0.2, Curve::EASE).delayed(0.1));
    assert!(tween.value(start + Duration::from_millis(50)).abs() < 1e-6);
    assert!(tween.is_active(start + Duration::from_millis(50)));
}
