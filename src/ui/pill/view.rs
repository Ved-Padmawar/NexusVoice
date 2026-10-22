//! The pill: a pure view of the controller's [`PillState`].
//!
//! Frames are requested only while something moves: a geometry or colour
//! tween, the spinner, a drag. The waveform and the transcript are entities of
//! their own, so a 30 Hz level update or a new partial repaints just that one.

use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, percentage, point, px, relative, size, svg, Animation,
    AnimationExt as _, AnyElement, BoxShadow, ClickEvent, Context, CursorStyle, Entity,
    FontWeight, MouseButton, MouseDownEvent, Pixels, Rgba, SharedString, StyleRefinement, Task,
    Transformation, Window,
};

use super::geometry::{
    capsule_width, capsule_window, card_height, card_window, BOTTOM_SLACK, CAPSULE_H, CARD_RADIUS,
    CARD_W,
};
use super::motion::{Curve, Transition, Tween};
use super::native::{Drag, NativeWindow};
use super::theme::{self, ButtonColors, Look, PillThemeDef, DOWNLOAD, DOWNLOAD_PCT};
use super::transcript::Transcript;
use super::waveform::{Align, Waveform};
use crate::events::{Event, Partial};
use crate::transcription::controller::{self, Input, PillState};
use crate::ui::settings::{PillTheme, UiSettings, WaveformStyle};

const SHAPE: Transition = Transition::new(0.28, Curve::SHAPE);
const FADE_IN_DELAY: f32 = 0.14;
const TIDE_H: f32 = 88.0;
const TIDE_ALPHA: f32 = 0.16;
const PAUSED_TIDE: f32 = 0.4;
const PAUSED_WAVE: f32 = 0.45;
/// The transcript's top edge fades out over this many pixels.
const FADE_H: f32 = 28.0;
const STRIP_H: f32 = 42.0;
const NOTICE_SHOWN: Duration = Duration::from_secs(3);
const KEEP_ON_TOP: Duration = Duration::from_secs(2);

const PAUSE_ICON: &str = "icons/pill/pause.svg";
const PLAY_ICON: &str = "icons/pill/play.svg";
const CHECK_ICON: &str = "icons/pill/check.svg";
const MIC_ICON: &str = "icons/pill/mic.svg";
const SPINNER_ICON: &str = "icons/pill/spinner.svg";

/// Every tween the pill owns, sampled once per render.
struct Motion {
    width: Tween,
    height: Tween,
    radius: Tween,
    tide_height: Tween,
    tide_opacity: Tween,
    transcript_opacity: Tween,
    wave_opacity: Tween,
    /// The capsule's contents fading back in after a card closes.
    strip_opacity: Tween,
    /// Progress from `look_from` to `look_to`.
    look_progress: Tween,
    look_from: Look,
    look_to: Look,
    /// Pause and commit: how far the pointer's hover has eased in.
    hover: [Tween; 2],
}

impl Motion {
    fn new(look: Look, width: f32) -> Self {
        Self {
            width: Tween::new(width),
            height: Tween::new(CAPSULE_H),
            radius: Tween::new(CAPSULE_H / 2.0),
            tide_height: Tween::new(0.0),
            tide_opacity: Tween::new(0.0),
            transcript_opacity: Tween::new(0.0),
            wave_opacity: Tween::new(1.0),
            strip_opacity: Tween::new(1.0),
            look_progress: Tween::new(1.0),
            look_from: look,
            look_to: look,
            hover: [Tween::new(0.0), Tween::new(0.0)],
        }
    }

    fn look(&self, now: Instant) -> Look {
        self.look_from
            .mix(self.look_to, self.look_progress.value(now))
    }

    fn tweens(&self) -> [&Tween; 11] {
        [
            &self.width,
            &self.height,
            &self.radius,
            &self.tide_height,
            &self.tide_opacity,
            &self.transcript_opacity,
            &self.wave_opacity,
            &self.strip_opacity,
            &self.look_progress,
            &self.hover[0],
            &self.hover[1],
        ]
    }

    fn is_active(&self, now: Instant) -> bool {
        self.tweens().iter().any(|tween| tween.is_active(now))
    }

