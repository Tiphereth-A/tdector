/// Request to open a specific type of popup window.
///
/// Used to decouple popup triggering from popup rendering, allowing the
/// central panel to request popup display without borrowing state mutably.
pub enum PopupRequest {
    Dictionary(String, DictionaryPopupType),
    Similar(usize),
    WordMenu(String, usize, usize, egui::Pos2),
    SentenceMenu(usize, egui::Pos2),
    Filter(String),
}

/// Dictionary popup display mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictionaryPopupType {
    /// Show segments where the word is the first token.
    Definition,
    /// Show all segments containing the word.
    Reference,
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
    Dictionary(String, DictionaryPopupType, u64, String),
}
