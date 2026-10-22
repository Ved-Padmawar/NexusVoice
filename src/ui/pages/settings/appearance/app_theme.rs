//! The app theme: a live miniature of the window and a swatch per theme.

use gpui_kit::{div, prelude::*, px, relative, Context, Div, Hsla, Subscription, Window};

use super::card::{choice_grid, group_label, preview_frame, section_card, strip, Choice};
use crate::ui::icons::AppIcon;
use crate::ui::settings::{ThemeName, UiSettings};
use crate::ui::theme::{alpha, Palette, R_MD};

pub(super) const THEMES: [(ThemeName, &str, &str); 8] = [
    (ThemeName::Abyss, "Abyss", "cool slate, periwinkle accent"),
    (ThemeName::Midnight, "Midnight", "slate indigo, cyan accent"),
    (ThemeName::Steel, "Steel", "neutral grey, cool highlight"),
    (ThemeName::Pine, "Pine", "forest green, mint accent"),
    (ThemeName::Canvas, "Canvas", "clean white, cobalt accent"),
    (ThemeName::Dawn, "Dawn", "warm paper, amber accent"),
    (ThemeName::Breeze, "Breeze", "pale cyan, teal accent"),
    (ThemeName::Blossom, "Blossom", "soft rose, crimson accent"),
];

pub struct ThemeSection {
    palettes: [Palette; 8],
    _settings_changed: Subscription,
}

impl ThemeSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            palettes: ThemeName::ALL.map(Palette::of),
            _settings_changed: cx.observe_global::<UiSettings>(|_, cx| cx.notify()),
        }
    }

    fn group(&self, dark: bool, current: ThemeName, p: &Palette) -> impl IntoElement {
        let (icon, label) = if dark {
            (AppIcon::Moon, "DARK")
        } else {
            (AppIcon::Sun, "LIGHT")
        };
        let choices = THEMES
            .iter()
            .zip(&self.palettes)
            .filter(|(_, palette)| palette.dark == dark)
            .map(|(&(name, title, _), palette)| {
                let lead = strip(
                    [
                        palette.bg,
                        palette.panel,
                        palette.surface,
                        palette.border,
                        palette.muted,
                        palette.accent,
                    ],
                    9.,
                    p,
                );
                Choice {
                    id: ("theme", name as u64).into(),
                    active: name == current,
                    lead,
                    title,
                    hint: None,
                }
                .render(p, move |_, cx| UiSettings::update(cx, |s| s.theme = name))
            });
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(group_label(icon, label, p))
            .child(choice_grid().children(choices))
    }
}

impl Render for ThemeSection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let current = UiSettings::global(cx).theme;
        let index = ThemeName::ALL
            .iter()
            .position(|&n| n == current)
            .unwrap_or(0);
        let (_, name, blurb) = THEMES[index];
        // The preview holds its place and only recolours, reading as before and after.
        let preview = preview_frame(&p).child(theme_preview(&self.palettes[index]));
        let choices = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.group(true, current, &p))
            .child(self.group(false, current, &p));
        section_card(preview, name, blurb, choices, &p)
    }
}

fn shape(w: f32, h: f32, radius: f32, color: Hsla, opacity: f32) -> Div {
    div()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .rounded(px(radius))
        .bg(alpha(color, opacity))
}

fn theme_preview(t: &Palette) -> Div {
    div()
        .relative()
        .w(px(280.))
        .h(px(168.))
        .overflow_hidden()
        .rounded(R_MD)
        .bg(t.bg)
        .child(rail(t))
        .child(content(t))
}

fn rail(t: &Palette) -> Div {
    let nav_item = |active: bool| {
        let (mark, label) = if active {
            (
                shape(5., 5., 1., t.accent, 1.),
                shape(22., 3., 99., t.accent, 1.),
            )
        } else {
            (
                shape(5., 5., 1., t.muted, 0.45),
                shape(18., 3., 99., t.muted, 0.35),
            )
        };
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(6.))
            .py(px(4.))
            .rounded(px(3.))
            .when(active, |el| el.bg(alpha(t.accent, 0.133)))
            .child(mark)
            .child(label)
    };
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .w(px(52.))
        .bg(t.panel)
        .border_r_1()
        .border_color(t.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .pt(px(10.))
                .child(shape(7., 7., 2., t.accent, 1.))
                .child(shape(24., 3., 99., t.muted, 0.7)),
        )
        .child(
            div()
                .mt(px(14.))
                .flex()
                .flex_col()
                .gap(px(5.))
                .px(px(6.))
                .child(nav_item(true))
                .child(nav_item(false))
                .child(nav_item(false)),
        )
        .child(
            div()
                .absolute()
                .left(px(6.))
                .right(px(6.))
                .bottom(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(6.))
                .py(px(4.))
                .rounded(px(3.))
                .bg(t.surface)
                .child(shape(8., 8., 99., t.accent, 0.6))
                .child(shape(20., 3., 99., t.muted, 0.4)),
        )
}

fn content(t: &Palette) -> Div {
    let faint = alpha(t.muted, 0.6);
    let window_controls = div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(6.))
        .h(px(14.))
        .pr(px(8.))
        .child(shape(6., 1., 0., t.muted, 0.6))
        .child(div().size(px(4.)).border_1().border_color(faint))
        .child(
            div()
                .size(px(4.))
                .border_l_1()
                .border_t_1()
                .border_color(faint),
        );
    let title = div()
        .flex()
        .items_center()
        .gap_2()
        .px(px(10.))
        .pb_2()
        .child(
            div()
                .child(shape(40., 4., 99., t.fg, 0.9))
                .child(shape(56., 3., 99., t.muted, 0.4).mt(px(4.))),
        )
        .child(
            div()
                .ml_auto()
                .w(px(44.))
                .h(px(10.))
                .rounded(px(3.))
                .bg(t.surface)
                .border_1()
                .border_color(t.border),
        );
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(52.))
        .right_0()
        .child(window_controls)
        .child(title)
        .child(body(t))
}

fn body(t: &Palette) -> Div {
    let line = |width: f32| {
        div()
            .rounded(px(3.))
            .bg(t.panel)
            .px(px(6.))
            .py(px(5.))
            .child(
                div()
                    .h(px(3.))
                    .w(relative(width))
                    .rounded_full()
                    .bg(alpha(t.fg, 0.65)),
            )
            .child(
                div()
                    .mt(px(4.))
                    .h(px(3.))
                    .w(relative(0.56))
                    .rounded_full()
                    .bg(alpha(t.muted, 0.3)),
            )
    };
    let stat = div()
        .rounded(px(3.))
        .bg(t.panel)
        .p(px(6.))
        .child(shape(28., 6., 99., t.fg, 0.85))
        .child(shape(0., 8., 2., t.accent, 0.3).mt(px(5.)).w_full());
    let note = div()
        .rounded(px(3.))
        .p(px(6.))
        .bg(alpha(t.accent, 0.122))
        .border_1()
        .border_color(alpha(t.accent, 0.267))
        .child(shape(24., 3., 99., t.accent, 1.))
        .child(shape(32., 3., 99., t.accent, 0.5).mt(px(4.)));
    div()
        .flex()
        .gap(px(6.))
        .px(px(10.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(6.))
                .child(line(1.0))
                .child(line(0.84))
                .child(line(0.94)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(px(60.))
                .gap(px(6.))
                .child(stat)
                .child(note),
        )
}
