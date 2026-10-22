//! The four capture waveforms (`bars`, `memo`, `eq`, `spectrum`). They ease
//! toward the latest levels and request frames only while something moves.

use std::time::Instant;

use gpui_kit::{
    canvas, fill, point, px, size, Bounds, Context, Corners, Entity, IntoElement, Pixels, Render,
    Rgba, Styled as _, Window,
};

use crate::audio::waveform::BANDS;
use crate::ui::settings::WaveformStyle;

/// Resting thickness: silence looks the same in every style.
const REST_H: f32 = 3.0;
/// Headroom so a loud syllable does not pin a style to its ceiling.
const PEAK_SCALE: f32 = 0.8;
/// Height of the mic icon; the spectrum's columns stand on its foot.
const ICON_H: f32 = 11.0;
/// A step is never longer than this, so a stalled window does not lurch.
const MAX_DT: f32 = 0.1;
/// Below this a decaying value counts as zero.
const EPSILON: f32 = 0.003;

const BARS_MIN_H: f32 = 3.0;
const BARS_MAX_H: f32 = 16.0;
const BARS_MAX_W: f32 = 4.0;

const MEMO_BARS: usize = 28;
const EQ_SEGMENTS: usize = 10;
const SPECTRUM_MAX_BINS: usize = 30;

/// Where the `bars` style sits when its slot is wider than the bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    Start,
    #[default]
    Center,
}

/// A 60 Hz smoothing factor scaled to the real frame time.
pub(crate) fn ease(factor: f32, dt: f32) -> f32 {
    1.0 - (1.0 - factor).powf(dt * 60.0)
}

// Indices here are below 64, far inside f32's exact range.
#[allow(clippy::cast_precision_loss)]
pub(crate) const fn f(index: usize) -> f32 {
    index as f32
}

#[derive(Default)]
struct Bars {
    height: [f32; BANDS],
}

/// Symmetric bars scrolling right to left, the voice-message look.
struct Memo {
    history: [f32; MEMO_BARS],
    scroll: f32,
    phase: f32,
    peak_since: f32,
    slot: f32,
}

/// Retro stereo equalizer: LED segments with peak-hold dots.
#[derive(Default)]
struct Eq {
    smooth: [f32; BANDS],
    peak: [f32; BANDS],
    peak_velocity: [f32; BANDS],
    phase: f32,
}

/// Analyser columns, low frequencies left, interpolated from the 8 bands.
struct Spectrum {
    bins: usize,
    value: [f32; SPECTRUM_MAX_BINS],
    seed: [f32; SPECTRUM_MAX_BINS],
    time: f32,
}

enum Drawing {
    Bars(Bars),
    Memo(Memo),
    Eq(Eq),
    Spectrum(Spectrum),
}

impl Drawing {
    fn new(style: WaveformStyle) -> Self {
        match style {
            WaveformStyle::Bars => Self::Bars(Bars::default()),
            WaveformStyle::Memo => Self::Memo(Memo {
                history: [0.0; MEMO_BARS],
                scroll: 0.0,
                phase: 0.0,
                peak_since: 0.0,
                slot: 0.0,
            }),
            WaveformStyle::Eq => Self::Eq(Eq::default()),
            WaveformStyle::Spectrum => Self::Spectrum(Spectrum {
                bins: 0,
                value: [0.0; SPECTRUM_MAX_BINS],
                // Golden-angle phases decorrelate neighbouring columns.
                seed: std::array::from_fn(|i| (f(i) * 2.399_963) % std::f32::consts::TAU),
                time: 0.0,
            }),
        }
    }
}

struct Frame<'a> {
    bounds: Bounds<Pixels>,
    levels: &'a [f32; BANDS],
    dt: f32,
    accent: Rgba,
    idle_motion: bool,
    window: &'a mut Window,
}

