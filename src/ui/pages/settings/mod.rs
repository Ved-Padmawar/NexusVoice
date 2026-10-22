//! Settings: four tabs under one header.

mod about;
mod appearance;
mod general;
mod voice;

pub use about::AboutTab;
pub use appearance::AppearanceTab;
pub use general::GeneralTab;
pub use voice::VoiceTab;

use gpui_kit::{div, prelude::*, Context, Window};

pub struct Settings;

impl Settings {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
