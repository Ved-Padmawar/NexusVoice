//! AppearanceTab.

use gpui_kit::{div, prelude::*, Context, Window};

pub struct AppearanceTab;

impl AppearanceTab {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for AppearanceTab {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
