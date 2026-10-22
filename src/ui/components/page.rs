//! The header every page opens with: an accent tile, the title and a subtitle.

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
            .gap_4()
            .pb(px(14.))
            .mb(px(14.))
            .border_b_1()
            .border_color(p.border_soft)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .size_8()
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
