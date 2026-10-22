//! Loading and error states: the spinner, skeleton blocks, and the section
//! wrapper that picks between skeleton, error and content.

use std::time::Duration;

use gpui_kit::{
    canvas, div, point, prelude::*, px, Animation, AnimationExt as _, App, Bounds, Div, ElementId,
    Hsla, IntoElement, PathBuilder, Pixels, SharedString, Window,
};

use super::icon;
use crate::ui::icons::AppIcon;
use crate::ui::theme::{alpha, Palette, R_MD};

/// A `size` ring in `track` with a quarter arc in `arc`, turning once every
/// `period`. Drawn as paths, so it stays crisp at any size.
pub fn spinner(
    id: impl Into<SharedString>,
    size: f32,
    stroke: f32,
    track: Hsla,
    arc: Hsla,
    period: Duration,
) -> impl IntoElement {
    div().size(px(size)).with_animation(
        ElementId::Name(id.into()),
        Animation::new(period).repeat(),
        move |el, turn| {
            el.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| paint_ring(bounds, size, stroke, turn, track, arc, window),
                )
                .size_full(),
            )
        },
    )
}

fn paint_ring(
    bounds: Bounds<Pixels>,
    size: f32,
    stroke: f32,
    turn: f32,
    track: Hsla,
    arc: Hsla,
    window: &mut Window,
) {
    let r = (size - stroke) / 2.;
    let c = bounds.center();
    let at = |deg: f32| {
        let rad = (deg + turn * 360.).to_radians();
        point(c.x + px(r * rad.cos()), c.y + px(r * rad.sin()))
    };
    let radii = point(px(r), px(r));
    let mut ring = PathBuilder::stroke(px(stroke));
    ring.move_to(at(0.));
    ring.arc_to(radii, px(0.), false, true, at(180.));
    ring.arc_to(radii, px(0.), false, true, at(360.));
    if let Ok(path) = ring.build() {
        window.paint_path(path, track);
    }
    let mut quarter = PathBuilder::stroke(px(stroke));
    quarter.move_to(at(-90.));
    quarter.arc_to(radii, px(0.), false, true, at(0.));
    if let Ok(path) = quarter.build() {
        window.paint_path(path, arc);
    }
}

/// A pulsing placeholder block, as Tailwind's `animate-pulse` draws it.
/// `shape` sizes and rounds it.
pub fn skeleton(
    id: impl Into<SharedString>,
    shape: impl FnOnce(Div) -> Div,
    cx: &App,
) -> impl IntoElement {
    let surface = Palette::global(cx).surface;
    shape(div().bg(surface).rounded(px(4.))).with_animation(
        ElementId::Name(id.into()),
        Animation::new(Duration::from_secs(2))
            .repeat()
            .with_easing(gpui_kit::pulsating_between(0.5, 1.)),
        |el, opacity| el.opacity(opacity),
    )
}

/// Where a section's data is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    Loading,
    Failed(SharedString),
    Loaded,
}

/// Picks what a section shows: content once loaded (or already on screen),
/// a skeleton while loading, or the error with a retry.
#[derive(IntoElement)]
pub struct SectionState {
    state: LoadState,
    /// Content is already on screen: a reload updates it in place.
    has_data: bool,
    skeleton: gpui_kit::AnyElement,
    content: gpui_kit::AnyElement,
    on_retry: Box<dyn Fn(&mut Window, &mut App)>,
}

impl SectionState {
    pub fn new(
        state: LoadState,
        has_data: bool,
        skeleton: impl IntoElement,
        content: impl IntoElement,
        on_retry: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            state,
            has_data,
            skeleton: skeleton.into_any_element(),
            content: content.into_any_element(),
            on_retry: Box::new(on_retry),
        }
    }
}

impl RenderOnce for SectionState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.has_data {
            return self.content;
        }
        match self.state {
            LoadState::Loaded => self.content,
            LoadState::Loading => self.skeleton,
            LoadState::Failed(error) => {
                let p = Palette::global(cx);
                let on_retry = self.on_retry;
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .py_12()
                    .px_6()
                    .child(
                        div()
                            .size(px(44.))
                            .rounded_full()
                            .bg(alpha(p.danger, 0.12))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(AppIcon::AlertCircle, 20., p.danger)),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(p.fg_2)
                            .child("Couldn’t load this section"),
                    )
                    .child(
                        div()
                            .max_w(px(288.))
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .text_center()
                            .child(error),
                    )
                    .child(
                        div()
                            .id("section-retry")
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h_8()
                            .px(px(14.))
                            .rounded(R_MD)
                            .border_1()
                            .border_color(p.border)
                            .bg(p.surface)
                            .text_color(p.fg)
                            .text_size(px(12.))
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .cursor_pointer()
                            .hover(|s| s.border_color(p.accent))
                            .on_click(move |_, window, cx| on_retry(window, cx))
                            .child(icon(AppIcon::RefreshCw, 12., p.fg))
                            .child("Try again"),
                    )
                    .into_any_element()
            }
        }
    }
}