    /// The card's shape is still travelling.
    fn is_morphing(&self, now: Instant) -> bool {
        [&self.width, &self.height, &self.radius]
            .iter()
            .any(|tween| tween.is_active(now))
    }
}

pub struct PillView {
    state: PillState,
    theme: PillTheme,
    waveform_style: WaveformStyle,
    live_transcript: bool,
    notice: Option<SharedString>,
    notice_timer: Option<Task<()>>,
    has_partial: bool,
    /// "42%" for the downloading spinner, formatted when it changes.
    percent: SharedString,
    waveform: Entity<Waveform>,
    transcript: Entity<Transcript>,
    motion: Motion,
    native: NativeWindow,
    /// The OS window is card-sized, not capsule-sized.
    window_large: bool,
    /// The layout has switched to the card's column. Lags a request to expand
    /// until the bigger window has painted, and a collapse until it is done.
    roomy: bool,
    /// Waiting for the bigger window to paint before expanding.
    opening: bool,
    /// The window has room above the capsule for the notice.
    headroom: bool,
    /// Whether the card was open at the last reconcile.
    was_expanded: bool,
    was_returning: bool,
    drag: Option<Drag>,
}

impl PillView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = UiSettings::global(cx);
        let (theme_id, style, live) = (
            settings.pill_theme,
            settings.waveform_style,
            settings.live_transcript,
        );
        let theme = theme::def(theme_id);

        cx.observe_global_in::<UiSettings>(window, |this, window, cx| {
            this.apply_settings(window, cx);
        })
        .detach();
        #[cfg(windows)]
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(KEEP_ON_TOP).await;
            if this.update(cx, |this, _| this.native.keep_on_top()).is_err() {
                return;
            }
        })
        .detach();

        let state = PillState::Idle;
        let look = theme.look(&state);
        Self {
            motion: Motion::new(look, capsule_width(&state)),
            state,
            theme: theme_id,
            waveform_style: style,
            live_transcript: live,
            notice: None,
            notice_timer: None,
            has_partial: false,
            percent: SharedString::default(),
            waveform: cx.new(|_| Waveform::new(style, theme.accent, false)),
            transcript: cx.new(|_| Transcript::new(theme)),
            native: NativeWindow::new(window),
            window_large: false,
            roomy: false,
            opening: false,
            headroom: false,
            was_expanded: false,
            was_returning: false,
            drag: None,
        }
    }

    fn def(&self) -> &'static PillThemeDef {
        theme::def(self.theme)
    }

    fn capturing(&self) -> bool {
        self.state.is_capturing()
    }

    fn paused(&self) -> bool {
        self.state == PillState::Dictation { paused: true }
    }

    fn dictating(&self) -> bool {
        matches!(self.state, PillState::Dictation { .. })
    }

    fn wants_card(&self) -> bool {
        self.live_transcript && self.capturing() && self.has_partial
    }

    fn expanded(&self) -> bool {
        self.wants_card() && self.roomy
    }

    pub fn handle(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::Pill(state) => self.set_state(state, window, cx),
            Event::PillNotice(text) => self.show_notice(text.into(), window, cx),
            Event::Waveform(levels) => {
                if self.capturing() {
                    self.waveform.update(cx, |wave, cx| wave.set_levels(levels, cx));
                }
            }
            Event::Partial(partial) => self.set_partial(partial, window, cx),
            Event::PartialEnd => self.clear_partial(window, cx),
            _ => {}
        }
    }

    fn set_state(&mut self, state: PillState, window: &mut Window, cx: &mut Context<Self>) {
        if state == self.state {
            return;
        }
        let was_capturing = self.capturing();
        self.state = state;
        if let PillState::Downloading(percent) = self.state {
            self.percent = format!("{percent}%").into();
        }
        if self.capturing() && !was_capturing {
            self.waveform.update(cx, |wave, cx| wave.reset(cx));
        }
        // Leaving the capture states drops the partial; the card's exit still
        // shows the last text, which the transcript keeps.
        if !self.capturing() {
            self.has_partial = false;
        }
        self.retarget_look(cx);
        self.native.keep_on_top();
        self.reconcile(window, cx);
        cx.notify();
    }

    fn set_partial(&mut self, partial: Partial, window: &mut Window, cx: &mut Context<Self>) {
        if !self.capturing() || (partial.committed.is_empty() && partial.tentative.is_empty()) {
            return;
        }
        self.transcript
            .update(cx, |transcript, cx| transcript.set_partial(partial, window, cx));
        self.has_partial = true;
        self.reconcile(window, cx);
        cx.notify();
    }

    fn clear_partial(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_partial {
            self.has_partial = false;
            self.reconcile(window, cx);
            cx.notify();
        }
    }

    fn show_notice(&mut self, text: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.notice = Some(text);
        self.notice_timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(NOTICE_SHOWN).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.notice = None;
                this.reconcile(window, cx);
                cx.notify();
            });
        }));
        self.reconcile(window, cx);
        cx.notify();
    }

    fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = UiSettings::global(cx);
        let (theme_id, style, live) = (
            settings.pill_theme,
            settings.waveform_style,
            settings.live_transcript,
        );
        let theme = theme::def(theme_id);
        if theme_id != self.theme {
            self.theme = theme_id;
            self.waveform
                .update(cx, |wave, cx| wave.set_accent(theme.accent, cx));
            self.transcript
                .update(cx, |transcript, cx| transcript.set_theme(theme, window, cx));
            self.retarget_look(cx);
        }
        if style != self.waveform_style {
            self.waveform_style = style;
            self.waveform.update(cx, |wave, cx| wave.set_style(style, cx));
        }
        self.live_transcript = live;
        self.reconcile(window, cx);
        cx.notify();
    }

    fn transition(cx: &Context<Self>, transition: Transition) -> Transition {
        if cx.reduce_motion() {
            Transition::INSTANT
        } else {
            transition
        }
    }

    fn retarget_look(&mut self, cx: &Context<Self>) {
        let now = Instant::now();
        let to = self.def().look(&self.state);
        let motion = &mut self.motion;
        motion.look_from = motion.look(now);
        motion.look_to = to;
        motion.look_progress.snap(0.0, now);
        motion.look_progress.retarget(
            1.0,
            now,
            Self::transition(cx, Transition::new(0.18, Curve::EASE)),
        );
    }

    /// Bring the window, the layout and every tween in line with what the
    /// pill now wants. Safe to call again at any time.
    fn reconcile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let wants_card = self.wants_card();

        if self.notice.is_some() {
            self.resize_window(window, true);
            self.headroom = true;
        }
        if wants_card && !self.roomy && !self.opening {
            self.resize_window(window, true);
            self.opening = true;
            // Two frames, so the bigger window has painted before the card
            // grows into it.
            let this = cx.entity();
            window.on_next_frame(move |window, _| {
                window.on_next_frame(move |window, cx| {
                    this.update(cx, |this, cx| this.finish_opening(window, cx));
                });
            });
        }
        let busy = wants_card || self.roomy || self.opening || self.notice.is_some();
        if !busy && self.window_large {
            self.resize_window(window, false);
            self.headroom = false;
        }

        self.waveform.update(cx, |wave, cx| {
            let (gap, align) = match (self.dictating(), self.roomy) {
                (true, _) => (1.5, Align::Center),
                (false, true) => (2.0, Align::Start),
                (false, false) => (2.0, Align::Center),
            };
            wave.set_bars_layout(gap, align, cx);
        });
        self.retarget_motion(now, cx);
    }

    fn finish_opening(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.opening = false;
        if self.wants_card() {
            self.roomy = true;
            self.reconcile(window, cx);
            cx.notify();
        }
    }

    fn resize_window(&mut self, window: &mut Window, large: bool) {
        if self.window_large == large {
            return;
        }
        let (width, height) = if large {
            card_window()
        } else {
            capsule_window()
        };
        self.native
            .resize(window, size(px(width), px(height)));
        self.window_large = large;
    }

    fn retarget_motion(&mut self, now: Instant, cx: &mut Context<Self>) {
        let expanded = self.expanded();
        let shape = Self::transition(cx, SHAPE);
        let fade = |seconds: f32| Self::transition(cx, Transition::new(seconds, Curve::EASE));
        let text_height = self.transcript.read(cx).height();
        let paused = self.paused();
        let returning = self.roomy && !expanded && self.state == PillState::Idle;

        let (width, height, radius) = if expanded {
            (CARD_W, card_height(text_height), CARD_RADIUS)
        } else {
            (capsule_width(&self.state), CAPSULE_H, CAPSULE_H / 2.0)
        };
        let m = &mut self.motion;
        m.width.retarget(width, now, shape);
        m.height.retarget(height, now, shape);
        m.radius.retarget(radius, now, shape);

        if expanded {
            if !self.was_expanded {
                m.tide_height.snap(0.0, now);
                m.tide_opacity.snap(0.0, now);
                m.transcript_opacity.snap(0.0, now);
            }
            m.tide_height.retarget(TIDE_H, now, shape);
            m.tide_opacity
                .retarget(if paused { PAUSED_TIDE } else { 1.0 }, now, fade(0.2));
            m.transcript_opacity
                .retarget(1.0, now, fade(0.24).delayed(FADE_IN_DELAY));
        } else {
            m.tide_opacity.retarget(0.0, now, fade(0.09));
            m.transcript_opacity.retarget(0.0, now, fade(0.09));
        }
        m.wave_opacity
            .retarget(if paused { PAUSED_WAVE } else { 1.0 }, now, fade(0.12));

        // On a direct card to idle result the capsule's contents appear only
        // once the shell is nearly compact; at full opacity in the wide card
        // the label would visibly sweep inward.
        if returning && !self.was_returning {
            m.strip_opacity.snap(0.0, now);
            m.strip_opacity.retarget(
                1.0,
                now,
                Self::transition(cx, Transition::new(0.14, Curve::EASE_OUT).delayed(0.1)),
            );
        } else if !returning {
            m.strip_opacity.snap(1.0, now);
        }
        self.was_expanded = expanded;
        self.was_returning = returning;

        let caret = expanded && !paused;
        self.transcript
            .update(cx, |transcript, cx| transcript.set_caret(caret, cx));
    }

    /// Everything that advances with the clock, once per frame: the collapse
    /// finishing, the drag, the children that draw a fade, the next frame.
    fn tick(&mut self, now: Instant, window: &mut Window, cx: &mut Context<Self>) {
        if self.roomy && !self.wants_card() && !self.motion.is_morphing(now) {
            self.roomy = false;
            self.reconcile(window, cx);
        }

        let mut busy = self.motion.is_active(now);
        if let Some(drag) = self.drag {
            if self.native.drag(drag, window) {
                busy = true;
            } else {
                self.drag = None;
            }
        }
        // The children are cached, so a fade has to be handed to them.
        let (wave, text) = (
            self.motion.wave_opacity.value(now),
            self.motion.transcript_opacity.value(now),
        );
        self.waveform.update(cx, |w, cx| w.set_opacity(wave, cx));
        self.transcript.update(cx, |t, cx| t.set_opacity(text, cx));
        if busy {
            window.request_animation_frame();
        }
    }

    fn begin_drag(&mut self, _: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.native.is_movable() {
            self.drag = self.native.begin_drag(window);
            cx.notify();
        } else {
            window.start_window_move();
        }
    }

    fn hover(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        let transition = Self::transition(cx, Transition::new(0.12, Curve::EASE));
        self.motion.hover[index].retarget(f32::from(u8::from(hovered)), Instant::now(), transition);
        cx.notify();
    }
}

