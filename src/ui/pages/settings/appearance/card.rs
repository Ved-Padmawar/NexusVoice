//! Pieces the three Appearance cards share.

use gpui_kit::{
    div, prelude::*, px, rgb, svg, AnyElement, ElementId, FontWeight, Hsla, StyleRefinement, Window,
};

use crate::ui::components::icon;
use crate::ui::icons::AppIcon;
use crate::ui::pill::theme::{shadows, PillThemeDef};
use crate::ui::theme::{mix, Palette, R_LG, R_MD, R_XS};
use gpui_kit::{App, Div, Rgba, Stateful};

const MIC_ICON: &str = "icons/pill/mic.svg";

pub fn section_card(
    preview: impl IntoElement,
    name: &'static str,
    hint: &'static str,
    choices: impl IntoElement,
    p: &Palette,
) -> Div {
    div()
        .flex()
        .gap(px(16.))
        .rounded(R_LG)
        .border_1()
        .border_color(p.border_soft)
        .bg(p.panel)
        .p(px(16.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .gap(px(10.))
                .child(preview)
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.fg)
                                .child(name),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(p.muted)
                                .child(hint),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .child(choices),
        )
}

pub fn preview_frame(p: &Palette) -> Div {
    div()
        .overflow_hidden()
        .rounded(R_MD)
        .border_1()
        .border_color(p.border_soft)
        .shadow(p.shadow_sm())
}

/// `text` is given in capitals.
pub fn group_label(name: AppIcon, text: &'static str, p: &Palette) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.muted)
        .child(icon(name, 11., p.muted))
        .child(text)
}

pub fn choice_grid() -> Div {
    div().grid().grid_cols(2).gap(px(6.))
}

/// One choice: swatch, title, optional hint, and a check when active.
pub struct Choice {
    pub id: ElementId,
    pub active: bool,
    pub lead: AnyElement,
    pub title: &'static str,
    pub hint: Option<&'static str>,
}

impl Choice {
    pub fn render(
        self,
        p: &Palette,
        on_pick: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let active = self.active;
        let title = div()
            .truncate()
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if active { p.accent } else { p.fg })
            .child(self.title);
        let text = div()
            .min_w_0()
            .flex_1()
            .child(title)
            .children(self.hint.map(|hint| {
                div()
                    .truncate()
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .child(hint)
            }));
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(10.))
            .min_w_0()
            .px(px(10.))
            .py(px(8.))
            .rounded(R_MD)
            .border_1()
            .cursor_pointer()
            .when(active, |el| el.border_color(p.accent).bg(p.accent_soft))
            .when(!active, |el| {
                el.border_color(p.border_soft)
                    .bg(p.surface)
                    .hover(|s| s.border_color(p.border).bg(p.surface_hover))
            })
            .on_click(move |_, window, cx| on_pick(window, cx))
            .child(self.lead)
            .child(text)
            .when(active, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(16.))
                        .rounded_full()
                        .bg(p.accent)
                        .child(icon(AppIcon::Check, 9., p.accent_fg)),
                )
            })
    }
}

pub fn strip(colors: impl IntoIterator<Item = Hsla>, cell_w: f32, p: &Palette) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .overflow_hidden()
        .rounded(R_XS)
        .border_1()
        .border_color(p.border_soft)
        .children(
            colors
                .into_iter()
                .map(|color| div().w(px(cell_w)).h(px(24.)).bg(color)),
        )
        .into_any_element()
}

pub fn over(color: Rgba, ground: Rgba) -> Hsla {
    mix(Hsla::from(Rgba { a: 1., ..color }), ground.into(), color.a)
}

/// The backdrop a pill preview sits on: the pill's ground mixed down.
pub fn stage(def: &PillThemeDef, p: &Palette, pill: impl IntoElement) -> Div {
    let ground = if def.bg == rgb(0xff_ff_ff) {
        rgb(0xe8_ea_f0).into()
    } else {
        mix(def.bg.into(), rgb(0x11_11_11).into(), 0.6)
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .w(px(220.))
        .h(px(86.))
        .rounded(R_MD)
        .border_1()
        .border_color(p.border_soft)
        .shadow(p.shadow_sm())
        .bg(ground)
        .child(pill)
}

pub fn mic(def: &PillThemeDef, size: f32) -> impl IntoElement {
    svg()
        .path(MIC_ICON)
        .size(px(size))
        .flex_none()
        .text_color(def.accent)
}

pub fn capsule(def: &PillThemeDef, height: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(height))
        .rounded_full()
        .bg(def.bg)
        .border_1()
        .border_color(def.border)
        .shadow(shadows(def, 1.))
}

pub fn full() -> StyleRefinement {
    StyleRefinement::default().size_full()
}
