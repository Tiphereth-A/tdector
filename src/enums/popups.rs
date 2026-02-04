/// Requests to open different types of popup windows
pub enum PopupRequest {
    /// Show definition or reference popup for a word
    Dictionary(String, DictionaryPopupType),
    /// Show segments similar to a given segment (by index)
    Similar(usize),
    /// Show context menu for a specific word in a segment
    WordMenu(String, usize, usize, egui::Pos2),
    /// Show context menu for a segment
    SentenceMenu(usize, egui::Pos2),
    /// Show word formation (formatting) chain for a word in a segment
    FormattingChain(usize, usize),
    /// Apply a filter query to the segment list
    Filter(String),
}

/// Type of dictionary popup to display
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictionaryPopupType {
    /// Show the definition field
    Definition,
    /// Show the reference field
    Reference,
}

/// A popup that persists across updates (pinned to the screen)
#[derive(Debug, Clone)]
pub enum PinnedPopup {
    /// Pinned similarity search results: (segment_index, similar_segments, popup_id, display_text)
    Similar(usize, Vec<(usize, f64)>, u64, String),
    /// Pinned dictionary popup: (word, popup_type, popup_id, display_text)
    Dictionary(String, DictionaryPopupType, u64, String),
}
