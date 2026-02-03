use std::sync::Arc;

/// User interaction result from UI components.
#[derive(Debug, PartialEq, Eq)]
pub enum UiAction {
    None,
    Changed,
    Filter(Arc<str>),
    #[allow(dead_code)]
    ShowSimilar(usize),
    /// Show dictionary definition for a word.
    #[allow(dead_code)]
    ShowDefinition(Arc<str>),
    /// Show reference sentences containing a word.
    #[allow(dead_code)]
    ShowReference(Arc<str>),
    /// Show context menu for a sentence (right-click).
    ShowSentenceMenu(usize),
    /// Show context menu for a word (right-click).
    ShowWordMenu(Arc<str>, usize),
}
