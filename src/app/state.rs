//! Application state management.
//!
//! This module defines the main [`DecryptionApp`] struct which holds all application
//! state including project data, UI state, caches, and dirty flags for invalidation.

use std::collections::HashMap;
use std::path::PathBuf;

use ndarray::Array2;

use crate::models::Project;

use super::actions::{AppAction, PinnedPopup, SortMode};
use crate::ui::PopupMode;

/// Request to open a specific type of popup window.
///
/// Used to decouple popup triggering from popup rendering, allowing the
/// central panel to request popup display without borrowing state mutably.
pub enum PopupRequest {
    Dictionary(String, PopupMode),
    Similar(usize),
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
/// The structure uses dirty flags to minimize recomputation: when data changes,
/// the relevant dirty flag is set, and caches are regenerated lazily on next access.
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
    pub(super) dictionary_mode: bool,

    pub(super) definition_popup: Option<String>,
    pub(super) reference_popup: Option<String>,
    pub(super) similar_popup: Option<(usize, Vec<(usize, f64)>)>,
    pub(super) pinned_popups: Vec<PinnedPopup>,
    pub(super) next_popup_id: u64,

    pub(super) cached_filtered_indices: Vec<usize>,
    pub(super) cached_headword_lookup: Option<HashMap<String, Vec<usize>>>,
    pub(super) cached_usage_lookup: Option<HashMap<String, Vec<usize>>>,
    pub(super) cached_tfidf_matrix: Option<Array2<f64>>,

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
            dictionary_mode: false,
            definition_popup: None,
            reference_popup: None,
            similar_popup: None,
            pinned_popups: Vec::new(),
            next_popup_id: 0,
            cached_filtered_indices: Vec::new(),
            cached_headword_lookup: None,
            cached_usage_lookup: None,
            cached_tfidf_matrix: None,
            filter_dirty: false,
            lookups_dirty: false,
            tfidf_dirty: false,
        }
    }
}
