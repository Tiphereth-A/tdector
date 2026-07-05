#[cfg(not(target_arch = "wasm32"))]
pub const WINDOW_WIDTH: f32 = 1024.0;

#[cfg(not(target_arch = "wasm32"))]
pub const WINDOW_HEIGHT: f32 = 768.0;

pub const POPUP_WIDTH: f32 = 400.0;

pub const POPUP_DEFINITION_HEIGHT: f32 = 120.0;

pub const POPUP_REFERENCE_HEIGHT: f32 = 500.0;

pub const POPUP_SIMILAR_HEIGHT: f32 = 480.0;

pub const TOKEN_FONT_SIZE: f32 = 20.0;

pub const GLOSS_FONT_SIZE: f32 = 12.0;

pub const TOKEN_SPACING_X: f32 = 4.0;

pub const TOKEN_SPACING_Y: f32 = 8.0;

pub const SEGMENT_SPACING_X: f32 = 10.0;

pub const SEGMENT_VERTICAL_SPACING: f32 = 5.0;

pub const GLOSS_BOX_EXTRA_WIDTH: f32 = 10.0;

pub const GLOSS_BOX_MIN_WIDTH: f32 = 40.0;

pub const GLOSS_BOX_LAYOUT_EXTRA: f32 = 8.0;

pub const BOX_STROKE_WIDTH: f32 = 1.5;

pub const GLOSS_BOX_INNER_MARGIN: f32 = 2.0;

pub const GLOSS_BOX_ROUNDING: f32 = 2.0;

pub const TRANSLATION_BOX_STROKE_WIDTH: f32 = 1.5;

pub const TRANSLATION_BOX_INNER_MARGIN: f32 = 4.0;

pub const TRANSLATION_BOX_ROUNDING: f32 = 2.0;

pub const TRANSLATION_BOX_ROWS: usize = 2;

pub const WORD_FORMATION_SCRIPT_ROWS: usize = 10;

pub const PAGINATION_SIZE_SMALL: usize = 10;

pub const PAGINATION_SIZE_MEDIUM: usize = 20;

pub const PAGINATION_SIZE_LARGE: usize = 50;

pub const PAGINATION_SIZE_EXTRA_LARGE: usize = 100;

pub const PAGINATION_DEFAULT_PAGE_SIZE: usize = PAGINATION_SIZE_SMALL;

pub const PAGINATION_NAV_WIDTH_DEDUCTION: f32 = 200.0;

pub const PAGINATION_ITEM_WIDTH: f32 = 40.0;

pub const PAGINATION_BUTTON_ADJACENT_COUNT: isize = 5;

pub const PAGINATION_BUTTON_SIDE_COUNT: isize = 2;

pub const PAGINATION_DRAG_SPEED: f64 = 0.1;

pub const PANEL_SPACING: f32 = 10.0;
