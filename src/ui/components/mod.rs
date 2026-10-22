//! Building blocks shared by every page.

mod button;
mod controls;
mod dropdown;
mod feedback;
mod modal;
mod page;
mod popover;
mod segmented;
mod switch;
mod text_input;

pub use button::{button, icon_button, Button, ButtonKind, ButtonSize};
pub use controls::{pill, progress_bar, Tone};
pub use dropdown::{Dropdown, DropdownEvent, DropdownItem};
pub use feedback::{skeleton, spinner, spinning_icon, LoadState, SectionState};
pub use modal::modal;
pub use page::{page_header, section_heading};
pub use popover::{popover_open, popover_panel};
pub use segmented::{segmented, Segment};
pub use switch::switch;
pub use text_input::text_input;

use gpui_kit::component::{Icon, IconNamed};
use gpui_kit::{px, App, Hsla, Styled as _, Window};

pub type Action = Box<dyn Fn(&mut Window, &mut App)>;

pub fn icon(name: impl IconNamed, size: f32, color: Hsla) -> Icon {
    Icon::new(name).size(px(size)).text_color(color)
}
