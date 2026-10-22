//! The sidebar: brand, navigation, and the update prompt at its foot.

use std::time::Duration;

use gpui_kit::{
    div, img, prelude::*, px, Animation, AnimationExt as _, App, FontWeight, IntoElement,
    SharedString, Window,
};

use crate::platform::paths;
use crate::ui::components::icon;
use crate::ui::icons::AppIcon;
use crate::ui::settings::{Route, UiSettings};
use crate::ui::store::{Store, UpdateStatus};
use crate::ui::theme::{alpha, mix, Palette, R_MD, R_SM, R_XS, SIDEBAR_W};

const NAV: [(Route, &str, AppIcon); 3] = [
    (Route::Dashboard, "Dashboard", AppIcon::LayoutDashboard),
    (Route::Dictionary, "Dictionary", AppIcon::BookOpen),
    (Route::Settings, "Settings", AppIcon::Settings2),
];

#[derive(IntoElement)]
pub struct Sidebar {
    pub route: Route,
}

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = *Palette::global(cx);
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(SIDEBAR_W)
            .h_full()
            .bg(p.panel)
            .border_r_1()
            .border_color(p.border)
            .child(brand(cx))
            .child(
                div()
                    .id("nav")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(1.))
                    .p_2()
                    .overflow_y_scroll()
                    .children(NAV.into_iter().map(|(route, label, glyph)| {
                        nav_item(route, label, glyph, route == self.route, cx)
                    })),
            )
            .child(update_prompt(cx))
    }
}

fn brand(cx: &App) -> impl IntoElement {
    let p = Palette::global(cx);
    div()
        .id("brand")
        .flex()
        .items_center()
        .px(px(14.))
        .pt(px(28.))
        .pb(px(14.))
        .border_b_1()
        .border_color(p.border_soft)
        .cursor_pointer()
        .on_click(|_, _, cx| UiSettings::update(cx, |s| s.route = Route::Dashboard))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(9.))
                .child(img("logo.png").size(px(36.)).rounded(R_MD).flex_shrink_0())
                .child(
                    div()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .flex()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::BLACK)
                                        .line_height(px(13.))
                                        .child(div().text_color(p.fg).child("Nexus"))
                                        .child(div().text_color(p.accent).child("Voice")),
                                )
                                .when(paths::IS_DEV, |el| {
                                    el.child(
                                        div()
                                            .px(px(5.))
                                            .py(px(1.))
                                            .rounded(R_XS)
                                            .bg(p.accent_soft)
                                            .text_color(p.accent)
                                            .text_size(px(9.))
                                            .font_weight(FontWeight::BOLD)
                                            .line_height(px(9.))
                                            .child("DEV"),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .mt(px(2.))
                                .text_size(px(10.))
                                .text_color(p.fg_2)
                                .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                        ),
                ),
        )
}

fn nav_item(route: Route, label: &'static str, glyph: AppIcon, active: bool, cx: &App) -> impl IntoElement {
    let p = Palette::global(cx);
    div()
        .id(label)
        .group(label)
        .relative()
        .flex()
        .items_center()
        .gap(px(9.))
        .px(px(10.))
        .py(px(7.))
        .rounded(R_MD)
        .text_size(px(13.))
        .cursor_pointer()
        .map(|el| {
            if active {
                el.text_color(p.fg).bg(p.surface).font_weight(FontWeight::SEMIBOLD)
            } else {
                el.text_color(p.muted)
                    .font_weight(FontWeight::MEDIUM)
                    .hover(|s| s.text_color(p.fg).bg(p.surface))
            }
        })
        .on_click(move |_, _, cx| UiSettings::update(cx, |s| s.route = route))
        .when(active, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(8.))
                    .h(px(16.))
                    .w(px(3.))
                    .rounded_r(px(3.))
                    .bg(p.accent),
            )
        })
        .child(
            div()
                .flex_shrink_0()
                .when(!active, |el| {
                    el.opacity(0.65).group_hover(label, |s| s.opacity(1.))
                })
                .child(icon(glyph, 16., if active { p.fg } else { p.muted })),
        )
        .child(label)
}

