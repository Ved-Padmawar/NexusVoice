//! ModelPicker.

use gpui_kit::{div, prelude::*, Context, Window};

pub struct ModelPicker;

impl ModelPicker {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for ModelPicker {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
