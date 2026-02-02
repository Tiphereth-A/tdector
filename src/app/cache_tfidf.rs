//! Incremental TF-IDF computation for efficient similarity search.
//!
//! This module provides an optimized similarity search that avoids
//! full matrix recomputation when only a few segments change.

use ndarray::Array2;
use std::collections::HashSet;

/// Tracks which segments have been modified since last TF-IDF computation.
///
/// Enables incremental updates for better performance on large documents.
pub struct TfidfUpdateTracker {
    /// Set of segment indices that have been modified
    dirty_segments: HashSet<usize>,
    /// Set of segment indices that have been added
    new_segments: HashSet<usize>,
    /// Set of segment indices that have been removed
    removed_segments: HashSet<usize>,
    /// Whether a full recomputation is needed
    needs_full_rebuild: bool,
}

impl TfidfUpdateTracker {
    /// Creates a new tracker with no dirty segments.
    pub fn new() -> Self {
        Self {
            dirty_segments: HashSet::new(),
            new_segments: HashSet::new(),
            removed_segments: HashSet::new(),
            needs_full_rebuild: false,
        }
    }

    /// Marks a segment as modified.
    #[allow(dead_code)]
    pub fn mark_modified(&mut self, idx: usize) {
        self.dirty_segments.insert(idx);
    }

    /// Marks a segment as newly added.
    #[allow(dead_code)]
    pub fn mark_added(&mut self, idx: usize) {
        self.new_segments.insert(idx);
    }

    /// Marks a segment as removed.
    #[allow(dead_code)]
    pub fn mark_removed(&mut self, idx: usize) {
        self.removed_segments.insert(idx);
    }

    /// Forces a full rebuild on next update.
    pub fn mark_full_rebuild(&mut self) {
        self.needs_full_rebuild = true;
    }

    /// Checks if incremental update is possible.
    ///
    /// Returns false if changes are too extensive or complex.
    #[allow(dead_code)]
    pub fn can_use_incremental(&self) -> bool {
        if self.needs_full_rebuild {
            return false;
        }

        let total_changes =
            self.dirty_segments.len() + self.new_segments.len() + self.removed_segments.len();

        total_changes < 50 && self.removed_segments.is_empty()
    }

    /// Returns the set of segments that need recomputation.
    #[allow(dead_code)]
    pub fn get_dirty_segments(&self) -> Vec<usize> {
        self.dirty_segments
            .iter()
            .chain(self.new_segments.iter())
            .copied()
            .collect()
    }

    /// Clears all tracking state after a successful update.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn clear(&mut self) {
        self.dirty_segments.clear();
        self.new_segments.clear();
        self.removed_segments.clear();
        self.needs_full_rebuild = false;
    }

    /// Returns true if any segments are dirty.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_dirty(&self) -> bool {
        !self.dirty_segments.is_empty()
            || !self.new_segments.is_empty()
            || !self.removed_segments.is_empty()
            || self.needs_full_rebuild
    }
}

impl Default for TfidfUpdateTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Cached TF-IDF computation with incremental update support.
pub struct CachedTfidf {
    matrix: Option<Array2<f64>>,
    tracker: TfidfUpdateTracker,
}

impl CachedTfidf {
    pub fn new() -> Self {
        Self {
            matrix: None,
            tracker: TfidfUpdateTracker::new(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_matrix(&self) -> Option<&Array2<f64>> {
        self.matrix.as_ref()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_matrix(&mut self, matrix: Array2<f64>) {
        self.matrix = Some(matrix);
        self.tracker.clear();
    }

    #[allow(dead_code)]
    pub fn mark_segment_modified(&mut self, idx: usize) {
        self.tracker.mark_modified(idx);
    }

    pub fn invalidate(&mut self) {
        self.tracker.mark_full_rebuild();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_dirty(&self) -> bool {
        self.tracker.is_dirty() || self.matrix.is_none()
    }
}

impl Default for CachedTfidf {
    fn default() -> Self {
        Self::new()
    }
}
