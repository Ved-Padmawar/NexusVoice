//! Building blocks shared by every page, so one control looks and behaves the
//! same everywhere. Page-specific pieces live with their page.

mod dropdown;
mod feedback;
mod modal;
mod page;
mod segmented;
mod text_input;

pub use dropdown::{Dropdown, DropdownEvent, DropdownItem};
pub use feedback::{skeleton, spinner, LoadState, SectionState};
pub use modal::modal;
pub use page::page_header;
pub use segmented::{segmented, Segment};
pub use text_input::text_input;

use gpui_kit::component::{Icon, IconNamed};
use gpui_kit::{px, App, Hsla, Styled as _, Window};

/// A callback a component runs on the user's behalf: retry, dismiss.
pub type Action = Box<dyn Fn(&mut Window, &mut App)>;

/// An icon at `size` px in `color`.
pub fn icon(name: impl IconNamed, size: f32, color: Hsla) -> Icon {
    Icon::new(name).size(px(size)).text_color(color)
}
