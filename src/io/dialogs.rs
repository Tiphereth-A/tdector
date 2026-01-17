//! Platform-native file picker dialogs.

use std::path::PathBuf;

/// Opens a file picker for text files (`.txt`).
pub fn pick_text_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Text", &["txt"])
        .pick_file()
}

/// Opens a file picker for project files (`.json`).
pub fn pick_project_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()
}

/// Opens a save dialog for project files (`.json`).
pub fn pick_save_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .save_file()
}

/// Opens a save dialog for Typst files (`.typ`).
pub fn pick_typst_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Typst", &["typ"])
        .save_file()
}

/// Opens a file picker for font files (`.ttf`, `.otf`, `.ttc`).
pub fn pick_font_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Font", &["ttf", "otf", "ttc"])
        .pick_file()
}
