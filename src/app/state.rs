//! Application state management.
//!
//! This module defines the main [`DecryptionApp`] struct which holds all application
//! state including project data, UI state, caches, and dirty flags for invalidation.

use std::path::PathBuf;

use eframe::egui;

use crate::models::Project;

use super::actions::{AppAction, PinnedPopup, SortMode};
use super::cache_lookup::LookupCache;
use super::cache_tfidf::CachedTfidf;
use super::commands::CommandQueue;
use crate::ui::PopupMode;

/// Dialog state for creating a new word formation rule.
#[derive(Debug, Clone)]
pub struct NewFormationRuleDialog {
    /// Human-readable description of the rule
    pub description: String,
    /// Type of formation rule
    pub rule_type: crate::models::FormationType,
    /// Rhai script command to apply the transformation
    pub command: String,
    /// Test word for previewing the transformation
    pub test_word: String,
    /// Preview of the transformation result
    pub preview: String,
}

/// Dialog state for applying word formation rules.
#[derive(Debug, Clone)]
pub struct WordFormationDialog {
    /// Index of the sentence containing the word
    pub sentence_idx: usize,
    /// Index of the word in the sentence
    pub word_idx: usize,
    /// The selected word to apply rule to
    pub selected_word: String,
    /// Base word to transform (editable by user)
    pub base_word: String,
    /// Currently previewed transformation result
    pub preview: String,
    /// Index of selected formation rule
    pub selected_rule: Option<usize>,
    /// Top 5 related words from vocabulary for suggestions
    pub related_words: Vec<String>,
    /// Search text for filtering formation rules
    pub rule_search_text: String,
}

/// Dialog state for updating word comments.
#[derive(Debug, Clone)]
pub struct UpdateCommentDialog {
    /// The word being commented on
    pub word: String,
    /// The comment text (editable by user)
    pub comment: String,
}

/// Dialog state for updating sentence comments.
#[derive(Debug, Clone)]
pub struct UpdateSentenceCommentDialog {
    /// Index of the sentence/segment being commented on
    pub segment_idx: usize,
    /// The comment text (editable by user)
    pub comment: String,
}

/// Request to open a specific type of popup window.
///
/// Used to decouple popup triggering from popup rendering, allowing the
/// central panel to request popup display without borrowing state mutably.
pub enum PopupRequest {
    Dictionary(String, PopupMode),
    Similar(usize),
    WordMenu(String, usize, usize, egui::Pos2), // word, sentence_idx, word_idx, cursor_pos
    SentenceMenu(usize, egui::Pos2),            // sentence_idx, cursor_pos
    Filter(String),
}

/// Main application state container.
///
/// This structure holds all application state including:
/// - Project data (segments, vocabulary, settings)
/// - UI state (pagination, filters, sort mode)
/// - Cached computations (filtered indices, lookup maps, TF-IDF matrix)
/// - Dirty flags for cache invalidation
/// - Dialog and popup state
///
/// Dictionary mode is always enabled - right-clicking words shows dictionary options,
/// while left-clicking applies filters.
pub struct DecryptionApp {
    pub(super) project: Project,
    pub(super) current_path: Option<PathBuf>,
    pub(super) current_page: usize,
    pub(super) page_size: usize,
    pub(super) is_dirty: bool,
    pub(super) pending_import: Option<(String, String, Option<String>)>,
    pub(super) filter_text: String,
    pub(super) sort_mode: SortMode,
    pub(super) error_message: Option<String>,
    pub(super) confirmation: Option<(String, AppAction)>,

    pub(super) definition_popup: Option<String>,
    pub(super) reference_popup: Option<String>,
    pub(super) similar_popup: Option<(usize, Vec<(usize, f64)>)>,
    pub(super) word_menu_popup: Option<(String, usize, usize, egui::Pos2)>, // word, sentence_idx, word_idx, cursor_pos
    pub(super) sentence_menu_popup: Option<(usize, egui::Pos2)>, // sentence_idx, cursor_pos
    pub(super) word_formation_popup: Option<WordFormationDialog>,
    pub(super) new_formation_rule_popup: Option<NewFormationRuleDialog>,
    pub(super) update_comment_popup: Option<UpdateCommentDialog>,
    pub(super) update_sentence_comment_popup: Option<UpdateSentenceCommentDialog>,
    pub(super) pinned_popups: Vec<PinnedPopup>,
    pub(super) next_popup_id: u64,

    pub(super) cached_filtered_indices: Vec<usize>,
    pub(super) lookup_cache: LookupCache,
    pub(super) tfidf_cache: CachedTfidf,
    #[allow(dead_code)]
    pub(super) command_queue: CommandQueue,

    pub(super) filter_dirty: bool,
    pub(super) lookups_dirty: bool,
    pub(super) tfidf_dirty: bool,
}

impl Default for DecryptionApp {
    fn default() -> Self {
        Self {
            project: Project::default(),
            current_path: None,
            current_page: 0,
            page_size: 10,
            is_dirty: false,
            pending_import: None,
            filter_text: String::new(),
            sort_mode: SortMode::IndexAsc,
            error_message: None,
            confirmation: None,
            definition_popup: None,
            reference_popup: None,
            similar_popup: None,
            word_menu_popup: None,
            sentence_menu_popup: None,
            word_formation_popup: None,
            new_formation_rule_popup: None,
            update_comment_popup: None,
            update_sentence_comment_popup: None,
            pinned_popups: Vec::new(),
            next_popup_id: 0,
            cached_filtered_indices: Vec::new(),
            lookup_cache: LookupCache::default(),
            tfidf_cache: CachedTfidf::default(),
            command_queue: CommandQueue::default(),
            filter_dirty: false,
            lookups_dirty: false,
            tfidf_dirty: false,
        }
    }
}
