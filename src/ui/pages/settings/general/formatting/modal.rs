//! The provider modal: pick a provider, fill in its endpoint, test it, save.

use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{notification::Notification, Icon, WindowExt as _};
use gpui_kit::{
    div, prelude::*, px, App, ClickEvent, Context, Div, ElementId, Entity, EventEmitter,
    FocusHandle, FontWeight, KeyDownEvent, SharedString, Stateful, Subscription, Window,
};

use super::presets::{draft, is_configured, preset, profile_for_switch, shows_base_url, Preset, PRESETS};
use crate::api;
use crate::llm::config::{FormatConfig, Profile};
use crate::ui::components::{icon, modal, text_input};
use crate::ui::icons::AppIcon;
use super::super::widgets::{border_width, spinning_icon};
use crate::ui::runtime::SpawnApi as _;
use crate::ui::theme::{alpha, Palette, R_LG, R_MD};
use crate::ui::vendors::{vendor_mark, Vendor};

/// How long a test result's icon stays before the button is ready again.
const RESULT_FLASH: Duration = Duration::from_secs(4);

pub enum ProviderModalEvent {
    /// Saved and enabled; carries the stored config.
    Saved(FormatConfig),
    /// Closed without saving.
    Closed,
}

impl EventEmitter<ProviderModalEvent> for ProviderModal {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TestResult {
    Idle,
    Ok,
    Failed,
}

pub struct ProviderModal {
    initial: FormatConfig,
    provider: String,
    base_url: Entity<InputState>,
    model: Entity<InputState>,
    api_key: Entity<InputState>,
    testing: bool,
    test_result: TestResult,
    /// Newest test result; the timer that clears an older one does nothing.
    flash: u64,
    saving: bool,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl ProviderModal {
    pub fn new(initial: FormatConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let provider = if initial.provider.is_empty() {
            PRESETS[0].id.to_string()
        } else {
            initial.provider.clone()
        };
        let seed = initial.active();
        let preset = preset(&provider);
        let field = |window: &mut Window, cx: &mut Context<Self>, value: &str, placeholder: &str, masked: bool| {
            cx.new(|cx| {
                let mut state = InputState::new(window, cx)
                    .placeholder(placeholder.to_string())
                    .masked(masked);
                state.set_value(value.to_string(), window, cx);
                state
            })
        };
        let base_url = field(window, cx, &seed.base_url, base_url_hint(preset), false);
        let model = field(window, cx, &seed.model, preset.model_hint, false);
        let api_key = field(window, cx, &seed.api_key, key_hint(preset), true);

        // Editing a field makes an earlier test result stale.
        let subscriptions = [&base_url, &model, &api_key]
            .into_iter()
            .map(|state| {
                cx.subscribe(state, |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) && this.test_result != TestResult::Idle {
                        this.test_result = TestResult::Idle;
                    }
                    cx.notify();
                })
            })
            .collect();

        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            initial,
            provider,
            base_url,
            model,
            api_key,
            testing: false,
            test_result: TestResult::Idle,
            flash: 0,
            saving: false,
            focus,
            _subscriptions: subscriptions,
        }
    }

    fn form(&self, cx: &App) -> Profile {
        Profile {
            base_url: self.base_url.read(cx).value().to_string(),
            model: self.model.read(cx).value().to_string(),
            api_key: self.api_key.read(cx).value().to_string(),
        }
    }

    fn can_submit(&self, cx: &App) -> bool {
        let form = self.form(cx);
        is_configured(&self.provider, &form.base_url, &form.model)
    }

    fn draft(&self, cx: &App) -> FormatConfig {
        draft(&self.initial, &self.provider, &self.form(cx))
    }

    fn select_provider(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.provider = id.to_string();
        self.test_result = TestResult::Idle;
        let profile = profile_for_switch(id, self.initial.profiles.get(id));
        let preset = preset(id);
        fill(&self.base_url, &profile.base_url, base_url_hint(preset), window, cx);
        fill(&self.model, &profile.model, preset.model_hint, window, cx);
        fill(&self.api_key, &profile.api_key, key_hint(preset), window, cx);
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(ProviderModalEvent::Closed);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" {
            self.close(cx);
            cx.stop_propagation();
        }
    }

    fn test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_submit(cx) || self.testing {
            return;
        }
        self.testing = true;
        self.test_result = TestResult::Idle;
        let config = self.draft(cx);
        cx.spawn_api_in(
            window,
            async move { api::test_format_connection(&config).await },
            |this, result, window, cx| {
                this.testing = false;
                let notification = match result {
                    Ok(()) => {
                        this.flash_result(TestResult::Ok, cx);
                        Notification::success("Connection successful")
                    }
                    Err(e) => {
                        this.flash_result(TestResult::Failed, cx);
                        let reason = match e.message() {
                            "" => "could not reach endpoint",
                            message => message,
                        };
                        Notification::error(format!("Connection failed: {reason}"))
                    }
                };
                window.push_notification(notification, cx);
                cx.notify();
            },
        );
        cx.notify();
    }

    /// Show a result briefly, then return the button to its ready state:
    /// leaving it on a past result would mislead.
    fn flash_result(&mut self, result: TestResult, cx: &mut Context<Self>) {
        self.test_result = result;
        self.flash += 1;
        let flash = self.flash;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RESULT_FLASH).await;
            let _ = this.update(cx, |this, cx| {
                if this.flash == flash && this.test_result != TestResult::Idle {
                    this.test_result = TestResult::Idle;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Saving also enables formatting: the user set up an endpoint to use it.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_submit(cx) || self.saving {
            return;
        }
        self.saving = true;
        let mut config = self.draft(cx);
        config.enabled = true;
        let to_save = config.clone();
        cx.spawn_api_in(
            window,
            async move { api::set_format_config(&to_save) },
            move |this, result, window, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        window.push_notification(Notification::success("Smart formatting enabled"), cx);
                        cx.emit(ProviderModalEvent::Saved(config));
                    }
                    Err(_) => window
                        .push_notification(Notification::error("Failed to save settings"), cx),
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    fn header(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .pt_6()
            .pb_4()
            .border_b_1()
            .border_color(p.border_soft)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .size(px(36.))
                            .rounded(R_LG)
                            .bg(p.accent_soft)
                            .child(icon(AppIcon::Sparkles, 15., p.accent)),
                    )
                    .child(
                        div()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(p.fg)
                                    .child("Formatting provider"),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child("Connect Anthropic or any OpenAI-compatible endpoint"),
                            ),
                    ),
            )
            .child(
                div()
                    .id("close-provider-modal")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_7()
                    .rounded(R_MD)
                    .text_color(p.muted)
                    .cursor_pointer()
                    .hover(|s| s.bg(alpha(p.danger, 0.12)).text_color(p.danger))
                    .on_click(cx.listener(|this, _, _, cx| this.close(cx)))
                    .child(Icon::new(AppIcon::X).size(px(14.))),
            )
    }

    fn preset_card(&self, preset: &'static Preset, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let active = preset.id == self.provider;
        let color = if active { p.accent } else { p.fg };
        let id = preset.id;
        border_width(
            div()
                .id(SharedString::from(id))
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .px_3()
                .py_2()
                .rounded(R_MD)
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .cursor_pointer()
                .map(|el| {
                    if active {
                        el.bg(p.accent_soft).border_color(p.accent)
                    } else {
                        el.bg(p.surface)
                            .border_color(p.border)
                            .hover(|s| s.bg(p.surface_hover))
                    }
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.select_provider(id, window, cx);
                })),
            px(1.5),
        )
        .children(
            Vendor::from_id(id).map(|vendor| vendor_mark(vendor, 16., color, cx)),
        )
        .child(div().min_w_0().truncate().child(preset.label))
    }

    fn field(label: &'static str, p: Palette, input: Input) -> Div {
        div().flex().flex_col().gap(px(6.)).child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.fg_2)
                .child(label),
        )
        .child(input)
    }

    fn input(state: &Entity<InputState>, cx: &App) -> Input {
        text_input(state, cx).px(px(12.)).text_size(px(12.))
    }

    fn body(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let preset = preset(&self.provider);
        let model = Self::field("Model", p, Self::input(&self.model, cx)).child(
            div()
                .text_size(px(10.))
                .text_color(p.muted)
                .child(
                    "Tip: use a small instruct model (e.g. qwen2.5-3b-instruct), not a reasoning \
                     model — much faster and more accurate for formatting.",
                ),
        );
        div()
            .flex()
            .flex_col()
            .gap_4()
            .px_6()
            .py_5()
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_2()
                    .children(PRESETS.iter().map(|preset| self.preset_card(preset, p, cx))),
            )
            .when(shows_base_url(&self.provider), |el| {
                el.child(Self::field("Base URL", p, Self::input(&self.base_url, cx)))
            })
            .child(model)
            .child(Self::field(
                if preset.needs_key { "API key" } else { "API key (optional)" },
                p,
                Self::input(&self.api_key, cx).mask_toggle(),
            ))
    }

    fn footer(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = self.can_submit(cx);
        let test_disabled = !ready || self.testing;
        let save_disabled = !ready || self.saving;
        let test_icon = if self.testing {
            spinning_icon("test-spin", AppIcon::Loader, 13., p.fg_2, Duration::from_secs(1))
                .into_any_element()
        } else {
            match self.test_result {
                TestResult::Ok => icon(AppIcon::CheckCircle, 13., p.success).into_any_element(),
                TestResult::Failed => icon(AppIcon::AlertCircle, 13., p.danger).into_any_element(),
                TestResult::Idle => icon(AppIcon::Plug, 13., p.fg_2).into_any_element(),
            }
        };
        let save_icon = if self.saving {
            spinning_icon("save-spin", AppIcon::Loader, 13., p.accent_fg, Duration::from_secs(1))
                .into_any_element()
        } else {
            icon(AppIcon::Save, 13., p.accent_fg).into_any_element()
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_6()
            .pb_5()
            .pt(px(4.))
            .child(
                footer_button("test-connection", test_disabled)
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border)
                    .text_color(p.fg_2)
                    .font_weight(FontWeight::MEDIUM)
                    .when(!test_disabled, |el| el.hover(|s| s.bg(p.surface_hover)))
                    .on_click(cx.listener(|this, _, window, cx| this.test(window, cx)))
                    .child(test_icon)
                    .child("Test connection"),
            )
            .child(
                footer_button("save-and-enable", save_disabled)
                    .px_4()
                    .bg(p.accent)
                    .text_color(p.accent_fg)
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(!save_disabled, |el| el.hover(|s| s.opacity(0.9)))
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))
                    .child(save_icon)
                    .child(if self.saving { "Saving…" } else { "Save & enable" }),
            )
    }
}

/// The footer buttons' shared frame; the caller adds the colours.
fn footer_button(id: &'static str, disabled: bool) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded(R_MD)
        .text_size(px(12.))
        .when(disabled, |el| el.opacity(0.4))
        .when(!disabled, |el| el.cursor_pointer())
}

fn base_url_hint(preset: &Preset) -> &'static str {
    if preset.base_url.is_empty() {
        "http://localhost:11434/v1"
    } else {
        preset.base_url
    }
}

fn key_hint(preset: &Preset) -> &'static str {
    if preset.needs_key { "sk-…" } else { "leave blank for local servers" }
}

fn fill(
    state: &Entity<InputState>,
    value: &str,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    state.update(cx, |state, cx| {
        state.set_placeholder(placeholder, window, cx);
        state.set_value(value.to_string(), window, cx);
    });
}

impl Render for ProviderModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let content = div()
            .id("provider-modal")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .flex()
            .flex_col()
            .child(self.header(p, cx))
            .child(self.body(p, cx))
            .child(self.footer(p, cx));
        let this = cx.entity();
        modal(
            "provider-modal-overlay",
            px(460.),
            content,
            Some(Box::new(move |_, cx| this.update(cx, |this, cx| this.close(cx)))),
            cx,
        )
    }
}
