use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::enums::{AppAction, CommentTarget, FormationType, PinnedPopup, SortMode};
use crate::libs::{
    Project,
    cache::{CachedTfidf, LookupCache},
};

/// Dialog for creating a new word formation rule
#[derive(Debug, Clone)]
pub struct NewFormationRuleDialog {
    /// Human-readable description of what the rule does
    pub description: String,
    /// Category: Derivation, Inflection, or Non-morphological
    pub rule_type: FormationType,
    /// Rhai script implementing the transformation
    pub command: String,
    /// Test word to preview the rule's effect
    pub test_word: String,
    /// Preview of the rule applied to the test word
    pub preview: String,
}

/// Dialog for applying word formation rules to create a derived form
#[derive(Debug, Clone)]
pub struct WordFormationDialog {
    /// The base word to apply rules to
    pub selected_word: String,
    /// The base word (may differ from `selected_word` for derived forms)
    pub base_word: String,
    /// Preview of the final derived form after all rules are applied
    pub preview: String,
    /// Index of the currently selected rule (if any)
    pub selected_rule: Option<usize>,
    /// Related words created by applying this rule
    pub related_words: Vec<String>,
    /// Search text to filter available rules
    pub rule_search_text: String,
}

/// Dialog for viewing the word formation (formatting) chain
#[derive(Debug, Clone)]
pub struct FormattingChainDialog {
    /// Index of the segment containing the word
    pub sentence_idx: usize,
    /// Index of the word within the segment
    pub word_idx: usize,
}

/// Dialog for editing comments on words
#[derive(Debug, Clone)]
pub struct UpdateCommentDialog {
    /// The word being commented on
    pub word: String,
    /// The comment text
    pub comment: String,
    /// Whether this is a base word or formatted word comment
    pub target: CommentTarget,
}

/// Dialog for editing segment-level comments
#[derive(Debug, Clone)]
pub struct UpdateSentenceCommentDialog {
    /// Index of the segment being commented on
    pub segment_idx: usize,
    /// The comment text
    pub comment: String,
}

/// Dialog for creating a custom tokenization rule during import
#[derive(Debug, Clone)]
pub struct CustomTokenizationDialog {
    /// Pending import data (content, name)
    pub import_data: (String, String),
    /// Rhai script implementing the tokenization logic
    pub command: String,
    /// Test text to preview the rule's effect
    pub test_text: String,
    /// Preview of tokens generated from test text
    pub preview: Vec<String>,
}

/// Main application state for the decryption UI
pub struct DecryptionApp {
    /// The loaded translation project
    pub(crate) project: Project,
    /// Path to the currently open project file (if saved to disk)
    pub(crate) current_path: Option<PathBuf>,
    /// Filename of the current project
    pub(crate) project_filename: Option<String>,
    /// Current page being displayed (0-indexed)
    pub(crate) current_page: usize,
    /// Number of segments per page
    pub(crate) page_size: usize,
    /// Whether the project has unsaved changes
    pub(crate) is_dirty: bool,
    /// Pending text content to import (text content, tokenization flag)
    pub(crate) pending_import: Option<(String, String)>,
    /// Result of async text file load operation
    pub(crate) pending_text_file: Arc<Mutex<Option<Result<(String, String), String>>>>,
    /// Result of async project file load operation
    pub(crate) pending_project_file:
        Arc<Mutex<Option<Result<(String, String, Option<String>), String>>>>,
    /// Result of async font file load operation
    pub(crate) pending_font_file: Arc<Mutex<Option<Result<(Vec<u8>, String), String>>>>,
    /// Result of async save operation
    pub(crate) pending_save_result: Arc<Mutex<Option<Result<(), String>>>>,
    /// Current filter query text
    pub(crate) filter_text: String,
    /// Current sort mode
    pub(crate) sort_mode: SortMode,
    /// Error message to display in error dialog (if any)
    pub(crate) error_message: Option<String>,
    /// Pending confirmation dialog with question and action to confirm
    pub(crate) confirmation: Option<(String, AppAction)>,

