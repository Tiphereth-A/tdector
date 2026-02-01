//! Segment filtering and sorting implementation.
//!
//! This module handles the filtering of segments based on user search queries
//! and applies various sorting strategies. All operations are designed to work
//! efficiently with cached indices to minimize recomputation.

use super::actions::SortMode;
use super::state::DecryptionApp;

/// Performs case-insensitive substring matching without allocations.
///
/// Uses character-by-character comparison with Unicode normalization to find
/// the needle within the haystack. This approach avoids allocating temporary
/// lowercase strings, making it suitable for frequent filtering operations.
///
/// # Performance
///
/// O(n*m) worst case where n is haystack length and m is needle length.
/// Early returns on empty needle or length mismatch.
#[inline]
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .char_indices()
        .filter(|(_, c)| {
            c.to_lowercase().next() == needle.chars().next().and_then(|n| n.to_lowercase().next())
        })
        .any(|(i, _)| {
            haystack[i..]
                .chars()
                .flat_map(|c| c.to_lowercase())
                .zip(needle.chars().flat_map(|c| c.to_lowercase()))
                .all(|(h, n)| h == n)
                && haystack[i..].chars().flat_map(|c| c.to_lowercase()).count()
                    >= needle.chars().flat_map(|c| c.to_lowercase()).count()
        })
}

impl DecryptionApp {
    pub(super) fn recalculate_filtered_indices(&mut self) {
        let mut indices: Vec<usize> = if self.filter_text.is_empty() {
            (0..self.project.segments.len()).collect()
        } else {
            let query = &self.filter_text;
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
                    // Pre-compute sort keys to avoid creating iterators on every comparison
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
                    indices.sort_by(|&a, &b| {
                        let len_a = self.project.segments[a].tokens.len();
                        let len_b = self.project.segments[b].tokens.len();
                        if self.sort_mode == SortMode::LengthAsc {
                            len_a.cmp(&len_b)
                        } else {
                            len_b.cmp(&len_a)
                        }
                    });
                }
                SortMode::TranslatedRatioAsc | SortMode::TranslatedRatioDesc => {
                    // Pre-calculate ratios to avoid repeated lookups during sorting
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
