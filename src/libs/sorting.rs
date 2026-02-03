//! Domain sorting operations.
//!
//! Provides business logic for segment sorting:
//! - **Flexible sorting**: By index, segment text, translation ratio, token count
//! - **Unified direction handling**: All sort functions use ascending order with optional reversal

use crate::enums::{SortDirection, SortField, SortMode};
use crate::libs::models::Project;

/// Represents sorting operations on project segments.
pub struct SortOperation;

impl SortOperation {
    /// Applies a sorting mode to a set of segment indices.
    pub fn apply_sort(project: &Project, indices: &mut [usize], sort_mode: SortMode) {
        use SortField::*;

        match sort_mode.field {
            Index => {
                // Already in ascending order
            }
            Original => {
                Self::sort_by_original_text(project, indices);
            }
            Length => {
                Self::sort_by_length(project, indices);
            }
            Count => {
                Self::sort_by_count(project, indices);
            }
            TranslatedRatio => {
                Self::sort_by_translation_ratio(project, indices);
            }
            TranslatedCount => {
                Self::sort_by_translated_count(project, indices);
            }
        }

        // Handle descending sort by reversing
        if sort_mode.direction == SortDirection::Descending {
            indices.reverse();
        }
    }

    /// Sorts segments by original text content (ascending).
    fn sort_by_original_text(project: &Project, indices: &mut [usize]) {
        let mut indexed: Vec<_> = indices
            .iter()
            .map(|&idx| {
                let key: String = project.segments[idx]
                    .tokens
                    .iter()
                    .flat_map(|t| t.original.chars())
                    .collect();
                (idx, key)
            })
            .collect();

        indexed.sort_by(|a, b| a.1.cmp(&b.1));

        for (i, (idx, _)) in indexed.into_iter().enumerate() {
            indices[i] = idx;
        }
    }

    /// Sorts segments by token count (ascending).
    fn sort_by_length(project: &Project, indices: &mut [usize]) {
        let mut indexed: Vec<_> = indices
            .iter()
            .map(|&idx| (idx, project.segments[idx].tokens.len()))
            .collect();

        indexed.sort_by_key(|&(_, len)| len);

        for (i, (idx, _)) in indexed.into_iter().enumerate() {
            indices[i] = idx;
        }
    }

    /// Sorts segments by token count (same as length, ascending).
    fn sort_by_count(project: &Project, indices: &mut [usize]) {
        let mut indexed: Vec<_> = indices
            .iter()
            .map(|&idx| (idx, project.segments[idx].tokens.len()))
            .collect();

        indexed.sort_by_key(|&(_, count)| count);

        for (i, (idx, _)) in indexed.into_iter().enumerate() {
            indices[i] = idx;
        }
    }

    /// Sorts segments by translation completion ratio (ascending).
    fn sort_by_translation_ratio(project: &Project, indices: &mut [usize]) {
        use super::text_analysis::TextProcessor;

        let mut indexed: Vec<_> = indices
            .iter()
            .map(|&idx| {
                let ratio = project
                    .segments
                    .get(idx)
                    .map(TextProcessor::calculate_translation_ratio)
                    .unwrap_or(0.0);
                (idx, ratio)
            })
            .collect();

        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        for (i, (idx, _)) in indexed.into_iter().enumerate() {
            indices[i] = idx;
        }
    }

    /// Sorts segments by the number of tokens with vocabulary entries (ascending).
    fn sort_by_translated_count(project: &Project, indices: &mut [usize]) {
        use super::text_analysis::TextProcessor;

        let mut indexed: Vec<_> = indices
            .iter()
            .map(|&idx| {
                let count = project
                    .segments
                    .get(idx)
                    .map(|seg| TextProcessor::count_segment_translated_tokens(seg, project))
                    .unwrap_or(0);
                (idx, count)
            })
            .collect();

        indexed.sort_by_key(|&(_, count)| count);

        for (i, (idx, _)) in indexed.into_iter().enumerate() {
            indices[i] = idx;
        }
    }
}
