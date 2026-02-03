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
