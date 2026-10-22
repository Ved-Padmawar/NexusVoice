//! The frameless window's title strip: a drag region across the top with
//! minimize, maximize and close at the right, laid over the content.

use gpui_kit::{
    canvas, div, point, prelude::*, px, rgb, App, Bounds, Hsla, IntoElement, MouseButton,
    PathBuilder, Pixels, Window, WindowControlArea,
};

use gpui_kit::base::InteractiveElementExt as _;

use crate::ui::theme::{Palette, CLOSE_RED};

const HEIGHT: Pixels = px(32.);
const BUTTON_W: Pixels = px(46.);

#[derive(Clone, Copy)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

impl Control {
    fn area(self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Maximize => WindowControlArea::Max,
            Self::Close => WindowControlArea::Close,
        }
    }

    fn act(self, window: &mut Window) {
        match self {
            Self::Minimize => window.minimize_window(),
            Self::Maximize => window.zoom_window(),
            Self::Close => window.remove_window(),
        }
    }

    /// The glyph, drawn as the original 10px vector marks.
    fn paint(self, bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
        let c = bounds.center();
        let at = |x: f32, y: f32| point(c.x + px(x), c.y + px(y));
        match self {
            Self::Minimize => {
                window.paint_quad(gpui_kit::fill(
                    Bounds::new(at(-5., -0.5), gpui_kit::size(px(10.), px(1.))),
                    color,
                ));
            }
            Self::Maximize => {
                window.paint_quad(
                    gpui_kit::outline(
                        Bounds::new(at(-4.5, -4.5), gpui_kit::size(px(9.), px(9.))),
                        color,
                        gpui_kit::BorderStyle::Solid,
                    )
                    .corner_radii(px(0.5)),
                );
            }
            Self::Close => {
                let mut cross = PathBuilder::stroke(px(1.5));
                cross.move_to(at(-4., -4.));
                cross.line_to(at(4., 4.));
                cross.move_to(at(4., -4.));
                cross.line_to(at(-4., 4.));
                if let Ok(path) = cross.build() {
                    window.paint_path(path, color);
                }
            }
        }
    }
}

#[derive(IntoElement)]
pub struct TitleBar;

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = *Palette::global(cx);
        let hovered = window.use_state(cx, |_, _| None::<usize>);
        let current = *hovered.read(cx);

        let button = |index: usize, control: Control| {
            let close = matches!(control, Control::Close);
            let is_hovered = current == Some(index);
            let color = match (is_hovered, close) {
                (true, true) => gpui_kit::white(),
                (true, false) => p.fg,
                (false, _) => p.muted,
            };
            let hovered = hovered.clone();
            div()
                .id(("title-control", index))
                .w(BUTTON_W)
                .h_full()
                .cursor_pointer()
                .when(is_hovered, |el| {
                    el.bg(if close {
                        rgb(CLOSE_RED).into()
                    } else {
                        p.surface_hover
                    })
                })
                .on_hover(move |over, _, cx| {
                    hovered.update(cx, |h, cx| {
                        let next = over.then_some(index);
                        if *h != next {
                            *h = next;
                            cx.notify();
                        }
                    });
                })
                .when(cfg!(target_os = "windows"), |el| {
                    el.window_control_area(control.area())
                })
                .when(!cfg!(target_os = "windows"), |el| {
                    el.on_mouse_down(MouseButton::Left, |_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        control.act(window);
                    })
                })
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, (), window, _| control.paint(bounds, color, window),
                    )
                    .size_full(),
                )
        };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(HEIGHT)
            .flex()
            .child(
                div()
                    .id("title-drag")
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
                    .on_double_click(|_, window, _| window.zoom_window())
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        if !cfg!(target_os = "windows") {
                            window.start_window_move();
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .h_full()
                    .child(button(0, Control::Minimize))
                    .child(button(1, Control::Maximize))
                    .child(button(2, Control::Close)),
            )
    }
}
