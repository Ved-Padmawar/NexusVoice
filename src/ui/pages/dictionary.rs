//! Dictionary.

use gpui_kit::{div, prelude::*, Context, Window};

pub struct Dictionary;

impl Dictionary {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for Dictionary {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
