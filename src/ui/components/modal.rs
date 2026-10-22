//! A centred dialog over a dimmed window. Its content is page-specific; the
//! frame is shared so every modal opens, sits and closes the same way.

use std::time::Duration;

use gpui_kit::{
    deferred, div, prelude::*, Animation, AnimationExt as _, AnyElement, App, IntoElement, Pixels,
    SharedString,
};

use crate::ui::theme::{alpha, Palette, R_XL};

/// `content` in a `width` panel. A click on the backdrop calls `on_dismiss`,
/// when given; without it the modal must be closed from inside.
pub fn modal(
    id: impl Into<SharedString>,
    width: Pixels,
    content: impl IntoElement,
    on_dismiss: Option<super::Action>,
    cx: &App,
) -> impl IntoElement {
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

    deferred(
        div()
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
            ),
    )
    .with_priority(2)
}