impl Frame<'_> {
    fn width(&self) -> f32 {
        f32::from(self.bounds.size.width)
    }

    fn height(&self) -> f32 {
        f32::from(self.bounds.size.height)
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, radii: Corners<Pixels>, alpha: f32) {
        let origin = self.bounds.origin;
        let bounds = Bounds::new(
            point(origin.x + px(x), origin.y + px(y)),
            size(px(w), px(h)),
        );
        let color = Rgba {
            a: self.accent.a * alpha,
            ..self.accent
        };
        self.window
            .paint_quad(fill(bounds, color).corner_radii(radii));
    }

    fn dot(&mut self, x: f32, y: f32, radius: f32, alpha: f32) {
        self.rect(
            x - radius,
            y - radius,
            radius * 2.0,
            radius * 2.0,
            Corners::all(px(radius)),
            alpha,
        );
    }
}

fn rounded(radius: f32) -> Corners<Pixels> {
    Corners::all(px(radius))
}

impl Bars {
    fn draw(&mut self, frame: &mut Frame, gap: f32, align: Align) {
        let mut moving = false;
        for (height, &level) in self.height.iter_mut().zip(frame.levels) {
            let target = BARS_MIN_H + (BARS_MAX_H - BARS_MIN_H) * level;
            *height += (target - *height) * ease(0.35, frame.dt);
            moving |= (target - *height).abs() > 0.02;
        }
        if !moving {
            self.height = frame
                .levels
                .map(|l| BARS_MIN_H + (BARS_MAX_H - BARS_MIN_H) * l);
        }

        let count = f(BANDS);
        let width = ((frame.width() - gap * (count - 1.0)) / count).clamp(0.0, BARS_MAX_W);
        let run = width * count + gap * (count - 1.0);
        let left = match align {
            Align::Start => 0.0,
            Align::Center => (frame.width() - run) / 2.0,
        };
        let mid = frame.height() / 2.0;
        for (i, &height) in self.height.iter().enumerate() {
            let height = height.max(BARS_MIN_H);
            frame.rect(
                left + f(i) * (width + gap),
                mid - height / 2.0,
                width,
                height,
                rounded(width.min(height) / 2.0),
                1.0,
            );
        }
    }

    fn settled(&self, levels: &[f32; BANDS]) -> bool {
        self.height
            .iter()
            .zip(levels)
            .all(|(h, l)| (BARS_MIN_H + (BARS_MAX_H - BARS_MIN_H) * l - h).abs() < 0.02)
    }
}

impl Memo {
    const BAR_W: f32 = 1.8;

    fn draw(&mut self, frame: &mut Frame) {
        let slot = frame.width() / f(MEMO_BARS);
        if self.slot > 0.0 && (self.slot - slot).abs() > f32::EPSILON {
            self.scroll *= slot / self.slot;
        }
        self.slot = slot;
        self.phase += frame.dt;

        // Hold the peak between emitted bars so a short syllable is never missed.
        let loudest = frame.levels.iter().copied().fold(0.0, f32::max);
        self.peak_since = self.peak_since.max(loudest);

        self.scroll += 22.0 * frame.dt;
        while self.scroll >= slot {
            self.scroll -= slot;
            let idle = if frame.idle_motion {
                0.04 + ((self.phase * 2.1).sin() * 0.5 + 0.5) * 0.05
            } else {
                0.0
            };
            self.history.rotate_left(1);
            self.history[MEMO_BARS - 1] = self.peak_since.max(idle);
            self.peak_since = 0.0;
        }

        let mid = frame.height() / 2.0;
        let span = (frame.height() - 2.0) * PEAK_SCALE;
        let half_of = |level: f32| (REST_H / 2.0).max(level * span / 2.0);
        for (i, &level) in self.history.iter().enumerate() {
            let half = half_of(level);
            let x = f(i) * slot + slot / 2.0 - self.scroll;
            if x < -Self::BAR_W || x > frame.width() + Self::BAR_W {
                continue;
            }
            let recency = f(i) / f(MEMO_BARS - 1);
            let alpha = (0.32 + recency * 0.55 + level * 0.18).min(1.0);
            // A round line cap adds half the width at each end.
            frame.rect(
                x - Self::BAR_W / 2.0,
                mid - half - Self::BAR_W / 2.0,
                Self::BAR_W,
                half * 2.0 + Self::BAR_W,
                rounded(Self::BAR_W / 2.0),
                alpha,
            );
        }

        let lead_x = f(MEMO_BARS - 1) * slot + slot / 2.0 - self.scroll;
        let lead_half = half_of(self.history[MEMO_BARS - 1]);
        frame.dot(lead_x, mid - lead_half, 1.3, 1.0);
    }

