//! Segment filtering and sorting logic.

use super::actions::SortMode;
use super::state::DecryptionApp;

impl DecryptionApp {
    pub(super) fn recalculate_filtered_indices(&mut self) {
        let mut indices: Vec<usize> = if self.filter_text.is_empty() {
            (0..self.project.segments.len()).collect()
        } else {
            let query = self.filter_text.to_lowercase();
            self.project
                .segments
                .iter()
                .enumerate()
                .filter(|(_idx, seg)| {
                    seg.translation.to_lowercase().contains(&query)
                        || seg
                            .tokens
                            .iter()
                            .any(|t| t.original.to_lowercase().contains(&query))
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
                    indices.sort_by(|&a, &b| {
                        let seg_a = self.project.segments[a]
                            .tokens
                            .iter()
                            .flat_map(|t| t.original.chars());
                        let seg_b = self.project.segments[b]
                            .tokens
                            .iter()
                            .flat_map(|t| t.original.chars());

                        let cmp = seg_a.cmp(seg_b);

                        if self.sort_mode == SortMode::OriginalAsc {
                            cmp
                        } else {
                            cmp.reverse()
                        }
                    });
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
