//! When an idle model is unloaded to free memory.

use gpui_kit::component::{notification::Notification, WindowExt as _};
use gpui_kit::{div, prelude::*, px, Context, FontWeight, Window};

use crate::api;
use crate::inference::idle::ModelUnload;
use crate::ui::components::{icon, pill};
use crate::ui::icons::AppIcon;
use crate::ui::theme::{Palette, R_MD};

const OPTIONS: [(ModelUnload, &str); 6] = [
    (ModelUnload::Never, "Never"),
    (ModelUnload::After2Minutes, "2 min"),
    (ModelUnload::After5Minutes, "5 min"),
    (ModelUnload::After10Minutes, "10 min"),
    (ModelUnload::After15Minutes, "15 min"),
    (ModelUnload::After1Hour, "1 hour"),
];

pub struct ModelUnloadSection {
    policy: ModelUnload,
}

impl ModelUnloadSection {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            policy: api::get_model_unload(),
        }
    }

    /// A failed save restores the previous choice.
    fn choose(&mut self, next: ModelUnload, window: &mut Window, cx: &mut Context<Self>) {
        if next == self.policy {
            return;
        }
        let previous = std::mem::replace(&mut self.policy, next);
        if let Err(e) = api::set_model_unload(next) {
            self.policy = previous;
            let message = format!("Could not save the unload setting: {e}");
            window.push_notification(Notification::error(message), cx);
        }
        cx.notify();
    }
}

impl Render for ModelUnloadSection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(10.))
            .px(px(12.))
            .py(px(8.))
            .rounded(R_MD)
            .border_1()
            .border_color(p.border_soft)
            .bg(p.panel)
            .child(icon(AppIcon::Timer, 13., p.accent))
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.fg_2)
                    .child("Unload when idle"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .child("Frees memory between dictations; the next one waits for the model to reload."),
            )
            .child(div().flex().flex_shrink_0().items_center().gap(px(4.)).children(
                OPTIONS.into_iter().enumerate().map(|(i, (value, label))| {
                    pill(("unload", i), label, value == self.policy, false, cx).on_click(
                        cx.listener(move |this, _, window, cx| this.choose(value, window, cx)),
                    )
                }),
            ))
    }
}