fn shadows(theme: &PillThemeDef, amount: f32) -> Vec<BoxShadow> {
    if amount <= 0.001 {
        return Vec::new();
    }
    theme
        .shadow
        .iter()
        .map(|shadow| {
            let built = BoxShadow::new(
                px(0.0),
                px(shadow.offset_y),
                theme::faded(shadow.color, amount).into(),
            )
            .blur_radius(px(shadow.blur));
            if shadow.inset {
                built.inset()
            } else {
                built
            }
        })
        .collect()
}

fn full() -> StyleRefinement {
    StyleRefinement::default().size_full()
}

impl PillView {
    fn icon(path: &'static str, size: f32, color: Rgba) -> impl IntoElement {
        svg()
            .path(path)
            .size(px(size))
            .flex_none()
            .text_color(color)
    }

    fn label(text: &'static str, size: f32, weight: FontWeight, color: Rgba) -> impl IntoElement {
        div()
            .flex_none()
            .whitespace_nowrap()
            .text_size(px(size))
            .font_weight(weight)
            .text_color(color)
            .child(text)
    }

    /// The waveform's slot: fills what the row leaves in the capsule, a fixed
    /// 54px in the card. Canvas styles draw into a box of their own width,
    /// centred even where it overflows the slot; `bars` fills it.
    fn wave_slot(&self, dictation: bool) -> impl IntoElement {
        let canvas_style = self.waveform_style != WaveformStyle::Bars;
        let canvas_w = if self.roomy {
            54.0
        } else if dictation {
            45.0
        } else {
            42.0
        };
        let slot = div()
            .h(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .when(self.roomy, |slot| {
                slot.w(px(54.0)).flex_none().overflow_hidden()
            })
            .when(!self.roomy, |slot| slot.flex_1().min_w_0())
            .when(dictation && !self.roomy, |slot| {
                slot.ml(px(if canvas_style { 3.0 } else { 2.0 }))
            });
        let inner = if canvas_style {
            div().w(px(canvas_w)).h(px(20.0)).flex_none()
        } else {
            div().size_full()
        };
        slot.child(inner.child(self.waveform.clone().cached(full())))
    }

    fn control(
        &self,
        index: usize,
        colors: ButtonColors,
        icon: &'static str,
        icon_size: f32,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let hover = self.motion.hover[index].value(now);
        let fg = theme::mix(colors.fg, colors.hover_fg, hover);
        let bg = theme::mix(colors.bg, colors.hover_bg, hover);
        let input = if index == 0 {
            Input::DictationToggle
        } else {
            Input::DictationCommit
        };
        // The slot stays 20px so a pressed button shrinks in place.
        div()
            .flex_none()
            .size(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id(("pill-control", index))
                    .size(px(20.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(bg)
                    .cursor_pointer()
                    .active(|style| style.size(px(18.8)))
                    // The pill drags from anywhere but its buttons.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hover(index, *hovered, cx);
                    }))
                    .on_click(move |_: &ClickEvent, _, _| controller::send(input.clone()))
                    .child(Self::icon(icon, icon_size, fg)),
            )
    }

