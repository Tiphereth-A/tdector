//! Presentation layer: User interface components and egui rendering.
//!
//! All UI rendering logic using the egui immediate-mode GUI framework:
//!
//! - **dialogs**: Native file pickers and system dialogs for I/O operations
//! - **panels**: Main application panels (vocabulary browser, segment list, status bar)
//! - **menu**: Application menu bar with File, Edit, View, and Tools menus
//! - **pagination**: Controls for segment list navigation and page size configuration
//! - **segment**: Segment rendering with token-level styling and interactions
//! - **popups**: Modal windows (definitions, references, comments, etc.)
//! - **constants**: Layout dimensions, padding, and UI configuration
//! - **highlight**: Text styling (colors, fonts) and visual emphasis
//! - **popup_utils**: Shared utilities for popup window management
//! - **types**: UI-specific types and configurations (moved to enums module)
//!
//! Note: Color palette has been moved to the `consts` module.

pub(crate) mod dialogs;
pub(crate) mod highlight;
mod menu;
mod pagination;
pub(crate) mod panels;
pub mod popup_utils;
pub(crate) mod popups;
mod segment;
pub(crate) mod states;

pub use menu::render_menu_bar;
pub use pagination::render_pagination;
pub use segment::{render_clickable_tokens, render_segment};
pub use states::DecryptionApp;
