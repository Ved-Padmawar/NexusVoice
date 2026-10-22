//! Appearance: app theme, pill theme and waveform style, each with a live preview.

mod app_theme;
mod card;
mod levels;
mod pill_theme;
mod waveform_style;

use gpui_kit::{div, prelude::*, Context, Entity, Window};

use self::app_theme::ThemeSection;
use self::pill_theme::PillSection;
use self::waveform_style::WaveformSection;

pub struct AppearanceTab {
    theme: Entity<ThemeSection>,
    pill: Entity<PillSection>,
    waveform: Entity<WaveformSection>,
}

impl AppearanceTab {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            theme: cx.new(ThemeSection::new),
            pill: cx.new(PillSection::new),
            waveform: cx.new(WaveformSection::new),
        }
    }
}

impl Render for AppearanceTab {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(self.theme.clone())
            .child(self.pill.clone())
            .child(self.waveform.clone())
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/ui/pages/appearance.rs"]
mod tests;
