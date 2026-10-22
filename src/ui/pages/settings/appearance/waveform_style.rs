//! The waveform style: a pill preview and a swatch per style. The frame driver
//! is a canvas in this view, so frames stop when the tab isn't rendered.

use std::time::{Duration, Instant};

use gpui_kit::{canvas, div, prelude::*, px, Context, Entity, Subscription, Task, Window};

use super::card::{capsule, choice_grid, full, group_label, mic, section_card, stage, Choice};
use super::levels::SpeechLevels;
use crate::ui::icons::AppIcon;
use crate::ui::pill::theme::{def, PillThemeDef};
use crate::ui::pill::waveform::Waveform;
use crate::ui::settings::{UiSettings, WaveformStyle};
use crate::ui::theme::{Palette, R_XS};

pub(super) const STYLES: [(WaveformStyle, &str, &str); 4] = [
    (WaveformStyle::Bars, "Bars", "live level meter"),
    (WaveformStyle::Memo, "Memo", "scrolling voice note"),
    (WaveformStyle::Eq, "EQ", "retro equalizer"),
    (WaveformStyle::Spectrum, "Spectrum", "analyser columns"),
];

/// 30 Hz: every step re-renders the views above the previews.
const FRAME: Duration = Duration::from_millis(33);

pub struct WaveformSection {
    levels: SpeechLevels,
    last_frame: Option<Instant>,
    preview: Entity<Waveform>,
    swatches: [Entity<Waveform>; 4],
    next_frame: Option<Task<()>>,
    _settings_changed: Subscription,
}

impl WaveformSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let settings = UiSettings::global(cx);
        let accent = def(settings.pill_theme).accent;
        let style = settings.waveform_style;
        let waveform =
            |cx: &mut Context<Self>, style| cx.new(|_| Waveform::new(style, accent, true).paced());
        Self {
            levels: SpeechLevels::new(),
            last_frame: None,
            preview: waveform(cx, style),
            swatches: STYLES.map(|(style, ..)| waveform(cx, style)),
            next_frame: None,
            _settings_changed: cx.observe_global::<UiSettings>(Self::settings_changed),
        }
    }

    fn settings_changed(&mut self, cx: &mut Context<Self>) {
        let settings = UiSettings::global(cx);
        let (accent, style) = (def(settings.pill_theme).accent, settings.waveform_style);
        for waveform in self.swatches.iter().chain([&self.preview]) {
            waveform.update(cx, |w, cx| w.set_accent(accent, cx));
        }
        self.preview.update(cx, |w, cx| w.set_style(style, cx));
        cx.notify();
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let dt = self
            .last_frame
            .replace(now)
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        let levels = self.levels.step(dt);
        for waveform in self.swatches.iter().chain([&self.preview]) {
            waveform.update(cx, |w, cx| w.set_levels(levels, cx));
        }
    }

    /// Step the levels one frame from now, unless a step is already pending.
    fn schedule(&mut self, cx: &mut Context<Self>) {
        if self.next_frame.is_some() {
            return;
        }
        // Timed from the last step, so render time does not stretch the period.
        let wait = FRAME.saturating_sub(self.last_frame.map_or(FRAME, |last| last.elapsed()));
        self.next_frame = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| {
                this.next_frame = None;
                this.advance(cx);
            });
        }));
    }

    /// An empty canvas whose paint schedules the next step.
    fn driver(cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        canvas(
            |_, _, _| (),
            move |_, (), _, cx| this.update(cx, Self::schedule),
        )
        .absolute()
        .size_0()
    }

    fn swatch(
        &self,
        index: usize,
        current: WaveformStyle,
        t: &PillThemeDef,
        p: &Palette,
    ) -> impl IntoElement {
        let (style, title, hint) = STYLES[index];
        // `bars` draws in a 42x20 slot like the pill's; the canvas styles in 44x16.
        let (w, h) = if style == WaveformStyle::Bars {
            (42., 20.)
        } else {
            (44., 16.)
        };
        let lead = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .w(px(52.))
            .h(px(24.))
            .rounded(R_XS)
            .border_1()
            .border_color(p.border_soft)
            .bg(t.bg)
            .child(
                div()
                    .size(px(0.))
                    .w(px(w))
                    .h(px(h))
                    .child(self.swatches[index].clone().cached(full())),
            )
            .into_any_element();
        Choice {
            id: ("waveform-style", index as u64).into(),
            active: style == current,
            lead,
            title,
            hint: Some(hint),
        }
        .render(p, move |_, cx| {
            UiSettings::update(cx, |s| s.waveform_style = style);
        })
    }

    fn preview_pill(&self, t: &PillThemeDef) -> impl IntoElement {
        capsule(t, 32.)
            .w(px(80.))
            .overflow_hidden()
            .gap(px(6.))
            .pl(px(8.))
            .pr(px(10.))
            .child(mic(t, 14.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(20.))
                    .child(self.preview.clone().cached(full())),
            )
    }
}

impl Render for WaveformSection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let settings = UiSettings::global(cx);
        let (current, t) = (settings.waveform_style, def(settings.pill_theme));
        let (_, name, hint) = STYLES
            .iter()
            .copied()
            .find(|&(style, ..)| style == current)
            .unwrap_or(STYLES[0]);
        let choices = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(group_label(AppIcon::AudioLines, "WAVEFORM", &p))
            .child(
                choice_grid().children((0..STYLES.len()).map(|i| self.swatch(i, current, t, &p))),
            );
        div().child(Self::driver(cx)).child(section_card(
            stage(t, &p, self.preview_pill(t)),
            name,
            hint,
            choices,
            &p,
        ))
    }
}
