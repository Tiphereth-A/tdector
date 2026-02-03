//! Main application state container and lifecycle.
//!
//! The [`DecryptionApp`] struct consolidates all application state:
//! - **Project data**: Segments, vocabulary, comments, translation rules
//! - **UI state**: Current page, filters, sort mode, popup windows
//! - **Caches**: Filtered indices, lookup maps, TF-IDF matrix (with dirty flags)
//! - **I/O state**: Pending file imports and UI dialogs

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::enums::{AppAction, FormationType, PinnedPopup, SortMode};
use crate::libs::{
    Project,
    cache::{CachedTfidf, LookupCache},
};

/// Dialog state for creating a new word formation rule.
#[derive(Debug, Clone)]
pub struct NewFormationRuleDialog {
    /// Human-readable description of the rule
    pub description: String,
    /// Type of formation rule
    pub rule_type: FormationType,
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

/// Main application state container.
///
/// Holds all persistent and transient application state:
/// - **project**: Current document with segments, vocabulary, and metadata
/// - **`current_path`**: File path of loaded project (None if new/unsaved)
/// - **`current_page` & `page_size`**: Pagination state for segment list
/// - **`filter_text` & `sort_mode`**: Active filters and sort order
/// - **`cached_filtered_indices`**: Pre-computed filtered segment indices (cached)
/// - **`lookup_cache` & `tfidf_cache`**: Expensive computation caches
/// - **`filter_dirty`, `lookups_dirty`, `tfidf_dirty`**: Cache invalidation flags
/// - **popup_* fields**: Open popup windows and their state
/// - **`pending_import`**: Deferred file import awaiting user confirmation
///
/// ## Interaction Model
/// Dictionary mode is always active: right-click selects words for definition lookup,
/// left-click applies vocabulary filters.
pub struct DecryptionApp {
    pub(crate) project: Project,
    pub(crate) current_path: Option<PathBuf>,
    pub(crate) project_filename: Option<String>,
    pub(crate) current_page: usize,
    pub(crate) page_size: usize,
    pub(crate) is_dirty: bool,
    pub(crate) pending_import: Option<(String, String)>,
    pub(crate) pending_text_file: Arc<Mutex<Option<Result<(String, String), String>>>>,
    pub(crate) pending_project_file: Arc<Mutex<Option<Result<(String, String, Option<String>), String>>>>,
    pub(crate) pending_font_file: Arc<Mutex<Option<Result<(Vec<u8>, String), String>>>>,
    pub(crate) filter_text: String,
    pub(crate) sort_mode: SortMode,
    pub(crate) error_message: Option<String>,
    pub(crate) confirmation: Option<(String, AppAction)>,

    pub(crate) definition_popup: Option<String>,
    pub(crate) reference_popup: Option<String>,
    pub(crate) similar_popup: Option<(usize, Vec<(usize, f64)>)>,
    pub(crate) word_menu_popup: Option<(String, usize, usize, egui::Pos2)>,
    pub(crate) sentence_menu_popup: Option<(usize, egui::Pos2)>,
    pub(crate) word_formation_popup: Option<WordFormationDialog>,
    pub(crate) new_formation_rule_popup: Option<NewFormationRuleDialog>,
    pub(crate) update_comment_popup: Option<UpdateCommentDialog>,
    pub(crate) update_sentence_comment_popup: Option<UpdateSentenceCommentDialog>,
    pub(crate) pinned_popups: Vec<PinnedPopup>,
    pub(crate) next_popup_id: u64,

    pub(crate) cached_filtered_indices: Vec<usize>,
    pub(crate) lookup_cache: LookupCache,
    pub(crate) tfidf_cache: CachedTfidf,

    pub(crate) filter_dirty: bool,
    pub(crate) lookups_dirty: bool,
    pub(crate) tfidf_dirty: bool,
}

impl DecryptionApp {
    /// Recalculate filtered and sorted segment indices.
    /// Delegates to the domain layer's filtering and sorting engines.
    pub(crate) fn recalculate_filtered_indices(&mut self) {
        use crate::libs::filtering::FilterOperation;
        use crate::libs::sorting::SortOperation;

        let mut indices = FilterOperation::apply_filter(&self.project, &self.filter_text);
        SortOperation::apply_sort(&self.project, &mut indices, self.sort_mode);
        self.cached_filtered_indices = indices;
    }

    /// Ensures the TF-IDF matrix cache is up-to-date.
    /// Delegates to the domain layer's similarity engine.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn ensure_tfidf_cache_impl(&mut self) {
        use crate::libs::similarity::SimilarityEngine;

        if !self.tfidf_dirty && !self.tfidf_cache.is_dirty() {
            return;
        }

        if self.project.segments.is_empty() {
            self.tfidf_cache.invalidate();
            self.tfidf_dirty = false;
            return;
        }

        if let Some(matrix) = SimilarityEngine::compute_tfidf_matrix(&self.project) {
            self.tfidf_cache.set_matrix(matrix);
        }
        self.tfidf_dirty = false;
    }

    /// Computes the most similar segments to the target segment.
    /// Delegates to the domain layer's similarity engine.
    /// Returns the top N most similar segments (N from domain constants).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn compute_similar_segments(&mut self, target_idx: usize) {
        use crate::consts::domain::DEFAULT_SIMILARITY_RESULTS;
        use crate::libs::similarity::SimilarityEngine;

        if target_idx >= self.project.segments.len() {
            return;
        }

        self.ensure_tfidf_cache_impl();

        let matrix = match self.tfidf_cache.get_matrix() {
            Some(m) => m,
            None => return,
        };

        let similarities =
            SimilarityEngine::find_similar(matrix, target_idx, DEFAULT_SIMILARITY_RESULTS);

        // Convert f32 scores to f64 for UI compatibility
        let scores: Vec<(usize, f64)> = similarities.into_iter().collect();

        self.similar_popup = Some((target_idx, scores));
    }

    /// WASM version: Similarity search not supported due to upstream SIMD dependency.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn compute_similar_segments(&mut self, _target_idx: usize) {
        self.error_message = Some(
            "Similarity search is not yet supported in the web version. \
            This feature requires the SciRS2 library which depends on SIMD instructions. \
            Support will be added once SciRS2 v0.3.0 is released with WASM compatibility. \
            Please use the desktop application for similarity features."
                .to_string(),
        );
    }
}

impl Default for DecryptionApp {
    fn default() -> Self {
        Self {
            project: Project::default(),
            current_path: None,
            project_filename: None,
            current_page: 0,
            page_size: 10,
            is_dirty: false,
            pending_import: None,
            pending_text_file: Arc::new(Mutex::new(None)),
            pending_project_file: Arc::new(Mutex::new(None)),
            pending_font_file: Arc::new(Mutex::new(None)),
            filter_text: String::new(),
            sort_mode: SortMode::DEFAULT,
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
            filter_dirty: false,
            lookups_dirty: false,
            tfidf_dirty: false,
        }
    }
}
