use crate::enums::{SortDirection, SortField, SortMode};
use crate::libs::Project;

/// Sorting operations to order segments by various criteria.
pub struct SortOperation;

impl SortOperation {
    /// Sort segment indices in-place according to the specified sort mode.
    /// Handles multiple sort fields (Index, Original text, Length, Count, TranslatedRatio, TranslatedCount)
    /// and sort directions (Ascending/Descending).
    pub fn apply_sort(project: &Project, indices: &mut [usize], sort_mode: SortMode) {
        use SortField::*;

        // Apply the primary sort based on selected field
        match sort_mode.field {
            Index => {} // Index order is default; no sorting needed
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

        // Reverse if descending order requested
        if sort_mode.direction == SortDirection::Descending {
            indices.reverse();
        }
    }

    /// Sort by the concatenated original text of all tokens in each segment (alphabetical)
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

    /// Sort by the number of tokens (words/characters) in each segment
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

    /// Sort by token count (same as length; kept for compatibility)
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

    /// Sort by translation ratio (0.0 if no translation, 1.0 if translated)
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