    /// Currently open definition popup word
    pub(crate) definition_popup: Option<String>,
    /// Currently open reference popup word
    pub(crate) reference_popup: Option<String>,
    /// Currently open similarity search popup
    pub(crate) similar_popup: Option<(usize, Vec<(usize, f64)>)>,
    /// Currently open similar tokens popup
    pub(crate) similar_tokens_popup:
        Option<(String, Vec<crate::libs::similarity_token::SimilarToken>)>,
    /// Currently open word context menu
    pub(crate) word_menu_popup: Option<(String, usize, usize, egui::Pos2)>,
    /// Currently open segment context menu
    pub(crate) sentence_menu_popup: Option<(usize, egui::Pos2)>,
    /// Word formation rule application dialog
    pub(crate) word_formation_popup: Option<WordFormationDialog>,
    /// Word formation chain inspection dialog
    pub(crate) formatting_chain_popup: Option<FormattingChainDialog>,
    /// New formation rule creation dialog
    pub(crate) new_formation_rule_popup: Option<NewFormationRuleDialog>,
    /// Word comment editing dialog
    pub(crate) update_comment_popup: Option<UpdateCommentDialog>,
    /// Segment comment editing dialog
    pub(crate) update_sentence_comment_popup: Option<UpdateSentenceCommentDialog>,
    /// Custom tokenization rule creation dialog during import
    pub(crate) custom_tokenization_popup: Option<CustomTokenizationDialog>,
    /// Popups pinned to remain visible (not auto-closing)
    pub(crate) pinned_popups: Vec<PinnedPopup>,
    /// Counter for generating unique popup IDs
    pub(crate) next_popup_id: u64,

    /// Cached list of segment indices matching current filter
    pub(crate) cached_filtered_indices: Vec<usize>,
    /// Cache for quick token lookups
    pub(crate) lookup_cache: LookupCache,
    /// Cache for TF-IDF matrix (similarity search)
    pub(crate) tfidf_cache: CachedTfidf,

    /// Whether filtered indices cache needs recalculation
    pub(crate) filter_dirty: bool,
    /// Whether lookup maps need recalculation
    pub(crate) lookups_dirty: bool,
    /// Whether TF-IDF matrix needs recalculation
    pub(crate) tfidf_dirty: bool,
}

impl DecryptionApp {
    /// Recalculate the cached list of segment indices based on current filter and sort settings
    pub(crate) fn recalculate_filtered_indices(&mut self) {
        use crate::libs::filtering::FilterOperation;
        use crate::libs::sorting::SortOperation;

        let mut indices = FilterOperation::apply_filter(&self.project, &self.filter_text);
        SortOperation::apply_sort(&self.project, &mut indices, self.sort_mode);
        self.cached_filtered_indices = indices;
    }

    /// Ensure the TF-IDF matrix cache is up-to-date (native only)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn ensure_tfidf_cache_impl(&mut self) {
        use crate::libs::similarity_sentence::SimilarityEngine;

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

    /// Compute similar segments to a target segment and update the UI
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn compute_similar_segments(&mut self, target_idx: usize) {
        use crate::consts::domain::DEFAULT_SIMILARITY_RESULTS;
        use crate::libs::similarity_sentence::SimilarityEngine;

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

        let scores: Vec<(usize, f64)> = similarities.into_iter().collect();

        self.similar_popup = Some((target_idx, scores));
    }

    /// WASM stub: similarity search not supported in web version
    /// WASM stub: similarity search not supported in web version
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
    /// Create a new default app state with empty project and default UI settings
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
            pending_save_result: Arc::new(Mutex::new(None)),
            filter_text: String::new(),
            sort_mode: SortMode::DEFAULT,
            error_message: None,
            confirmation: None,
            definition_popup: None,
            reference_popup: None,
            similar_popup: None,
            similar_tokens_popup: None,
            word_menu_popup: None,
            sentence_menu_popup: None,
            word_formation_popup: None,
            formatting_chain_popup: None,
            new_formation_rule_popup: None,
            update_comment_popup: None,
            update_sentence_comment_popup: None,
            custom_tokenization_popup: None,
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
