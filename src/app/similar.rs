//! BM25-based similar segment search.

use bm25::{Language, LanguageMode, SearchEngineBuilder};

use super::state::DecryptionApp;

impl DecryptionApp {
    /// Finds segments similar to `target_idx` using BM25 scoring.
    ///
    /// Stores results in `self.active_popup`.
    pub(super) fn compute_similar_segments(&mut self, target_idx: usize) {
        if target_idx >= self.project.segments.len() {
            return;
        }

        // Reconstruct all segments as strings for BM25
        let corpus: Vec<String> = self
            .project
            .segments
            .iter()
            .map(|seg| {
                seg.tokens
                    .iter()
                    .map(|t| t.original.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();

        if let Some(target_text) = corpus.get(target_idx).cloned() {
            // Build BM25 Search Engine
            let engine = SearchEngineBuilder::<u32, u32>::with_corpus(
                LanguageMode::Fixed(Language::English),
                corpus,
            )
            .build();

            // Search for similar documents. Request 11 results to account for self-match.
            let results = engine.search(&target_text, 11);

            let scores: Vec<(usize, f64)> = results
                .iter()
                .map(|r| (r.document.id as usize, r.score as f64))
                .filter(|(idx, _)| *idx != target_idx)
                .take(10)
                .collect();

            self.similar_popup = Some((target_idx, scores));
        }
    }
}
