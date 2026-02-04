use std::sync::Arc;

/// Actions triggered by UI elements that affect application state
#[derive(Debug, PartialEq, Eq)]
pub enum UiAction {
    /// No action occurred
    None,
    /// Some data was modified
    Changed,
    /// Apply text filter to segment list
    Filter(Arc<str>),
    /// Request to show similar segments (desktop only)
    #[allow(dead_code)]
    ShowSimilar(usize),
    /// Request to show word definition popup
    #[allow(dead_code)]
    ShowDefinition(Arc<str>),
    /// Request to show word reference popup
    #[allow(dead_code)]
    ShowReference(Arc<str>),
    /// Show context menu for a segment
    ShowSentenceMenu(usize),
    /// Show context menu for a word (with segment and word indices)
    ShowWordMenu(Arc<str>, usize),
}
