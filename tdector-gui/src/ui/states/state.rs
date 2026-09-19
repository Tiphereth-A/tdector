use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::enums::{AppAction, CommentTarget, FormationType, PinnedPopup, SortMode};
use tdector_app::{SaveToken, Session};
use tdector_eval::AppError;

type AsyncFileResult<T> = Arc<Mutex<Option<Result<T, AppError>>>>;
type PendingTextFile = AsyncFileResult<(u64, String, String)>;
type PendingProjectFile = AsyncFileResult<(u64, String, String, Option<String>)>;
type PendingFontFile = AsyncFileResult<(Vec<u8>, String)>;
type PendingSaveResult = AsyncFileResult<(SaveToken, Option<String>)>;

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

/// Dialog for removing an applied formation rule from a formatted word
#[derive(Debug, Clone)]
pub struct RemoveFormationRuleDialog {
    /// Index of the segment containing the formatted word
    pub sentence_idx: usize,
    /// Index of the token within the segment
    pub word_idx: usize,
    /// Current formatted word text
    pub formatted_word: String,
    /// Root/base word for the formatted chain
    pub base_word: String,
    /// Description of the rule that will be removed
    pub rule_description: String,
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
    /// Shared application session; views only receive immutable project data.
    pub(crate) session: Session,
    /// Font selection is presentation state and is never stored in the project.
    pub(crate) custom_font_name: Option<String>,
    /// Instance-local mirror for the browser's before-unload callback.
    pub(crate) unsaved_changes: Rc<Cell<bool>>,
    /// Avoid repeatedly prompting after a confirmed native window close.
    pub(crate) closing: bool,
    /// Filename of the current project
    pub(crate) project_filename: Option<String>,
    /// Current page being displayed (0-indexed)
    pub(crate) current_page: usize,
    /// Number of segments per page
    pub(crate) page_size: usize,
    /// Pending text content to import (text content, tokenization flag)
    pub(crate) pending_import: Option<(String, String)>,
    /// Result of async text file load operation
    pub(crate) pending_text_file: PendingTextFile,
    /// Result of async project file load operation
    pub(crate) pending_project_file: PendingProjectFile,
    /// Result of async font file load operation
    pub(crate) pending_font_file: PendingFontFile,
    /// Result of async save operation
    pub(crate) pending_save_result: PendingSaveResult,
    pub(crate) pending_export_result: AsyncFileResult<()>,
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
    pub(crate) similar_tokens_popup: Option<(String, Vec<tdector_app::SimilarToken>)>,
    /// Currently open word context menu
    pub(crate) word_menu_popup: Option<(String, usize, usize, egui::Pos2)>,
    /// Currently open segment context menu
    pub(crate) sentence_menu_popup: Option<(usize, egui::Pos2)>,
    /// Word formation rule application dialog
    pub(crate) word_formation_popup: Option<WordFormationDialog>,
    /// Word formation chain inspection dialog
    pub(crate) formatting_chain_popup: Option<FormattingChainDialog>,
    /// Remove formation rule confirmation dialog
    pub(crate) remove_formation_rule_popup: Option<RemoveFormationRuleDialog>,
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
    /// Whether the displayed filter/sort result needs refreshing.
    pub(crate) filter_dirty: bool,
}

impl DecryptionApp {
    pub(crate) fn recalculate_filtered_indices(&mut self) {
        self.cached_filtered_indices = self
            .session
            .filtered_indices(&self.filter_text, self.sort_mode);
    }

    pub(crate) fn compute_similar_segments(&mut self, target_idx: usize) {
        use crate::consts::domain::DEFAULT_SIMILARITY_RESULTS;
        match self
            .session
            .similar_segments(target_idx, DEFAULT_SIMILARITY_RESULTS)
        {
            Ok(scores) => self.similar_popup = Some((target_idx, scores)),
            Err(error) => {
                self.similar_popup = None;
                self.error_message = Some(error.to_string());
            }
        }
    }
}

impl Default for DecryptionApp {
    /// Create a new default app state with empty project and default UI settings
    fn default() -> Self {
        Self {
            session: Session::default(),
            custom_font_name: None,
            unsaved_changes: Rc::new(Cell::new(false)),
            closing: false,
            project_filename: None,
            current_page: 0,
            page_size: 10,
            pending_import: None,
            pending_text_file: Arc::new(Mutex::new(None)),
            pending_project_file: Arc::new(Mutex::new(None)),
            pending_font_file: Arc::new(Mutex::new(None)),
            pending_save_result: Arc::new(Mutex::new(None)),
            pending_export_result: Arc::new(Mutex::new(None)),
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
            remove_formation_rule_popup: None,
            new_formation_rule_popup: None,
            update_comment_popup: None,
            update_sentence_comment_popup: None,
            custom_tokenization_popup: None,
            pinned_popups: Vec::new(),
            next_popup_id: 0,
            cached_filtered_indices: Vec::new(),
            filter_dirty: false,
        }
    }
}
