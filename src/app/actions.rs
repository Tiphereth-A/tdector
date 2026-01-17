//! Action enums and sorting configuration.

use crate::ui::PopupMode;

/// Menu/keyboard actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    /// Import a text file.
    Import,
    /// Open a project file.
    Open,
    /// Export to Typst.
    Export,
    /// Quit the application.
    Quit,
}

/// Segment list sorting modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    IndexAsc,
    IndexDesc,
    OriginalAsc,
    OriginalDesc,
    LengthAsc,
    LengthDesc,
    TranslatedRatioAsc,
    TranslatedRatioDesc,
}

impl SortMode {
    /// Returns the display text for this sort mode.
    #[must_use]
    pub fn display_text(self) -> &'static str {
        match self {
            SortMode::IndexAsc => "Index (Asc)",
            SortMode::IndexDesc => "Index (Desc)",
            SortMode::OriginalAsc => "Original (Asc)",
            SortMode::OriginalDesc => "Original (Desc)",
            SortMode::LengthAsc => "Length (Shortest First)",
            SortMode::LengthDesc => "Length (Longest First)",
            SortMode::TranslatedRatioAsc => "Translated Ratio (Asc)",
            SortMode::TranslatedRatioDesc => "Translated Ratio (Desc)",
        }
    }
}

/// Popup types that can be pinned (kept open independently).
#[derive(Debug)]
pub enum PinnedPopup {
    Similar(usize, Vec<(usize, f64)>, u64),
    Dictionary(String, PopupMode, u64),
}
