//! The panel menus and popovers open in.

use std::time::Duration;

use gpui_kit::{
    div, prelude::*, px, Animation, AnimationExt as _, App, BoxShadow, Div, ElementId, Pixels,
    SharedString, Stateful,
};

use crate::ui::theme::{Palette, R_LG};

pub fn popover_panel(id: impl Into<ElementId>, shadow: Vec<BoxShadow>, cx: &App) -> Stateful<Div> {
    let p = Palette::global(cx);
    div()
        .id(id)
        .occlude()
        .overflow_hidden()
        .rounded(R_LG)
        .bg(p.panel)
        .border_1()
        .border_color(p.border)
        .shadow(shadow)
}

/// Fade in while sliding down 4px to `gap` below the anchor.
pub fn popover_open(
    panel: Stateful<Div>,
    id: impl Into<SharedString>,
    gap: Pixels,
) -> impl IntoElement {
    panel.with_animation(
        ElementId::Name(id.into()),
        Animation::new(Duration::from_millis(140)).with_easing(gpui_kit::ease_out_quint()),
        move |el, t| el.opacity(t).mt(gap - px(4. * (1. - t))),
    )
}
