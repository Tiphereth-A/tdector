//! Application actions and sorting configuration.
//!
//! This module defines the various user actions that can be triggered through
//! the UI (menu items, keyboard shortcuts, etc.) and the different sorting
//! modes available for segment lists.

use crate::ui::PopupMode;

/// High-level application actions triggered by menu items or keyboard shortcuts.
///
/// These actions represent user intentions that may require confirmation
/// dialogs (e.g., if there are unsaved changes) before execution.
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

/// Available sorting modes for the segment list.
///
/// Segments can be sorted by index (original order), original text content,
/// token count, or translation completion ratio.
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
    /// Returns all available sort modes in their display order.
    ///
    /// This method provides the canonical ordering for UI selection widgets.
    #[must_use]
    pub const fn all() -> [Self; 8] {
        [
            Self::IndexAsc,
            Self::IndexDesc,
            Self::OriginalAsc,
            Self::OriginalDesc,
            Self::LengthAsc,
            Self::LengthDesc,
            Self::TranslatedRatioAsc,
            Self::TranslatedRatioDesc,
        ]
    }

    /// Returns the display text for this sort mode.
    #[must_use]
    pub fn display_text(self) -> &'static str {
        match self {
            Self::IndexAsc => "Index (Asc)",
            Self::IndexDesc => "Index (Desc)",
            Self::OriginalAsc => "Original (Asc)",
            Self::OriginalDesc => "Original (Desc)",
            Self::LengthAsc => "Length (Shortest First)",
            Self::LengthDesc => "Length (Longest First)",
            Self::TranslatedRatioAsc => "Translated Ratio (Asc)",
            Self::TranslatedRatioDesc => "Translated Ratio (Desc)",
        }
    }
}

/// Popup window types that can be pinned to remain open independently.
///
/// Pinned popups allow users to keep multiple reference windows open simultaneously,
/// which is useful for comparing segments or referring to multiple dictionary entries.
/// Each variant caches its title string to avoid repeated allocations during rendering.
#[derive(Debug, Clone)]
pub enum PinnedPopup {
    /// Similar segments popup: (`target_idx`, `similarity_scores`, `popup_id`, `cached_title`)
    Similar(usize, Vec<(usize, f64)>, u64, String),
    /// Dictionary popup: (word, mode, `popup_id`, `cached_title`)
    Dictionary(String, PopupMode, u64, String),
}
