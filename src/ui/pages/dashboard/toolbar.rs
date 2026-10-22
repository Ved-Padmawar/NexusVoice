//! The activity toolbar's buttons and the panel each opens below it. The panel
//! is styled like the shared dropdown menu: panel fill, 1px edge, rounded,
//! fading in over 140ms.

use std::time::Duration;

use gpui_kit::{
    anchored, deferred, div, prelude::*, px, Anchor, Animation, AnimationExt as _, App, ElementId,
    IntoElement, Pixels, Window,
};

use gpui_kit::component::Icon;
use crate::ui::icons::AppIcon;
use crate::ui::theme::{alpha, Palette, R_LG, R_MD};

/// The 28px pill button: an icon and a label. `highlighted` is the filter's
/// "something is applied" look.
pub fn toolbar_button(
    id: &'static str,
    glyph: AppIcon,
    label: &'static str,
    highlighted: bool,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = Palette::global(cx);
    let (fg, hover_fg, border, bg) = if highlighted {
        (p.accent, p.accent, alpha(p.accent, 0.6), p.accent_soft)
    } else {
        (p.fg_2, p.fg, p.border, p.panel)
    };
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(5.))
        .h(px(28.))
        .px(px(10.))
        .rounded(R_MD)
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_size(px(11.))
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .text_color(fg)
        .cursor_pointer()
        .hover(move |s| s.text_color(hover_fg).border_color(alpha(p.accent, 0.6)))
        .child(Icon::new(glyph).size(px(11.)))
        .child(label)
}

/// A zero-size anchor at the trigger's bottom-right corner (so the trigger
/// must be `relative`), holding `content` right-aligned below it, `gap` away.
/// `on_dismiss` runs on a click outside the panel.
pub fn popover_below(
    id: &'static str,
    content: impl IntoElement,
    gap: Pixels,
    shadow_lg: bool,
    on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = Palette::global(cx);
    let shadow = if shadow_lg { p.shadow_lg() } else { p.shadow_md() };
    let panel = div()
        .id(id)
        .occlude()
        .overflow_hidden()
        .rounded(R_LG)
        .bg(p.panel)
        .border_1()
        .border_color(p.border)
        .shadow(shadow)
        .on_mouse_down_out(move |_, window, cx| on_dismiss(window, cx))
        .child(content)
        .with_animation(
            ElementId::Name(id.into()),
            Animation::new(Duration::from_millis(140)).with_easing(gpui_kit::ease_out_quint()),
            move |el, t| el.opacity(t).mt(gap - px(4. * (1. - t))),
        );

    div()
        .absolute()
        .right_0()
        .bottom_0()
        .size_0()
        .child(deferred(anchored().anchor(Anchor::TopRight).child(panel)).with_priority(1))
}
