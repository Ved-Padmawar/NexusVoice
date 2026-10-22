//! A centred dialog over the dimmed window, anchored at the window origin so
//! one opened from inside a page still covers everything.

use std::time::Duration;

use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, Animation, AnimationExt as _, AnyElement, App,
    IntoElement, Pixels, SharedString, Window,
};

use crate::ui::theme::{alpha, Palette, R_XL};

/// A backdrop click calls `on_dismiss`; without it the modal closes from inside.
pub fn modal(
    id: impl Into<SharedString>,
    width: Pixels,
    content: impl IntoElement,
    on_dismiss: Option<super::Action>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let viewport = window.viewport_size();
    let p = Palette::global(cx);
    let id: SharedString = id.into();
    let panel: AnyElement = div()
        .id("modal-panel")
        .occlude()
        .w(width)
        .max_h(gpui_kit::relative(0.9))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(R_XL)
        .bg(p.panel)
        .border_1()
        .border_color(p.border)
        .shadow(p.shadow_lg())
        .child(content)
        .into_any_element();

    let backdrop = div()
        .id(id.clone())
        .occlude()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(alpha(gpui_kit::black(), 0.55))
        .when_some(on_dismiss, |el, dismiss| {
            el.on_mouse_down(gpui_kit::MouseButton::Left, move |_, window, cx| {
                dismiss(window, cx);
            })
        })
        .child(panel)
        .with_animation(
            gpui_kit::ElementId::Name(id),
            Animation::new(Duration::from_millis(160)).with_easing(gpui_kit::ease_out_quint()),
            Styled::opacity,
        );
    deferred(
        anchored()
            .position(point(px(0.), px(0.)))
            .child(div().w(viewport.width).h(viewport.height).child(backdrop)),
    )
    .with_priority(2)
}