/// Installs in place — the About tab is the secondary path, not the only one.
fn update_prompt(cx: &App) -> impl IntoElement {
    let p = *Palette::global(cx);
    let update = &Store::entity(cx).read(cx).update;
    let shown = !update.dismissed
        && matches!(
            update.status,
            UpdateStatus::Available | UpdateStatus::Downloading | UpdateStatus::Ready
        );
    if !shown {
        return div().into_any_element();
    }
    let status = update.status;
    let version: SharedString = update.version.clone().unwrap_or_default();
    let progress = update.progress;

    let title = match status {
        UpdateStatus::Ready => "Update installed",
        UpdateStatus::Downloading => "Downloading",
        _ => "Update available",
    };
    let accent_version = div()
        .text_color(p.accent)
        .font_weight(FontWeight::SEMIBOLD)
        .child(format!("v{version}"));
    let detail = div()
        .flex()
        .items_center()
        .gap_1()
        .text_size(px(10.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.muted)
        .whitespace_nowrap()
        .overflow_hidden()
        .map(|el| match status {
            UpdateStatus::Available => el
                .child(concat!("v", env!("CARGO_PKG_VERSION")))
                .child(div().opacity(0.5).child("→"))
                .child(accent_version),
            UpdateStatus::Ready => el.child(accent_version).child("· restart to finish"),
            _ => el.child(accent_version),
        });

    let body = if status == UpdateStatus::Downloading {
        div()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(
                div()
                    .h(px(3.))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(alpha(p.fg, 0.12))
                    .child(
                        div()
                            .h_full()
                            .rounded_full()
                            .bg(p.accent)
                            .w(gpui_kit::relative(f32::from(progress) / 100.)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .text_size(px(10.))
                    .child(div().text_color(p.muted).child("Downloading…"))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.fg_2)
                            .child(format!("{progress}%")),
                    ),
            )
            .into_any_element()
    } else {
        let ready = status == UpdateStatus::Ready;
        div()
            .id("update-action")
            .w_full()
            .h(px(25.))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(5.))
            .rounded(R_SM)
            .text_size(px(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .map(|el| {
                if ready {
                    el.text_color(p.accent)
                        .border_1()
                        .border_color(alpha(p.accent, 0.4))
                        .hover(|s| s.bg(alpha(p.accent, 0.12)))
                } else {
                    el.bg(p.accent)
                        .text_color(p.accent_fg)
                        .hover(|s| s.bg(mix(p.accent, gpui_kit::white(), 0.85)))
                }
            })
            .on_click(move |_, _, cx| {
                Store::entity(cx).update(cx, |store, cx| {
                    if ready {
                        store.restart(cx);
                    } else {
                        store.install_update(cx);
                    }
                });
            })
            .child(icon(
                if ready { AppIcon::RotateCcw } else { AppIcon::Download },
                10.,
                if ready { p.accent } else { p.accent_fg },
            ))
            .child(if ready { "Restart now" } else { "Install update" })
            .into_any_element()
    };

    div()
        .px_2()
        .pb_2()
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .gap(px(9.))
                .px(px(11.))
                .py(px(10.))
                .rounded(R_MD)
                .bg(p.surface)
                .border_1()
                .border_color(mix(p.accent, p.border_soft, 0.22))
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .pr(px(18.))
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .justify_center()
                                .size(px(22.))
                                .rounded(R_SM)
                                .bg(alpha(p.accent, 0.15))
                                .child(icon(AppIcon::ArrowUp, 11., p.accent)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(1.))
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.fg)
                                        .truncate()
                                        .child(title),
                                )
                                .child(detail),
                        ),
                )
                .when(status != UpdateStatus::Downloading, |el| {
                    el.child(
                        div()
                            .id("update-dismiss")
                            .absolute()
                            .top(px(5.))
                            .right(px(5.))
                            .size_4()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(R_SM)
                            .cursor_pointer()
                            .hover(|s| s.bg(p.danger_soft))
                            .on_click(|_, _, cx| {
                                Store::entity(cx).update(cx, |store, cx| store.dismiss_update(cx));
                            })
                            .child(icon(AppIcon::X, 10., p.muted)),
                    )
                })
                .child(body),
        )
        .with_animation(
            "update-prompt",
            Animation::new(Duration::from_millis(200)),
            |el, t| el.opacity(t),
        )
        .into_any_element()
}
