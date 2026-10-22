//! General: input first, since the rest of the app assumes it is right.

mod formatting;
mod hotkeys;
mod injection;
mod language;
mod live_transcript;
mod microphone;
mod shortcut;

use gpui_kit::{div, prelude::*, Context, Entity, Subscription, Window};

use self::formatting::FormattingToggle;
use self::hotkeys::HotkeySection;
use self::injection::TextInjectionSection;
use self::language::LanguageSection;
use self::live_transcript::LiveTranscriptToggle;
use self::microphone::MicrophoneSection;
use crate::ui::store::Store;
use crate::ui::theme::{Palette, R_LG};

pub struct GeneralTab {
    microphone: Entity<MicrophoneSection>,
    language: Entity<LanguageSection>,
    formatting: Entity<FormattingToggle>,
    live_transcript: Entity<LiveTranscriptToggle>,
    hotkeys: Entity<HotkeySection>,
    injection: Entity<TextInjectionSection>,
    _language_changed: Subscription,
}

impl GeneralTab {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Settings can open straight here, before anything resolved the model.
        Store::entity(cx).update(cx, Store::refresh_models);

        let language = cx.new(|cx| LanguageSection::new(window, cx));
        Self {
            microphone: cx.new(|cx| MicrophoneSection::new(window, cx)),
            formatting: cx.new(|cx| FormattingToggle::new(window, cx)),
            live_transcript: cx.new(|cx| LiveTranscriptToggle::new(window, cx)),
            hotkeys: cx.new(|cx| HotkeySection::new(window, cx)),
            injection: cx.new(|cx| TextInjectionSection::new(window, cx)),
            _language_changed: cx.observe(&language, |_, _, cx| cx.notify()),
            language,
        }
    }
}

impl Render for GeneralTab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let card = || {
            div()
                .rounded(R_LG)
                .bg(p.panel)
                .border_1()
                .border_color(p.border_soft)
                .p_4()
        };
        let language_supported = self.language.read(cx).supported();
        div()
            .flex()
            .flex_col()
            .gap_4()
            // Both selects open panels past the card edge.
            .child(
                card()
                    .grid()
                    .grid_cols(if language_supported { 2 } else { 1 })
                    .gap_4()
                    .child(div().min_w_0().child(self.microphone.clone()))
                    .when(language_supported, |el| {
                        el.child(div().min_w_0().child(self.language.clone()))
                    }),
            )
            .child(self.formatting.clone())
            .child(self.live_transcript.clone())
            .child(card().child(self.hotkeys.clone()))
            .child(self.injection.clone())
    }
}
