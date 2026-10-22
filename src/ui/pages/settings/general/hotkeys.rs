//! Global hotkeys: one recorder per action.

use gpui_kit::component::{notification::Notification, Icon, WindowExt as _};
use gpui_kit::{
    div, prelude::*, px, App, ClickEvent, Context, Entity, FocusHandle, FontWeight, Hsla,
    KeyDownEvent, KeyUpEvent, ModifiersChangedEvent, SharedString, Window,
};

use super::shortcut::{badge_label, Recording};
use crate::state::HotkeyKind;
use crate::ui::components::{icon, section_heading};
use crate::ui::icons::AppIcon;
use crate::ui::shell::Hotkeys;
use crate::ui::theme::{alpha, Palette, R_MD, R_SM};

struct Spec {
    kind: HotkeyKind,
    title: &'static str,
    description: &'static str,
    icon: AppIcon,
}

const SPECS: [Spec; 3] = [
    Spec {
        kind: HotkeyKind::PushToTalk,
        title: "Recording Hotkey",
        description: "Hold to record. Release to transcribe and paste.",
        icon: AppIcon::Keyboard,
    },
    Spec {
        kind: HotkeyKind::Dictation,
        title: "Dictation Hotkey",
        description: "Press to start, pause, or resume dictation.",
        icon: AppIcon::Mic,
    },
    Spec {
        kind: HotkeyKind::DictationCommit,
        title: "Commit Dictation Hotkey",
        description: "Press to save the current dictation without using the pill button.",
        icon: AppIcon::Save,
    },
];

pub struct HotkeySection {
    cards: Vec<Entity<HotkeyCard>>,
}

impl HotkeySection {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            cards: SPECS
                .iter()
                .map(|spec| cx.new(|cx| HotkeyCard::new(spec, cx)))
                .collect(),
        }
    }
}

impl Render for HotkeySection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(section_heading(
                "Keyboard shortcuts",
                "Set global hotkeys for recording and hands-free dictation.",
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .children(self.cards.iter().cloned()),
            )
    }
}

struct HotkeyCard {
    spec: &'static Spec,
    /// Capturing keys. Ends with the first key or modifier let go.
    listening: bool,
    recording: Recording,
    /// The hotkey unbound while recording, put back if the change is dropped.
    restore: Option<String>,
    focus: FocusHandle,
}

impl HotkeyCard {
    fn new(spec: &'static Spec, cx: &mut Context<Self>) -> Self {
        Self {
            spec,
            listening: false,
            recording: Recording::default(),
            restore: None,
            focus: cx.focus_handle(),
        }
    }

    fn current(&self, cx: &App) -> Option<String> {
        Hotkeys::bindings(cx)
            .and_then(|bindings| bindings.get(self.spec.kind))
            .map(str::to_owned)
    }

    fn editing(&self) -> bool {
        self.listening || !self.recording.is_empty()
    }

    fn toast_error(window: &mut Window, message: &str, cx: &mut App) {
        window.push_notification(Notification::error(message.to_string()), cx);
    }

