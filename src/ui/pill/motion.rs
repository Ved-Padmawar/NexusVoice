//! Time-based tweens that ask for frames only while they move.
//!
//! A [`Tween`] is a value and a clock, nothing more: the owner samples it each
//! time it renders and requests another frame while [`Tween::is_active`].
//! Retargeting mid-flight starts from wherever the value is, so a reversal
//! never jumps.

use std::time::Instant;

/// A CSS-style cubic Bezier easing through (0,0) and (1,1).
#[derive(Debug, Clone, Copy)]
pub struct Curve(f32, f32, f32, f32);

impl Curve {
    /// The pill's shape curve. Monotone, so a reversal cannot overshoot.
    pub const SHAPE: Self = Self(0.22, 1.0, 0.36, 1.0);
    /// CSS `ease`, the default for colour and opacity.
    pub const EASE: Self = Self(0.25, 0.1, 0.25, 1.0);
    pub const EASE_OUT: Self = Self(0.0, 0.0, 0.58, 1.0);

    /// The eased progress for linear progress `x` in 0..=1.
    pub fn at(self, x: f32) -> f32 {
        let Self(x1, y1, x2, y2) = self;
        let (cx, cy) = (3.0 * x1, 3.0 * y1);
        let (bx, by) = (3.0 * (x2 - x1) - cx, 3.0 * (y2 - y1) - cy);
        let (ax, ay) = (1.0 - cx - bx, 1.0 - cy - by);
        let sample = |a: f32, b: f32, c: f32, t: f32| ((a * t + b) * t + c) * t;
        let slope = |t: f32| (3.0 * ax * t + 2.0 * bx) * t + cx;

        // Newton converges in a few steps for these gentle curves; bisection
        // covers the near-flat slopes where it would overshoot.
        let mut t = x;
        for _ in 0..8 {
            let error = sample(ax, bx, cx, t) - x;
            if error.abs() < 1e-5 {
                return sample(ay, by, cy, t);
            }
            let slope = slope(t);
            if slope.abs() < 1e-6 {
                break;
            }
            t -= error / slope;
        }
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        t = x;
        for _ in 0..24 {
            let error = sample(ax, bx, cx, t) - x;
            if error.abs() < 1e-5 {
                break;
            }
            if error > 0.0 {
                hi = t;
            } else {
                lo = t;
            }
            t = f32::midpoint(lo, hi);
        }
        sample(ay, by, cy, t)
    }
}

/// How a [`Tween`] travels: seconds, not frames.
#[derive(Debug, Clone, Copy)]
pub struct Transition {
    pub duration: f32,
    pub delay: f32,
    pub curve: Curve,
}

impl Transition {
    pub const fn new(duration: f32, curve: Curve) -> Self {
        Self {
            duration,
            delay: 0.0,
            curve,
        }
    }

    pub const fn delayed(self, delay: f32) -> Self {
        Self { delay, ..self }
    }

    /// Reduced motion: arrive on the next sample.
    pub const INSTANT: Self = Self::new(0.0, Curve::EASE);
}

#[derive(Debug, Clone, Copy)]
pub struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    transition: Transition,
}

impl Tween {
    pub fn new(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: Instant::now(),
            transition: Transition::INSTANT,
        }
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    fn progress(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.start).as_secs_f32();
        let t = &self.transition;
        if t.duration <= 0.0 {
            return if elapsed >= t.delay { 1.0 } else { 0.0 };
        }
        ((elapsed - t.delay) / t.duration).clamp(0.0, 1.0)
    }

    pub fn value(&self, now: Instant) -> f32 {
        self.from + (self.to - self.from) * self.transition.curve.at(self.progress(now))
    }

    /// Still travelling, or waiting out its delay.
    pub fn is_active(&self, now: Instant) -> bool {
        self.progress(now) < 1.0
    }

    /// Head for `to` from the current value. Aiming where it already aims is a
    /// no-op, so callers can retarget on every state change without restarting.
    pub fn retarget(&mut self, to: f32, now: Instant, transition: Transition) {
        if (to - self.to).abs() <= f32::EPSILON {
            return;
        }
        self.from = self.value(now);
        self.to = to;
        self.start = now;
        self.transition = transition;
    }

    /// Arrive at `to` without travelling.
    pub fn snap(&mut self, to: f32, now: Instant) {
        self.from = to;
        self.to = to;
        self.start = now;
    }
}
