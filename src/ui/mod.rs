//! UI components: menu, pagination, and segment rendering.

pub mod colors;
pub mod constants;
pub(crate) mod highlight;
mod menu;
mod pagination;
mod segment;
mod types;

pub use menu::render_menu_bar;
pub use pagination::render_pagination;
pub use segment::{render_clickable_tokens, render_segment};
pub use types::{PopupMode, UiAction};