    fn settled(&self, levels: &[f32; BANDS]) -> bool {
        self.peak_since == 0.0
            && levels.iter().all(|&l| l == 0.0)
            && self.history.iter().all(|&v| v == 0.0)
    }
}

impl Eq {
    fn draw(&mut self, frame: &mut Frame) {
        self.phase += frame.dt;
        for i in 0..BANDS {
            let level = frame.levels[i];
            let rising = level > self.smooth[i];
            let factor = if rising { 0.55 } else { 0.10 + f(i) * 0.006 };
            self.smooth[i] += (level - self.smooth[i]) * ease(factor, frame.dt);
            if level == 0.0 && self.smooth[i] < EPSILON {
                self.smooth[i] = 0.0;
            }

            if self.smooth[i] > self.peak[i] {
                self.peak[i] = self.smooth[i];
                self.peak_velocity[i] = 0.0;
            } else {
                self.peak_velocity[i] += 0.9 * frame.dt;
                self.peak[i] -= self.peak_velocity[i] * frame.dt;
                if self.peak[i] < self.smooth[i] + EPSILON {
                    self.peak[i] = self.smooth[i];
                    self.peak_velocity[i] = 0.0;
                }
            }
        }

        let slot = frame.width() / f(BANDS);
        let bar_w = (slot - 1.4).max(1.2);
        // Scale the ladder, not just how many rungs light.
        let seg_h = (frame.height() - 2.0) * PEAK_SCALE / f(EQ_SEGMENTS);
        let mid = frame.height() / 2.0;
        let span = (frame.height() - 2.0) * PEAK_SCALE;

        for i in 0..BANDS {
            let cx = f(i) * slot + slot / 2.0;
            let left = cx - bar_w / 2.0;
            let lit_count = self.smooth[i] * f(EQ_SEGMENTS);
            let idle = if frame.idle_motion {
                0.10 + ((self.phase * 2.0 + f(i) * 0.6).sin() * 0.5 + 0.5) * 0.10
            } else {
                0.0
            };
            let base_lit = self.smooth[i].max(idle);

            // The segment grid goes dark at zero, which reads as broken.
            frame.rect(left, mid - REST_H / 2.0, bar_w, REST_H, rounded(0.0), 0.85);

            for seg in 0..EQ_SEGMENTS {
                let seg_mid = (f(seg) + 0.5) / f(EQ_SEGMENTS);
                let half_step = 0.5 / f(EQ_SEGMENTS);
                let lit = base_lit >= seg_mid + half_step;
                let half_lit = !lit && base_lit > seg_mid - half_step;
                if !lit && !half_lit {
                    continue;
                }
                let half = (f(seg) + 1.0) * seg_h / 2.0;
                let alpha = if lit {
                    1.0
                } else {
                    (base_lit - seg_mid) * f(EQ_SEGMENTS) * 0.9
                };
                let alpha = 0.85 * alpha;
                frame.rect(left, mid - half, bar_w, seg_h * 0.78, rounded(0.0), alpha);
                frame.rect(
                    left,
                    mid + half - seg_h * 0.78,
                    bar_w,
                    seg_h * 0.78,
                    rounded(0.0),
                    alpha,
                );
            }

            if self.peak[i] * f(EQ_SEGMENTS) > 0.05 {
                let peak_half = self.peak[i] * span / 2.0;
                frame.dot(cx, mid - peak_half, 1.1, 1.0);
                frame.dot(cx, mid + peak_half, 1.1, 1.0);
            }

            if lit_count > 0.5 {
                frame.dot(cx, mid - lit_count.min(f(EQ_SEGMENTS)) * seg_h, 2.2, 0.18);
            }
        }
    }

