//! UI action types and enumerations.

use std::sync::Arc;

/// User interaction result from UI components.
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum UiAction {
    None,
    Changed,
    Filter(Arc<str>),
    ShowSimilar(usize),
    /// Show dictionary definition for a word.
    ShowDefinition(Arc<str>),
    /// Show reference sentences containing a word.
    ShowReference(Arc<str>),
    /// Show context menu for a sentence (right-click).
    ShowSentenceMenu(usize), // segment index
    /// Show context menu for a word (right-click).
    ShowWordMenu(Arc<str>, usize), // word, word_idx_in_segment
}

/// Dictionary popup display mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupMode {
    /// Show segments where the word is the first token.
    Definition,
    /// Show all segments containing the word.
    Reference,
}
