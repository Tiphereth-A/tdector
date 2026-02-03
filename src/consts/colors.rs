//! UI color constants.
//!
//! Defines the color palette used throughout the user interface.

use eframe::egui::Color32;

// Box background colors

/// Vocabulary entry glossbox background color (cyan).
pub const GLOSSBOX: Color32 = Color32::from_rgb(0x44, 0xEA, 0xFC);

/// Word formation rule glossbox background color (lime green).
pub const GLOSSBOX_BYFORMATION: Color32 = Color32::from_rgb(0x6A, 0xD8, 0x3F);

/// Sentence/reference box background color (magenta).
pub const SENTENCEBOX: Color32 = Color32::from_rgb(0xFC, 0x44, 0xF9);

// Text colors

/// Light text color for de-emphasized elements.
pub const FONT_LIGHT: Color32 = Color32::from_rgb(0x40, 0x40, 0x40);

/// Dark text color for de-emphasized elements.
pub const FONT_DARK: Color32 = Color32::from_rgb(0xB0, 0xB0, 0xB0);

// Highlight colors

/// Text highlight background color (gold/yellow).
pub const HIGHLIGHT_BG: Color32 = Color32::from_rgb(0xFC, 0xC8, 0x44);

/// Text highlight foreground color (black).
pub const HIGHLIGHT_FG: Color32 = Color32::from_rgb(0, 0, 0);