    /// Pause/resume, waveform and commit. In the card they sit in a 22/1fr/22
    /// row with the waveform hugging the pause button; in the capsule they
    /// fill it edge to edge.
    fn dictation(&self, theme: &PillThemeDef, now: Instant, cx: &mut Context<Self>) -> AnyElement {
        let (icon, icon_size) = if self.paused() {
            (PLAY_ICON, 11.0)
        } else {
            (PAUSE_ICON, 11.0)
        };
        let pause = self.control(0, theme.control, icon, icon_size, now, cx);
        let commit = self.control(1, theme.commit, CHECK_ICON, 12.0, now, cx);
        if self.roomy {
            let cell = |child: AnyElement| div().flex_none().w(px(22.0)).child(child);
            div()
                .flex_none()
                .h(px(STRIP_H))
                .pl(px(15.0))
                .pr(px(13.0))
                .pb(px(4.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(cell(pause.into_any_element()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .child(self.wave_slot(true)),
                )
                .child(cell(commit.into_any_element()))
                .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .gap(px(3.0))
                .child(pause)
                .child(self.wave_slot(true))
                .child(commit)
                .into_any_element()
        }
    }
}

impl PillView {
    /// The icon, brand label, waveform or error label: whatever the state
    /// puts in the capsule's row or the card's strip.
    fn strip_items(&self, theme: &PillThemeDef) -> Vec<AnyElement> {
        let mic = Self::icon(MIC_ICON, 11.0, theme.accent).into_any_element();
        match self.state {
            PillState::Idle => vec![
                mic,
                Self::label("NexusVoice", 11.0, FontWeight::SEMIBOLD, theme.brand)
                    .into_any_element(),
            ],
            PillState::Recording => {
                let mut items = vec![mic, self.wave_slot(false).into_any_element()];
                if self.roomy {
                    // `margin-left: auto` pushes the hint to the strip's end.
                    items.push(
                        div()
                            .ml_auto()
                            .child(Self::label(
                                "Hold to speak",
                                10.0,
                                FontWeight::MEDIUM,
                                theme.hint,
                            ))
                            .into_any_element(),
                    );
                }
                items
            }
            PillState::Error(_) => vec![
                mic,
                Self::label("Error", 10.0, FontWeight::SEMIBOLD, theme.error_label)
                    .into_any_element(),
            ],
            PillState::Dictation { .. } | PillState::Processing | PillState::Downloading(_) => {
                Vec::new()
            }
        }
    }

    /// The card's strip: the capsule's row at the capsule's size while the
    /// card closes, the roomier one while it is open.
    fn strip(&self, closing: bool, opacity: f32) -> gpui_kit::Div {
        let (height, left, right, bottom) = if closing {
            (32.0, 8.0, 10.0, 0.0)
        } else {
            (STRIP_H, 15.0, 13.0, 4.0)
        };
        div()
            .flex_none()
            .h(px(height))
            .max_h_full()
            .pl(px(left))
            .pr(px(right))
            .pb(px(bottom))
            .flex()
            .items_center()
            .opacity(opacity)
    }

    /// The accent wash pooling at the card's bottom edge.
    fn tide(&self, theme: &PillThemeDef, now: Instant, inner_h: f32, radius: f32) -> impl IntoElement {
        let height = self.motion.tide_height.value(now).min(inner_h);
        let alpha = TIDE_ALPHA * self.motion.tide_opacity.value(now);
        let accent = |a: f32| Rgba { a, ..theme.accent };
        let wash = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(height))
            .bg(linear_gradient(
                0.0,
                linear_color_stop(accent(alpha), 0.0),
                linear_color_stop(accent(0.0), 1.0),
            ));
        // The pill clips to its rounded shape; a rectangle must round its own corners.
        let inner = (radius - 1.0).max(0.0);
        if height >= inner_h {
            wash.rounded(px(inner))
        } else {
            wash.rounded_bl(px(inner)).rounded_br(px(inner))
        }
    }

    /// The transcript's top edge fading into the pill's own colour, the way
    /// the old mask faded the text. It rounds its corners with the pill, which
    /// clips only to a rectangle.
    fn transcript_fade(
        &self,
        theme: &PillThemeDef,
        look: &Look,
        now: Instant,
        inner_h: f32,
        radius: f32,
    ) -> impl IntoElement {
        // What is under the fade: the surface, tinted by the tide where it reaches.
        let tide = TIDE_ALPHA * self.motion.tide_opacity.value(now);
        let tide_h = self.motion.tide_height.value(now);
        let distance = inner_h - FADE_H / 2.0;
        let tint = if distance < tide_h {
            tide * (1.0 - distance / tide_h)
        } else {
            0.0
        };
        let surface = theme::mix(
            Rgba { a: 1.0, ..look.bg },
            Rgba {
                a: 1.0,
                ..theme.accent
            },
            tint,
        );
        let clear = Rgba { a: 0.0, ..surface };
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(FADE_H))
            .rounded_t(px((radius - 1.0).max(0.0)))
            .bg(linear_gradient(
                180.0,
                linear_color_stop(surface, 0.0),
                linear_color_stop(clear, 1.0),
            ))
    }

    fn transcript(&self) -> impl IntoElement {
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .w(px(CARD_W))
            .child(self.transcript.clone().cached(full()))
    }

    fn spinner(&self, theme: &PillThemeDef) -> Option<AnyElement> {
        let color = match self.state {
            PillState::Processing => theme.spinner,
            PillState::Downloading(_) => DOWNLOAD,
            _ => return None,
        };
        let ring = svg()
            .path(SPINNER_ICON)
            .size(px(22.0))
            .text_color(color)
            .with_animation(
                "pill-spinner",
                Animation::new(Duration::from_secs(1)).repeat(),
                |ring, turn| ring.with_transformation(Transformation::rotate(percentage(turn))),
            );
        let slot = div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(ring);
        Some(
            slot.when(matches!(self.state, PillState::Downloading(_)), |slot| {
                slot.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(7.0))
                        .font_weight(FontWeight::BOLD)
                        .line_height(relative(1.0))
                        .text_color(DOWNLOAD_PCT)
                        .child(self.percent.clone()),
                )
            })
            .into_any_element(),
        )
    }

    /// The notice bubble: above the capsule, or inside the card's top edge
    /// where the card fills the window.
    fn notice(&self, text: &SharedString) -> impl IntoElement {
        let bubble = div()
            .max_w(px(300.0))
            .px(px(10.0))
            .py(px(5.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(Rgba { a: 0.4, ..DOWNLOAD })
            .bg(theme::NOTICE_BG)
            .text_size(px(9.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::NOTICE_TEXT)
            .text_center()
            .shadow(vec![BoxShadow::new(
                px(0.0),
                px(4.0),
                theme::faded(Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }, 0.5).into(),
            )
            .blur_radius(px(16.0))])
            .child(text.clone());
        div()
            .absolute()
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .when(self.roomy, |row| row.top(px(8.0)))
            .when(!self.roomy, |row| row.bottom(relative(1.0)).mb(px(8.0)))
            .child(bubble)
    }
}

impl Render for PillView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        self.tick(now, window, cx);

        let theme = self.def();
        let look = self.motion.look(now);
        let width = self.motion.width.value(now);
        let height = self.motion.height.value(now);
        let radius = self.motion.radius.value(now);
        let roomy = self.roomy;
        let closing = roomy && !self.expanded();
        let strip_opacity = self.motion.strip_opacity.value(now);
        let inner_h = height - 2.0;

        let mut pill = div()
            .id("pill")
            .relative()
            .flex()
            .flex_none()
            .overflow_hidden()
            .w(px(width))
            .h(px(height))
            .rounded(px(radius))
            .border_1()
            .border_color(look.border)
            .bg(look.bg)
            .shadow(shadows(theme, look.shadow))
            .cursor_grab()
            .active(|style| style.cursor_grabbing())
            .on_mouse_down(MouseButton::Left, cx.listener(Self::begin_drag));

        let has_strip = !matches!(
            self.state,
            PillState::Processing | PillState::Downloading(_)
        );
        if roomy {
            pill = pill.flex_col().justify_end().child(self.tide(theme, now, inner_h, radius));
            let fading = self.motion.transcript_opacity.value(now) > 0.0
                || self.motion.transcript_opacity.is_active(now);
            if self.expanded() || fading {
                pill = pill.child(self.transcript()).child(self.transcript_fade(
                    theme, &look, now, inner_h, radius,
                ));
            }
            pill = if !has_strip {
                // Keep the transcript's bottom inset while the strip is gone.
                pill.child(div().flex_none().h(px(STRIP_H)))
            } else if self.dictating() {
                let strip = self.dictation(theme, now, cx);
                pill.child(strip)
            } else {
                pill.child(
                    self.strip(closing, strip_opacity)
                        .gap(px(6.0))
                        .children(self.strip_items(theme)),
                )
            };
        } else if self.dictating() {
            pill = pill.px(px(5.0)).items_center().child(self.dictation(theme, now, cx));
        } else {
            pill = pill
                .items_center()
                .gap(px(6.0))
                .pl(px(8.0))
                .pr(px(10.0))
                .children(self.strip_items(theme));
        }
        if let Some(spinner) = self.spinner(theme) {
            pill = pill.child(spinner);
        }

        let notice = self
            .notice
            .as_ref()
            .filter(|_| self.headroom)
            .map(|text| self.notice(text));
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_end()
            .items_center()
            .pb(px(BOTTOM_SLACK))
            .font_family("Inter")
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(height))
                    .flex()
                    .justify_center()
                    .children(notice)
                    .child(pill),
            )
    }
}
