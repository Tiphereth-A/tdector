/// High-level application actions triggered by menu commands or keyboard shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    /// Trigger text import dialog to add new content to the project
    Import,

    /// Trigger project open dialog to load a saved project from disk
    Open,

    /// Save the current project to disk
    Export,

    /// Close the application
    Quit,
}
