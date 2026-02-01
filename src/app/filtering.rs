//! Segment filtering and sorting implementation.
//!
//! This module handles the filtering of segments based on user search queries
//! and applies various sorting strategies. All operations are designed to work
//! efficiently with cached indices to minimize recomputation.

use super::actions::SortMode;
use super::state::DecryptionApp;

/// Performs case-insensitive substring matching with a pre-lowercased needle.
///
/// # Performance
///
/// Avoids repeated lowercasing of the needle across many calls. The haystack is
/// lowercased once per call and searched with `contains`.
#[inline]
fn contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if needle_lower.len() > haystack.len() {
        return false;
    }

    haystack.to_lowercase().contains(needle_lower)
}

impl DecryptionApp {
    pub(super) fn recalculate_filtered_indices(&mut self) {
        let mut indices: Vec<usize> = if self.filter_text.is_empty() {
            (0..self.project.segments.len()).collect()
        } else {
            let query_lower = self.filter_text.to_lowercase();
            let query = query_lower.as_str();
            self.project
                .segments
                .iter()
                .enumerate()
                .filter(|(_idx, seg)| {
                    contains_ignore_case(&seg.translation, query)
                        || seg
                            .tokens
                            .iter()
                            .any(|t| contains_ignore_case(&t.original, query))
                })
                .map(|(idx, _)| idx)
                .collect()
        };

        if self.sort_mode != SortMode::IndexAsc {
            match self.sort_mode {
                SortMode::IndexDesc => {
                    indices.reverse();
                }
                SortMode::OriginalAsc | SortMode::OriginalDesc => {
                    let mut indexed: Vec<_> = indices
                        .into_iter()
                        .map(|idx| {
                            let key: String = self.project.segments[idx]
                                .tokens
                                .iter()
                                .flat_map(|t| t.original.chars())
                                .collect();
                            (idx, key)
                        })
                        .collect();

                    indexed.sort_by(|a, b| {
                        let cmp = a.1.cmp(&b.1);
                        if self.sort_mode == SortMode::OriginalAsc {
                            cmp
                        } else {
                            cmp.reverse()
                        }
                    });

                    indices = indexed.into_iter().map(|(idx, _)| idx).collect();
                }
                SortMode::LengthAsc | SortMode::LengthDesc => {
                    let mut indexed: Vec<_> = indices
                        .into_iter()
                        .map(|idx| (idx, self.project.segments[idx].tokens.len()))
                        .collect();

                    indexed.sort_by(|a, b| {
                        if self.sort_mode == SortMode::LengthAsc {
                            a.1.cmp(&b.1)
                        } else {
                            b.1.cmp(&a.1)
                        }
                    });

                    indices = indexed.into_iter().map(|(idx, _)| idx).collect();
                }
                SortMode::TranslatedRatioAsc | SortMode::TranslatedRatioDesc => {
                    let mut indexed: Vec<_> = indices
                        .into_iter()
                        .map(|idx| {
                            let seg = &self.project.segments[idx];
                            let ratio = if seg.tokens.is_empty() {
                                0.0
                            } else {
                                let translated = seg
                                    .tokens
                                    .iter()
                                    .filter(|t| {
                                        self.project
                                            .vocabulary
                                            .get(&t.original)
                                            .map(|g| !g.trim().is_empty())
                                            .unwrap_or(false)
                                    })
                                    .count();
                                translated as f64 / seg.tokens.len() as f64
                            };
                            (idx, ratio)
                        })
                        .collect();

                    indexed.sort_by(|a, b| {
                        let cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
                        if self.sort_mode == SortMode::TranslatedRatioAsc {
                            cmp
                        } else {
                            cmp.reverse()
                        }
                    });

                    indices = indexed.into_iter().map(|(idx, _)| idx).collect();
                }
                SortMode::IndexAsc => unreachable!(),
            }
        }

        self.cached_filtered_indices = indices;
    }
}
