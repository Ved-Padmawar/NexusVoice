//! Appearance: app theme, pill theme and waveform style, each with a live preview.

mod app_theme;
mod card;
mod levels;
mod pill_theme;
mod waveform_style;

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, AnyView, Context, Entity, Pixels, StyleRefinement,
    Window,
};

use self::app_theme::ThemeSection;
use self::pill_theme::PillSection;
use self::waveform_style::WaveformSection;

/// The waveform previews animate every frame, and a frame re-renders every view
/// above them. GPUI skips a view only when it is cached at a definite size, so
/// the still sections are laid out once to learn their height, then cached.
#[derive(Default, Clone)]
struct Measured(Rc<Cell<Option<Pixels>>>);

impl Measured {
    fn render(&self, view: AnyView) -> AnyElement {
        if let Some(height) = self.0.get() {
            return view
                .cached(StyleRefinement::default().w_full().h(height))
                .into_any_element();
        }
        let measured = self.0.clone();
        let probe = canvas(
            move |bounds, _, _| measured.set(Some(bounds.size.height)),
            // Measured: render again, now cached.
            |_, (), window, _| window.request_animation_frame(),
        )
        .absolute()
        .size_full();
        div().relative().child(view).child(probe).into_any_element()
    }
}

pub struct AppearanceTab {
    theme: Entity<ThemeSection>,
    pill: Entity<PillSection>,
    waveform: Entity<WaveformSection>,
    theme_height: Measured,
    pill_height: Measured,
}

impl AppearanceTab {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            theme: cx.new(ThemeSection::new),
            pill: cx.new(PillSection::new),
            waveform: cx.new(WaveformSection::new),
            theme_height: Measured::default(),
            pill_height: Measured::default(),
        }
    }
}

impl Render for AppearanceTab {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(self.theme_height.render(self.theme.clone().into()))
            .child(self.pill_height.render(self.pill.clone().into()))
            .child(self.waveform.clone())
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/ui/pages/appearance.rs"]
mod tests;
