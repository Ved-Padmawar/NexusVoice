//! Offers the CUDA pack once the first-run picker has closed, on NVIDIA GPUs
//! only. The dialog follows the download through to its restart.

use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, FontWeight, Window};

use crate::api;
use crate::inference::cuda::PackState;
use crate::ui::components::{icon, modal};
use crate::ui::icons::AppIcon;
use crate::ui::pages::models::{
    button, icon_button, progress_bar, Button, ButtonKind, ButtonSize, Tone,
};
use crate::ui::runtime::SpawnApi as _;
use crate::ui::store::Store;
use crate::ui::theme::{Palette, R_LG, R_SM};

pub struct CudaOffer {
    gpu_name: Option<String>,
}

impl CudaOffer {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&Store::entity(cx), |_, _, cx| cx.notify())
            .detach();
        cx.spawn_api(
            async { api::get_hardware_profile().gpu_name },
            |this: &mut Self, name, cx| {
                this.gpu_name = Some(name);
                cx.notify();
            },
        );
        Self { gpu_name: None }
    }
}

fn store_do(cx: &mut App, action: impl FnOnce(&mut Store, &mut Context<Store>)) {
    Store::entity(cx).update(cx, action);
}

fn badge(tone: Tone, glyph: AppIcon, p: &Palette) -> impl IntoElement {
    let (bg, fg) = tone.badge(p);
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(36.))
        .mb(px(14.))
        .rounded(R_LG)
        .bg(bg)
        .child(icon(glyph, 17., fg))
}

fn title(text: &str, p: &Palette) -> impl IntoElement {
    div()
        .text_size(px(15.))
        .font_weight(FontWeight::BOLD)
        .text_color(p.fg)
        .child(text.to_string())
}

fn text(body: String, p: &Palette) -> impl IntoElement {
    div()
        .mt(px(6.))
        .text_size(px(12.))
        .line_height(px(18.))
        .text_color(p.muted)
        .child(body)
}

/// Two buttons of equal width.
fn actions(left: impl IntoElement, right: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .gap(px(8.))
        .mt(px(18.))
        .child(left)
        .child(right)
}

fn secondary(id: &'static str, label: &'static str) -> Button {
    button(id, label)
        .kind(ButtonKind::Outline)
        .size(ButtonSize::Sm)
        .flex_1()
}

fn primary(id: &'static str, label: &'static str, glyph: AppIcon) -> Button {
    button(id, label).size(ButtonSize::Sm).icon(glyph).flex_1()
}

fn close_button(cx: &App) -> impl IntoElement {
    icon_button("cuda-close", AppIcon::X, 24., 13., R_SM, cx)
        .absolute()
        .top(px(12.))
        .right(px(12.))
        .on_click(|_, _, cx| store_do(cx, Store::close_cuda_offer))
}

fn close(_: &mut Window, cx: &mut App) {
    store_do(cx, Store::close_cuda_offer);
}

fn download(_: &mut Window, cx: &mut App) {
    store_do(cx, Store::download_cuda);
}

fn downloading(percent: u8, p: &Palette, cx: &App) -> AnyElement {
    div()
        .child(badge(Tone::Accent, AppIcon::Zap, p))
        .child(title("Downloading CUDA", p))
        .child(text(
            "You can keep using NexusVoice on Vulkan while this finishes.".into(),
            p,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(24.))
                .mt(px(16.))
                .mb(px(4.))
                .child(progress_bar(percent, 3., p.border, p.accent))
                .child(
                    div()
                        .min_w(px(32.))
                        .text_right()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent)
                        .child(format!("{percent}%")),
                )
                .child(
                    icon_button("cuda-cancel", AppIcon::X, 24., 13., R_SM, cx)
                        .on_click(|_, _, cx| store_do(cx, Store::cancel_cuda_download)),
                ),
        )
        .into_any_element()
}

fn failed(error: &str, p: &Palette, cx: &App) -> AnyElement {
    div()
        .child(badge(Tone::Danger, AppIcon::AlertCircle, p))
        .child(title("CUDA download failed", p))
        .child(text(
            format!("{error}. NexusVoice keeps working on Vulkan, and you can retry later from Settings → About."),
            p,
        ))
        .child(actions(
            secondary("cuda-close-failed", "Close").on_click(close),
            primary("cuda-retry", "Retry", AppIcon::RefreshCw).on_click(download),
        ))
        .into_any_element()
}

fn ready(p: &Palette, cx: &App) -> AnyElement {
    div()
        .child(badge(Tone::Success, AppIcon::CheckCircle, p))
        .child(title("CUDA is ready", p))
        .child(text(
            "Restart NexusVoice to start transcribing on CUDA.".into(),
            p,
        ))
        .child(actions(
            secondary("cuda-later", "Later").on_click(close),
            primary("cuda-restart", "Restart now", AppIcon::RotateCcw)
                .on_click(|_, cx| store_do(cx, Store::restart)),
        ))
        .into_any_element()
}

fn offer(gpu: &str, p: &Palette, cx: &App) -> AnyElement {
    div()
        .child(badge(Tone::Accent, AppIcon::Zap, p))
        .child(title("Your NVIDIA GPU supports CUDA", p))
        .child(text(
            format!("CUDA makes transcription faster on {gpu}. You can add it now or later from Settings → About."),
            p,
        ))
        .child(
            div()
                .flex()
                .gap(px(14.))
                .mt(px(14.))
                .text_size(px(11.))
                .text_color(p.fg_2)
                .child(fact(AppIcon::HardDrive, "About 500 MB download", p))
                .child(fact(AppIcon::RotateCcw, "Applies after a restart", p)),
        )
        .child(actions(
            secondary("cuda-not-now", "Not now").on_click(close),
            primary("cuda-download", "Download CUDA", AppIcon::Download).on_click(download),
        ))
        .into_any_element()
}

impl Render for CudaOffer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let store = Store::entity(cx).read(cx);
        let progress = store.cuda.and_then(|c| c.download_progress);
        let body = if let Some(percent) = progress {
            downloading(percent, &p, cx)
        } else if let Some(error) = store.cuda_error.as_deref() {
            failed(error, &p, cx)
        } else if store.cuda.map(|c| c.pack) == Some(PackState::PendingInstall) {
            ready(&p, cx)
        } else {
            offer(self.gpu_name.as_deref().unwrap_or("your GPU"), &p, cx)
        };
        modal(
            "cuda-offer",
            px(380.),
            div()
                .relative()
                .p(px(20.))
                .child(body)
                .when(progress.is_none(), |el| el.child(close_button(cx))),
            None,
            cx,
        )
    }
}

fn fact(glyph: AppIcon, label: &'static str, p: &Palette) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .child(icon(glyph, 11., p.muted))
        .child(label)
}
