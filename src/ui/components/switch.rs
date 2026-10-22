//! The on/off switch.

use std::time::Duration;

use gpui_kit::{div, prelude::*, px, Animation, AnimationExt as _, App, ElementId, Window};

use crate::ui::theme::{mix, Palette};

const TRACK_W: f32 = 42.;
const KNOB: f32 = 18.;
const KNOB_INSET: f32 = 3.;

/// A 42x24 switch. `animate` stays false until the first flip, so a switch
/// that starts on doesn't slide in.
pub fn switch(
    id: &'static str,
    on: bool,
    animate: bool,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = Palette::global(cx);
    let (off_color, on_color) = (p.border, p.accent);
    let travel = TRACK_W - KNOB - 2. * KNOB_INSET;
    let at = move |progress: f32| {
        let progress = if on { progress } else { 1. - progress };
        (
            mix(on_color, off_color, progress),
            KNOB_INSET + travel * progress,
        )
    };
    let shadow = p.shadow_sm();
    let track = div()
        .id(id)
        .relative()
        .flex_shrink_0()
        .w(px(TRACK_W))
        .h(px(24.))
        .rounded_full()
        .cursor_pointer()
        .on_click(move |_, window, cx| on_toggle(window, cx));
    if !animate {
        let (color, x) = at(1.);
        return track.bg(color).child(knob(x, shadow)).into_any_element();
    }
    track
        .with_animation(
            ElementId::NamedInteger(id.into(), u64::from(on)),
            Animation::new(Duration::from_millis(200)).with_easing(gpui_kit::ease_out_quint()),
            move |track, t| {
                let (color, x) = at(t);
                track.bg(color).child(knob(x, shadow.clone()))
            },
        )
        .into_any_element()
}

fn knob(x: f32, shadow: Vec<gpui_kit::BoxShadow>) -> impl IntoElement {
    div()
        .absolute()
        .top(px(KNOB_INSET))
        .left(px(x))
        .size(px(KNOB))
        .rounded_full()
        .bg(gpui_kit::white())
        .shadow(shadow)
}
