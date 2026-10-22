//! Headings: the header every page opens with, and the section heading.

use gpui_kit::{div, prelude::*, px, App, FontWeight, IntoElement, SharedString, Window};

use super::icon;
use crate::ui::icons::AppIcon;
use crate::ui::theme::{Palette, R_LG};

#[derive(IntoElement)]
pub struct PageHeader {
    icon: AppIcon,
    title: SharedString,
    subtitle: SharedString,
}

pub fn page_header(
    icon: AppIcon,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> PageHeader {
    PageHeader {
        icon,
        title: title.into(),
        subtitle: subtitle.into(),
    }
}

impl RenderOnce for PageHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = Palette::global(cx);
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .pb(px(14.))
            .mb(px(14.))
            .border_b_1()
            .border_color(p.border_soft)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .size(px(32.))
                            .rounded(R_LG)
                            .bg(p.accent_soft)
                            .child(icon(self.icon, 16., p.accent)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .line_height(px(17.6))
                                    .text_color(p.fg)
                                    .child(self.title),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(self.subtitle),
                            ),
                    ),
            )
    }
}

pub fn section_heading(
    title: &'static str,
    description: &'static str,
    cx: &App,
) -> impl IntoElement {
    let p = Palette::global(cx);
    div()
        .child(
            div()
                .mb(px(4.))
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.fg_2)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(p.muted)
                .child(description),
        )
}
