//! UI layout and styling constants.
//!
//! This module centralizes all magic numbers used for UI layout, sizing,
//! and styling to ensure consistency across the application and simplify
//! future adjustments to the visual design.

/// Default width for popup windows.
pub const POPUP_WIDTH: f32 = 400.0;

/// Default height for definition popups.
pub const POPUP_DEFINITION_HEIGHT: f32 = 120.0;

/// Default height for reference (usage) popups.
pub const POPUP_REFERENCE_HEIGHT: f32 = 500.0;

/// Default height for similarity search popups.
pub const POPUP_SIMILAR_HEIGHT: f32 = 480.0;

/// Default window width.
pub const WINDOW_WIDTH: f32 = 1024.0;
/// Default window height.
pub const WINDOW_HEIGHT: f32 = 768.0;

/// Horizontal spacing between tokens in pixels.
pub const TOKEN_SPACING_X: f32 = 4.0;
/// Vertical spacing between token rows in pixels.
pub const TOKEN_SPACING_Y: f32 = 8.0;
/// Font size for gloss (translation) text above tokens.
pub const GLOSS_FONT_SIZE: f32 = 12.0;
/// Font size for the original token text.
pub const TOKEN_FONT_SIZE: f32 = 20.0;
/// Horizontal spacing between segments in pixels.
pub const SEGMENT_SPACING_X: f32 = 10.0;
/// Vertical spacing between segment components.
pub const SEGMENT_VERTICAL_SPACING: f32 = 5.0;

/// Extra width added to gloss boxes beyond text width for padding.
pub const GLOSS_BOX_EXTRA_WIDTH: f32 = 10.0;
/// Minimum width for gloss boxes to maintain visual consistency.
pub const GLOSS_BOX_MIN_WIDTH: f32 = 40.0;
/// Additional layout space allocated for gloss box rendering.
pub const GLOSS_BOX_LAYOUT_EXTRA: f32 = 8.0;

/// Stroke width for gloss box borders.
pub const BOX_STROKE_WIDTH: f32 = 1.5;
/// Inner margin (padding) for gloss boxes.
pub const GLOSS_BOX_INNER_MARGIN: f32 = 2.0;
/// Corner rounding radius for gloss boxes.
pub const GLOSS_BOX_ROUNDING: f32 = 2.0;

/// Stroke width for translation box borders.
pub const TRANSLATION_BOX_STROKE_WIDTH: f32 = 1.5;
/// Inner margin (padding) for translation boxes.
pub const TRANSLATION_BOX_INNER_MARGIN: f32 = 4.0;
/// Corner rounding radius for translation boxes.
pub const TRANSLATION_BOX_ROUNDING: f32 = 2.0;
/// Default number of rows in translation text boxes.
pub const TRANSLATION_BOX_ROWS: usize = 2;

/// Default number of segments to display per page.
pub const PAGINATION_DEFAULT_PAGE_SIZE: usize = 10;
/// Width reserved for pagination controls (deducted from available space).
pub const PAGINATION_NAV_WIDTH_DEDUCTION: f32 = 200.0;
/// Width of individual pagination button items.
pub const PAGINATION_ITEM_WIDTH: f32 = 40.0;
/// Number of page buttons to show adjacent to current page.
pub const PAGINATION_BUTTON_ADJACENT_COUNT: isize = 5;
/// Number of page buttons to show at start/end of pagination.
pub const PAGINATION_BUTTON_SIDE_COUNT: isize = 2;

/// Vertical spacing between major UI panels.
pub const PANEL_SPACING: f32 = 10.0;
