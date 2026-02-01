//! Command pattern for decoupling UI actions from business logic.
//!
//! This module implements a command queue system that separates user
//! interactions from their execution, improving testability and
//! enabling features like undo/redo in the future.

use crate::ui::PopupMode;

/// Commands that can be queued and executed on the application state.
///
/// Commands represent atomic state modifications that result from
/// user actions. They encapsulate the "what" without the "how",
/// allowing the application to decide when and how to execute them.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Command {
    // File operations
    ImportTextFile,
    OpenProject,
    SaveProject,
    ExportTypst,
    LoadFont,

    // UI state changes
    SetFilterText(String),
    SetPage(usize),
    SetPageSize(usize),
    SetSortMode(super::actions::SortMode),

    // Popup commands
    OpenDefinitionPopup(String),
    OpenReferencePopup(String),
    OpenSimilarPopup(usize),
    OpenWordMenuPopup(String, usize, usize, eframe::egui::Pos2),
    PinPopup(PinnedPopupData),
    CloseAllPopups,

    // Data modifications
    UpdateSegmentTranslation(usize, String),
    UpdateVocabulary(String, String),
    UpdateVocabularyComment(String, String),

    // Cache invalidation
    InvalidateFilterCache,
    InvalidateLookupsCache,
    InvalidateTfidfCache,
    InvalidateAllCaches,

    // Application lifecycle
    MarkDirty,
    MarkClean,
    RequestQuit,
}

/// Data for pinning a popup window
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PinnedPopupData {
    Dictionary(String, PopupMode, String),
    Similar(usize, Vec<(usize, f64)>, String),
}

/// Command queue for batching and sequencing operations.
///
/// Provides a simple queue-based execution model that can be extended
/// with priority ordering, validation, or undo/redo capabilities.
#[allow(dead_code)]
pub struct CommandQueue {
    commands: Vec<Command>,
}

impl CommandQueue {
    /// Creates a new empty command queue.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Adds a command to the end of the queue.
    #[allow(dead_code)]
    pub fn push(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// Adds multiple commands to the queue.
    #[allow(dead_code)]
    pub fn extend(&mut self, commands: impl IntoIterator<Item = Command>) {
        self.commands.extend(commands);
    }

    /// Removes and returns the next command, or None if empty.
    #[allow(dead_code)]
    pub fn pop(&mut self) -> Option<Command> {
        if self.commands.is_empty() {
            None
        } else {
            Some(self.commands.remove(0))
        }
    }

    /// Returns the number of pending commands.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Checks if the queue is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Clears all pending commands.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.commands.clear();
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}