    /// The grabbed hotkey would swallow the combination being recorded, so it is
    /// unbound until the recording settles.
    fn start_listening(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing() {
            self.restore = self.current(cx);
            if self.restore.is_some() {
                if let Err(e) = Hotkeys::set(self.spec.kind, None, cx) {
                    log::warn!("could not unbind the {} hotkey: {e}", self.spec.title);
                }
            }
        }
        self.listening = true;
        self.recording = Recording::default();
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.listening = false;
        self.recording = Recording::default();
        if let Some(previous) = self.restore.take() {
            if let Err(e) = Hotkeys::set(self.spec.kind, Some(&previous), cx) {
                log::warn!("could not restore the {} hotkey: {e}", self.spec.title);
            }
        }
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(shortcut) = self.recording.shortcut() else {
            Self::toast_error(window, "Invalid combination - use modifier + key", cx);
            return;
        };
        match Hotkeys::set(self.spec.kind, Some(&shortcut), cx) {
            Ok(()) => {
                self.listening = false;
                self.recording = Recording::default();
                self.restore = None;
                window.push_notification(
                    Notification::success(format!("{} registered", self.spec.title)),
                    cx,
                );
            }
            Err(e) => Self::toast_error(window, e.message(), cx),
        }
        cx.notify();
    }

    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match Hotkeys::set(self.spec.kind, None, cx) {
            Ok(()) => cx.notify(),
            Err(e) => Self::toast_error(window, e.message(), cx),
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.listening {
            self.recording.press_key(&event.keystroke);
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn on_key_up(&mut self, _: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.listening {
            self.listening = false;
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn on_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.listening {
            return;
        }
        if self.recording.modifier_released(event.modifiers) {
            self.listening = false;
        } else {
            self.recording.press_modifiers(event.modifiers);
        }
        cx.notify();
    }

    fn recorder(&self, current: Option<&str>, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let editing = self.editing();
        let tokens: Vec<SharedString> = if editing {
            self.recording
                .tokens()
                .map(|t| t.to_string().into())
                .collect()
        } else {
            current
                .map(|c| c.split('+').map(|t| t.to_string().into()).collect())
                .unwrap_or_default()
        };
        div()
            .id("recorder")
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .px(px(10.))
            .h_8()
            .min_w(px(120.))
            .rounded(R_SM)
            .cursor_pointer()
            .border_1()
            .map(|el| {
                if editing {
                    el.border_color(alpha(p.accent, 0.55)).bg(p.accent_soft)
                } else {
                    el.border_color(p.border_soft).bg(p.bg_alt)
                }
            })
            .on_click(
                cx.listener(|this, _: &ClickEvent, window, cx| this.start_listening(window, cx)),
            )
            .when(!tokens.is_empty(), |el| el.child(key_badges(&tokens, &p)))
            .when(tokens.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .italic()
                        .text_color(if self.listening { p.accent } else { p.muted })
                        .child(if self.listening {
                            "Press keys…"
                        } else {
                            "Click to set…"
                        }),
                )
            })
    }

    fn actions(&self, current: Option<&str>, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let row = div().flex().flex_shrink_0().items_center().gap(px(6.));
        if self.editing() {
            let can_save = !self.recording.is_empty();
            return row
                .child(
                    text_button("save-hotkey", "Save", true, !can_save, &p)
                        .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                )
                .child(
                    text_button("cancel-hotkey", "Cancel", false, false, &p)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                );
        }
        if current.is_none() {
            return row.child(
                div()
                    .px_2()
                    .py_1()
                    .rounded(R_SM)
                    .bg(p.bg_alt)
                    .border_1()
                    .border_color(p.border_soft)
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.muted)
                    .child("Not set"),
            );
        }
        row.child(
            icon_button(
                "change-hotkey",
                AppIcon::Pencil,
                p.accent,
                p.accent_soft,
                p.fg_2,
            )
            .on_click(cx.listener(|this, _, window, cx| this.start_listening(window, cx))),
        )
        .child(
            icon_button(
                "remove-hotkey",
                AppIcon::X,
                p.danger,
                alpha(p.danger, 0.12),
                p.fg_2,
            )
            .on_click(cx.listener(|this, _, window, cx| this.remove(window, cx))),
        )
    }
}

impl Render for HotkeyCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let current = self.current(cx);
        let bound = current.is_some();
        div()
            .id(SharedString::from(self.spec.kind.as_str()))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_key_up(cx.listener(Self::on_key_up))
            .on_modifiers_changed(cx.listener(Self::on_modifiers))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.listening {
                    this.cancel(cx);
                }
            }))
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .py(px(10.))
            .rounded(R_MD)
            .bg(p.surface)
            .border_1()
            .border_color(if self.editing() {
                alpha(p.accent, 0.55)
            } else {
                p.border_soft
            })
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size_7()
                    .rounded(R_SM)
                    .bg(if bound { p.accent_soft } else { p.bg_alt })
                    .child(icon(
                        self.spec.icon,
                        14.,
                        if bound { p.accent } else { p.muted },
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(px(15.))
                            .text_color(p.fg)
                            .child(self.spec.title),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(10.5))
                            .line_height(px(13.))
                            .text_color(p.muted)
                            .child(self.spec.description),
                    ),
            )
            .child(self.recorder(current.as_deref(), cx))
            .child(self.actions(current.as_deref(), cx))
    }
}

fn key_badges(tokens: &[SharedString], p: &Palette) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(3.))
        .children(tokens.iter().enumerate().map(|(i, token)| {
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .when(i > 0, |el| {
                    el.child(
                        div()
                            .px(px(1.))
                            .text_size(px(9.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.muted)
                            .child("+"),
                    )
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .min_w_6()
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(R_SM)
                        .bg(p.bg_alt)
                        .border_1()
                        .border_color(p.border)
                        .text_size(px(10.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.fg)
                        .child(badge_label(token).to_string()),
                )
        }))
}

fn text_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    disabled: bool,
    p: &Palette,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h_8()
        .px_3()
        .rounded(R_MD)
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .map(|el| {
            if primary {
                el.bg(p.accent).text_color(p.accent_fg)
            } else {
                el.text_color(p.muted)
            }
        })
        .when(disabled, |el| el.opacity(0.5))
        .when(!disabled, |el| {
            el.cursor_pointer().hover(|s| {
                if primary {
                    s.bg(p.accent_hover)
                } else {
                    s.bg(p.surface_hover).text_color(p.fg)
                }
            })
        })
        .child(label)
}

fn icon_button(
    id: &'static str,
    name: AppIcon,
    hover_color: Hsla,
    hover_bg: Hsla,
    rest_color: Hsla,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size_7()
        .rounded(R_SM)
        .cursor_pointer()
        .text_color(rest_color)
        .hover(move |s| s.bg(hover_bg).text_color(hover_color))
        .child(Icon::new(name).size(px(12.)))
}
