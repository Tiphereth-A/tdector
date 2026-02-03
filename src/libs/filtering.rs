//! Domain filtering and search operations.
//!
//! Provides business logic for segment filtering and sorting:
//! - **Case-insensitive search** across segment text and metadata
//! - **Flexible sorting**: By index, segment text, translation, headword count
//! - **Cached filtering**: Works with pre-computed indices for efficiency

use crate::libs::models::Project;

/// Represents a filtering and sorting operation on a project's segments.
pub struct FilterOperation;

impl FilterOperation {
    /// Performs case-insensitive substring matching.
    ///
    /// # Arguments
    ///
    /// * `haystack` - The text to search in
    /// * `needle_lower` - The search query (should already be lowercased)
    ///
    /// # Performance
    ///
    /// Avoids repeated lowercasing of the needle across multiple calls.
    #[inline]
    pub fn contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
        if needle_lower.is_empty() {
            return true;
        }
        if needle_lower.len() > haystack.len() {
            return false;
        }

        haystack.to_lowercase().contains(needle_lower)
    }

    /// Filters segments by a search query.
    ///
    /// Returns a vector of segment indices that match the filter criteria.
    ///
    /// # Arguments
    ///
    /// * `project` - The project containing segments to filter
    /// * `query` - The search query (empty string matches all)
    ///
    /// # Returns
    ///
    /// Vector of filtered segment indices
    pub fn apply_filter(project: &Project, query: &str) -> Vec<usize> {
        if query.is_empty() {
            (0..project.segments.len()).collect()
        } else {
            let query_lower = query.to_lowercase();
            project
                .segments
                .iter()
                .enumerate()
                .filter(|(_idx, seg)| {
                    Self::contains_ignore_case(&seg.translation, &query_lower)
                        || seg
                            .tokens
                            .iter()
                            .any(|t| Self::contains_ignore_case(&t.original, &query_lower))
                })
                .map(|(idx, _)| idx)
                .collect()
        }
    }
}
