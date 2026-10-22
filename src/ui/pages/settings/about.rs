//! About: the version, the machine, the CUDA pack and app updates.

use gpui_kit::{div, prelude::*, px, AnyElement, Context, FontWeight, Subscription, Window};

use crate::api::{self, DownloadedModel, HardwareProfileResponse};
use crate::inference::cuda::PackState;
use crate::ui::components::{
    button, icon, icon_button, progress_bar, spinner, ButtonKind, ButtonSize, Tone,
};
use crate::ui::icons::AppIcon;
use crate::ui::pages::models::format_about_size;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::store::{Store, UpdateState, UpdateStatus};
use crate::ui::theme::{alpha, Palette, R_LG, R_MD, R_SM};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Check,
    CheckAgain,
    Checking,
    RetryCheck,
    InstallUpdate,
    Restart,
    DownloadCuda,
    RemoveCuda,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Check => "Check",
            Self::CheckAgain => "Check again",
            Self::Checking => "Checking…",
            Self::RetryCheck => "Retry",
            Self::InstallUpdate | Self::DownloadCuda => "Download",
            Self::Restart => "Restart",
            Self::RemoveCuda => "Remove",
        }
    }

    fn glyph(self) -> AppIcon {
        match self {
            Self::Check | Self::CheckAgain | Self::Checking | Self::RetryCheck => {
                AppIcon::RefreshCw
            }
            Self::InstallUpdate | Self::DownloadCuda => AppIcon::Download,
            Self::Restart => AppIcon::RotateCcw,
            Self::RemoveCuda => AppIcon::Trash,
        }
    }

    fn run(self, store: &mut Store, cx: &mut Context<Store>) {
        match self {
            Self::Check | Self::CheckAgain | Self::RetryCheck => store.check_for_update(cx),
            Self::Checking => {}
            Self::InstallUpdate => store.install_update(cx),
            Self::Restart => store.restart(cx),
            Self::DownloadCuda => store.download_cuda(cx),
            Self::RemoveCuda => store.remove_cuda(cx),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Body {
    Detail { text: String, tone: Option<Tone> },
    Progress { percent: u8, cancel: bool },
}

/// A flat status card: badge, title, then a detail or progress line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tile {
    tone: Tone,
    glyph: AppIcon,
    spinning: bool,
    title: String,
    body: Body,
    action: Option<Action>,
}

impl Tile {
    fn detail(tone: Tone, glyph: AppIcon, title: &str, text: &str, action: Action) -> Self {
        Self {
            tone,
            glyph,
            spinning: false,
            title: title.to_string(),
            body: Body::Detail {
                text: text.to_string(),
                tone: None,
            },
            action: Some(action),
        }
    }
}

fn update_tile(update: &UpdateState, current: &str) -> Tile {
    let version = update.version.as_deref().unwrap_or_default();
    match update.status {
        UpdateStatus::Downloading => Tile {
            tone: Tone::Accent,
            glyph: AppIcon::Download,
            spinning: false,
            title: format!("Downloading v{version}"),
            body: Body::Progress {
                percent: update.progress,
                cancel: false,
            },
            action: None,
        },
        UpdateStatus::Idle => Tile::detail(
            Tone::Accent,
            AppIcon::RefreshCw,
            "Check for updates",
            &format!("Currently on v{current}"),
            Action::Check,
        ),
        UpdateStatus::Checking => Tile {
            spinning: true,
            ..Tile::detail(
                Tone::Accent,
                AppIcon::RefreshCw,
                "Looking for updates…",
                "Please wait…",
                Action::Checking,
            )
        },
        UpdateStatus::UpToDate => Tile::detail(
            Tone::Success,
            AppIcon::CheckCircle,
            "You're up to date",
            &format!("v{current} is the latest"),
            Action::CheckAgain,
        ),
        UpdateStatus::Available => Tile::detail(
            Tone::Accent,
            AppIcon::ArrowUpCircle,
            &format!("v{version} available"),
            "Ready to download",
            Action::InstallUpdate,
        ),
        UpdateStatus::Ready => Tile::detail(
            Tone::Success,
            AppIcon::CheckCircle,
            "Ready to install",
            &format!("Restart to apply v{version}"),
            Action::Restart,
        ),
        UpdateStatus::Failed => Tile::detail(
            Tone::Danger,
            AppIcon::AlertCircle,
            update.error.as_deref().unwrap_or("Update failed"),
            "Check your network connection",
            Action::RetryCheck,
        ),
    }
}

/// `None` where CUDA does not apply.
fn cuda_tile(
    cuda: Option<crate::api::CudaStatus>,
    backend: Option<&str>,
    error: Option<&str>,
) -> Option<Tile> {
    let cuda = cuda.filter(|c| c.supported)?;
    if let Some(percent) = cuda.download_progress {
        return Some(Tile {
            tone: Tone::Accent,
            glyph: AppIcon::Zap,
            spinning: false,
            title: "Downloading CUDA".into(),
            body: Body::Progress {
                percent,
                cancel: true,
            },
            action: None,
        });
    }
    // A failed remove or download replaces the hint under the title.
    let with_error = |tile: Tile| match (error, &tile.body) {
        (Some(error), Body::Detail { .. }) => Tile {
            body: Body::Detail {
                text: error.to_string(),
                tone: Some(Tone::Danger),
            },
            ..tile
        },
        _ => tile,
    };
    let active = backend == Some("cuda");
    Some(match cuda.pack {
        PackState::PendingInstall => Tile::detail(
            Tone::Accent,
            AppIcon::Zap,
            "CUDA downloaded",
            "Restart to start using it",
            Action::Restart,
        ),
        PackState::PendingRemoval => Tile::detail(
            Tone::Accent,
            AppIcon::Zap,
            "CUDA removed",
            "Restart to finish removing it",
            Action::Restart,
        ),
        PackState::Installed => with_error(Tile::detail(
            if active { Tone::Success } else { Tone::Accent },
            AppIcon::Zap,
            if active {
                "CUDA active"
            } else {
                "CUDA installed, not in use"
            },
            if active {
                "Transcription runs on CUDA"
            } else {
                "CUDA could not start, so NexusVoice uses Vulkan"
            },
            Action::RemoveCuda,
        )),
        PackState::Absent => with_error(Tile::detail(
            Tone::Accent,
            AppIcon::Zap,
            "CUDA acceleration",
            "Faster on NVIDIA GPUs · about 500 MB download",
            Action::DownloadCuda,
        )),
    })
}

pub struct AboutTab {
    profile: Option<HardwareProfileResponse>,
    on_disk: Vec<DownloadedModel>,
    _store: Subscription,
}

impl AboutTab {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::entity(cx);
        let subscription = cx.observe(&store, |_, _, cx| cx.notify());
        store.update(cx, Store::refresh_cuda);
        cx.spawn_api(
            async { (api::get_hardware_profile(), api::get_downloaded_models()) },
            |this: &mut Self, (profile, on_disk), cx| {
                this.profile = Some(profile);
                this.on_disk = on_disk;
                cx.notify();
            },
        );
        Self {
            profile: None,
            on_disk: Vec::new(),
            _store: subscription,
        }
    }

    fn render_hero(p: &Palette) -> impl IntoElement {
        let chip = |glyph: AppIcon, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(10.))
                .py(px(6.))
                .rounded(R_MD)
                .border_1()
                .border_color(p.border_soft)
                .bg(p.surface)
                .text_size(px(11.))
                .text_color(p.fg_2)
                .child(icon(glyph, 11., p.muted))
                .child(label)
        };
        div()
            .flex()
            .items_center()
            .gap(px(14.))
            .p(px(16.))
            .rounded(R_LG)
            .border_1()
            .border_color(p.border_soft)
            .bg(p.panel)
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(44.))
                    .rounded(R_LG)
                    .bg(p.accent_soft)
                    .child(icon(AppIcon::Mic, 20., p.accent)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(p.fg)
                                    .child("NexusVoice"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.muted)
                                    .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child("Local speech to text. Audio never leaves this machine."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .gap(px(8.))
                    .child(chip(AppIcon::Cpu, "transcribe-cpp"))
                    .child(chip(AppIcon::Shield, "100% on-device")),
            )
    }

    fn render_system(&self, p: &Palette) -> impl IntoElement {
        let detecting = || "Detecting…".to_string();
        let compute = self.profile.as_ref().map_or_else(detecting, |profile| {
            format!(
                "{} · {}",
                profile.gpu_name,
                profile.execution_provider.to_uppercase()
            )
        });
        let memory = self.profile.as_ref().map_or_else(detecting, |profile| {
            let vram = if profile.vram_gb > 0. {
                format!(" · {} GB VRAM", profile.vram_gb)
            } else {
                String::new()
            };
            format!("{} GB RAM{vram}", profile.ram_gb)
        });
        let models = match self.on_disk.len() {
            0 => "None downloaded".to_string(),
            n => {
                let total = self.on_disk.iter().map(|m| m.size_bytes).sum();
                format!(
                    "{n} model{} · {}",
                    if n > 1 { "s" } else { "" },
                    format_about_size(total)
                )
            }
        };
        let rows = [
            (AppIcon::Monitor, "Compute", compute),
            (AppIcon::MemoryStick, "Memory", memory),
            (AppIcon::HardDrive, "Models on disk", models),
        ];
        div()
            .overflow_hidden()
            .rounded(R_LG)
            .border_1()
            .border_color(p.border_soft)
            .bg(p.panel)
            .child(
                div()
                    .px(px(16.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(p.border_soft)
                    .child(section_label("System", p)),
            )
            .children(
                rows.into_iter()
                    .enumerate()
                    .map(|(i, (glyph, label, value))| {
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(16.))
                            .px(px(16.))
                            .py(px(10.))
                            .when(i > 0, |el| el.border_t_1().border_color(p.border_soft))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .text_size(px(12.))
                                    .text_color(p.fg_2)
                                    .child(icon(glyph, 13., p.muted))
                                    .child(label),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_right()
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .child(value),
                            )
                    }),
            )
    }

    fn render_tile(id: &'static str, tile: Tile, cx: &Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let (tint, ink) = tile.tone.badge(&p);
        let glyph: AnyElement = if tile.spinning {
            spinner(
                "about-spinner",
                15.,
                2.,
                alpha(ink, 0.3),
                ink,
                std::time::Duration::from_secs(1),
            )
            .into_any_element()
        } else {
            icon(tile.glyph, 15., ink).into_any_element()
        };
        let body = match tile.body {
            Body::Detail { text, tone } => div()
                .mt(px(2.))
                .text_size(px(11.))
                .text_color(tone.map_or(p.muted, |t| t.title(&p)))
                .child(text)
                .into_any_element(),
            Body::Progress { percent, cancel } => div()
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(24.))
                .mt(px(4.))
                .child(progress_bar(percent, 3., p.border_soft, p.accent))
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent)
                        .child(format!("{percent}%")),
                )
                .when(cancel, |el| {
                    el.child(
                        icon_button("about-cancel", AppIcon::X, 24., 13., R_SM, cx).on_click(
                            |_, _, cx| {
                                Store::entity(cx).update(cx, Store::cancel_cuda_download);
                            },
                        ),
                    )
                })
                .into_any_element(),
        };
        div()
            .flex()
            .items_center()
            .gap(px(14.))
            .px(px(16.))
            .py(px(14.))
            .rounded(R_LG)
            .bg(p.panel)
            .border_1()
            .border_color(p.border_soft)
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(34.))
                    .rounded(R_MD)
                    .bg(tint)
                    .child(glyph),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(tile.tone.title(&p))
                            .child(tile.title),
                    )
                    .child(body),
            )
            .children(tile.action.map(|action| {
                let kind = if action == Action::RemoveCuda {
                    ButtonKind::OutlineDanger
                } else {
                    ButtonKind::Primary
                };
                button((id, 0usize), action.label())
                    .size(ButtonSize::Sm)
                    .kind(kind)
                    .icon(action.glyph())
                    .busy(action == Action::Checking)
                    .on_click(move |_, cx| Store::entity(cx).update(cx, |s, cx| action.run(s, cx)))
            }))
    }

    fn section(label: &'static str, tile: impl IntoElement, p: &Palette) -> impl IntoElement {
        div()
            .child(div().mb(px(8.)).ml(px(4.)).child(section_label(label, p)))
            .child(tile)
    }
}

fn section_label(text: &'static str, p: &Palette) -> impl IntoElement {
    div()
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.muted)
        .child(text.to_uppercase())
}

impl Render for AboutTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let store = Store::entity(cx).read(cx);
        let backend = self
            .profile
            .as_ref()
            .map(|profile| profile.execution_provider.to_lowercase());
        let cuda = cuda_tile(store.cuda, backend.as_deref(), store.cuda_error.as_deref());
        let update = update_tile(&store.update, env!("CARGO_PKG_VERSION"));

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(Self::render_hero(&p))
            .child(self.render_system(&p))
            .children(cuda.map(|tile| {
                Self::section(
                    "GPU acceleration",
                    Self::render_tile("cuda-tile", tile, cx),
                    &p,
                )
            }))
            .child(Self::section(
                "Updates",
                Self::render_tile("update-tile", update, cx),
                &p,
            ))
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/ui/pages/about.rs"]
mod tests;