    fn settled(&self, levels: &[f32; BANDS]) -> bool {
        levels.iter().all(|&l| l == 0.0)
            && self.smooth.iter().all(|&v| v == 0.0)
            && self.peak.iter().all(|&v| v == 0.0)
    }
}

impl Spectrum {
    /// The bands interpolated at `t`, tilted up toward the quieter highs.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0 <= x < BANDS
    fn band_at(levels: &[f32; BANDS], t: f32) -> f32 {
        let x = t * f(BANDS - 1);
        let i = (x.floor() as usize).min(BANDS - 2);
        let frac = x - f(i);
        let level = levels[i] * (1.0 - frac) + levels[i + 1] * frac;
        level * (1.0 + t * 1.15)
    }

    /// Re-grid `values` over `bins` columns, keeping its shape.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0 <= x < `from`
    fn resample(values: &mut [f32; SPECTRUM_MAX_BINS], from: usize, to: usize) {
        let old = *values;
        for (i, value) in values.iter_mut().enumerate().take(to) {
            let x = f(i) * f(from - 1) / f(to - 1);
            let left = x.floor() as usize;
            let right = (left + 1).min(from - 1);
            *value = old[left] + (old[right] - old[left]) * (x - f(left));
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a width of tens of pixels
    fn draw(&mut self, frame: &mut Frame) {
        let bins = ((frame.width() / 3.0).round() as usize).clamp(12, SPECTRUM_MAX_BINS);
        if self.bins == 0 {
            self.bins = bins;
        } else if self.bins != bins {
            Self::resample(&mut self.value, self.bins, bins);
            Self::resample(&mut self.seed, self.bins, bins);
            self.bins = bins;
        }
        self.time += frame.dt;

        let slot = frame.width() / f(bins);
        let col_w = (slot * 0.55).clamp(1.4, 2.6);
        let radius = col_w / 2.0;
        let floor_y = frame.height() / 2.0 + ICON_H / 2.0;
        let max_h = (floor_y - 1.0) * PEAK_SCALE;
        let all_silent = frame.levels.iter().all(|&l| l == 0.0);

        for i in 0..bins {
            let t = f(i) / f(bins - 1);
            let jitter = 0.72 + ((self.time * 9.0 + self.seed[i]).sin() * 0.5 + 0.5) * 0.55;
            let idle = if frame.idle_motion {
                0.05 + ((self.time * 1.8 + f(i) * 0.4).sin() * 0.5 + 0.5) * 0.06
            } else {
                0.0
            };
            let target = (Self::band_at(frame.levels, t) * jitter).max(idle).min(1.0);
            let rising = target > self.value[i];
            self.value[i] +=
                (target - self.value[i]) * ease(if rising { 0.55 } else { 0.16 }, frame.dt);
            if all_silent && !frame.idle_motion && self.value[i] < EPSILON {
                self.value[i] = 0.0;
            }

            let x = f(i) * slot + (slot - col_w) / 2.0;
            let height = (self.value[i] * max_h).max(REST_H);
            frame.rect(
                x,
                floor_y - height,
                col_w,
                height,
                Corners {
                    top_left: px(radius),
                    top_right: px(radius),
                    bottom_right: px(0.0),
                    bottom_left: px(0.0),
                },
                0.92,
            );
        }
    }

    fn settled(&self, levels: &[f32; BANDS]) -> bool {
        levels.iter().all(|&l| l == 0.0) && self.value.iter().all(|&v| v == 0.0)
    }
}

pub struct Waveform {
    drawing: Drawing,
    style: WaveformStyle,
    accent: Rgba,
    idle_motion: bool,
    /// Frames come from the owner's clock, not from paint.
    paced: bool,
    bars_gap: f32,
    bars_align: Align,
    opacity: f32,
    levels: [f32; BANDS],
    last_frame: Option<Instant>,
}

impl Waveform {
    pub fn new(style: WaveformStyle, accent: Rgba, idle_motion: bool) -> Self {
        Self {
            drawing: Drawing::new(style),
            style,
            accent,
            idle_motion,
            paced: false,
            bars_gap: 2.0,
            bars_align: Align::Center,
            opacity: 1.0,
            levels: [0.0; BANDS],
            last_frame: None,
        }
    }

    /// The owner calls [`Self::set_levels`] on its own clock, once per frame.
    #[must_use]
    pub const fn paced(mut self) -> Self {
        self.paced = true;
        self
    }

    /// The newest levels, 0 to 1 per band.
    pub fn set_levels(&mut self, levels: [f32; BANDS], cx: &mut Context<Self>) {
        let levels = levels.map(|l| {
            if l.is_finite() {
                l.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        if self.paced
            || levels
                .iter()
                .zip(&self.levels)
                .any(|(a, b)| (a - b).abs() > f32::EPSILON)
        {
            self.levels = levels;
            cx.notify();
        }
    }

    pub fn set_style(&mut self, style: WaveformStyle, cx: &mut Context<Self>) {
        if style != self.style {
            self.style = style;
            self.reset(cx);
        }
    }

    pub fn set_accent(&mut self, accent: Rgba, cx: &mut Context<Self>) {
        if accent != self.accent {
            self.accent = accent;
            cx.notify();
        }
    }

    /// How the `bars` style spaces itself in its slot; the other styles fill it.
    pub fn set_bars_layout(&mut self, gap: f32, align: Align, cx: &mut Context<Self>) {
        if (gap - self.bars_gap).abs() > f32::EPSILON || align != self.bars_align {
            self.bars_gap = gap;
            self.bars_align = align;
            cx.notify();
        }
    }

    pub fn set_opacity(&mut self, opacity: f32, cx: &mut Context<Self>) {
        if (opacity - self.opacity).abs() > f32::EPSILON {
            self.opacity = opacity;
            cx.notify();
        }
    }

    /// Forget the scroll and peak history, as a fresh recording starts flat.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.drawing = Drawing::new(self.style);
        self.levels = [0.0; BANDS];
        self.last_frame = None;
        cx.notify();
    }

    /// Step the drawing to now and paint it. Returns whether it has settled.
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window) -> bool {
        let now = Instant::now();
        let dt = self.last_frame.map_or(0.0, |last| {
            now.duration_since(last).as_secs_f32().min(MAX_DT)
        });
        self.last_frame = Some(now);

        let levels = self.levels;
        let mut frame = Frame {
            bounds,
            levels: &levels,
            dt,
            accent: Rgba {
                a: self.accent.a * self.opacity,
                ..self.accent
            },
            idle_motion: self.idle_motion,
            window,
        };
        let settled = match &mut self.drawing {
            Drawing::Bars(bars) => {
                bars.draw(&mut frame, self.bars_gap, self.bars_align);
                bars.settled(&levels)
            }
            Drawing::Memo(memo) => {
                memo.draw(&mut frame);
                memo.settled(&levels)
            }
            Drawing::Eq(eq) => {
                eq.draw(&mut frame);
                eq.settled(&levels)
            }
            Drawing::Spectrum(spectrum) => {
                spectrum.draw(&mut frame);
                spectrum.settled(&levels)
            }
        };
        let settled = settled && !self.idle_motion;
        if settled {
            // The next frame after a rest starts from zero, not from the pause.
            self.last_frame = None;
        }
        settled
    }
}

impl Render for Waveform {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this: Entity<Self> = cx.entity();
        let paced = self.paced;
        canvas(
            |_, _, _| (),
            move |bounds, (), window, cx| {
                let settled = this.update(cx, |waveform, _| waveform.paint(bounds, window));
                if !settled && !paced {
                    window.request_animation_frame();
                }
            },
        )
        .size_full()
    }
}
