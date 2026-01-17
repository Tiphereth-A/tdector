//! UI action types and enumerations.

/// User interaction result from UI components.
#[derive(Debug, PartialEq, Eq)]
pub enum UiAction {
    None,
    Changed,
    Filter(String),
    ShowSimilar(usize),
    /// Show dictionary definition for a word.
    ShowDefinition(String),
    /// Show reference sentences containing a word.
    ShowReference(String),
}

/// Dictionary popup display mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupMode {
    /// Show segments where the word is the first token.
    Definition,
    /// Show all segments containing the word.
    Reference,
}
