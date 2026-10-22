//! AboutTab.

use gpui_kit::{div, prelude::*, Context, Window};

pub struct AboutTab;

impl AboutTab {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for AboutTab {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
