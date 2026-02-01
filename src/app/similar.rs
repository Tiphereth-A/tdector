//! TF-IDF similarity search for finding related text segments.
//!
//! This module implements a similarity search engine using TF-IDF (Term Frequency-Inverse
//! Document Frequency) vectorization and cosine similarity scoring. The implementation
//! uses cached matrices to avoid recomputation and provides the top-5 most similar
//! segments for any given segment.
//!
//! # Algorithm
//!
//! 1. Convert each segment into a TF-IDF vector representing term importance
//! 2. Calculate cosine similarity between the target vector and all other vectors
//! 3. Return the top-5 highest-scoring segments
//!
//! The TF-IDF matrix is cached and only regenerated when segments change.

use ndarray::Array2;
use scirs2_text::{TfidfVectorizer, Vectorizer, WhitespaceTokenizer, cosine_similarity};

use super::state::DecryptionApp;

impl DecryptionApp {
    /// Ensures the TF-IDF matrix cache is up-to-date.
    ///
    /// Regenerates the TF-IDF vectorization matrix if the cache is dirty or missing.
    /// This method is called automatically by [`compute_similar_segments`] but can
    /// also be called proactively to amortize computation cost.
    ///
    /// # Implementation Notes
    ///
    /// - Uses whitespace tokenization to preserve pre-tokenized words
    /// - Applies L2 normalization to the TF-IDF vectors
    /// - Returns early if cache is valid or project is empty
    ///
    /// [`compute_similar_segments`]: Self::compute_similar_segments
    pub(super) fn ensure_tfidf_cache(&mut self) {
        if !self.tfidf_dirty && !self.tfidf_cache.is_dirty() {
            return;
        }

        if self.project.segments.is_empty() {
            self.tfidf_cache = super::cache_tfidf::CachedTfidf::default();
            self.tfidf_dirty = false;
            return;
        }

        let documents: Vec<String> = self
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

        let tokenizer = Box::new(WhitespaceTokenizer::new());
        let mut vectorizer =
            TfidfVectorizer::with_tokenizer(tokenizer, false, true, Some("l2".to_string()));

        let doc_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();
        match vectorizer.fit_transform(&doc_refs) {
            Ok(matrix) => {
                self.tfidf_cache.set_matrix(matrix);
            }
            Err(_) => {
                self.tfidf_cache = super::cache_tfidf::CachedTfidf::default();
            }
        }
        self.tfidf_dirty = false;
    }

    /// Computes the top-5 most similar segments to the target segment.
    ///
    /// Uses the cached TF-IDF matrix to calculate cosine similarity scores between
    /// the target segment and all other segments, then stores the top-5 results
    /// in the similarity popup state.
    ///
    /// # Arguments
    ///
    /// * `target_idx` - Index of the segment to find similarities for
    ///
    /// # Implementation
    ///
    /// 1. Ensures TF-IDF cache is current
    /// 2. Extracts the target segment's TF-IDF vector
    /// 3. Computes cosine similarity with all other segments
    /// 4. Sorts by similarity (descending) and takes top 5
    /// 5. Stores results in `self.similar_popup`
    ///
    /// Segments with zero similarity are excluded from results.
    pub(super) fn compute_similar_segments(&mut self, target_idx: usize) {
        if target_idx >= self.project.segments.len() {
            return;
        }

        self.ensure_tfidf_cache();

        let matrix: &Array2<f64> = match self.tfidf_cache.get_matrix() {
            Some(m) => m,
            None => return,
        };

        let target_vector = matrix.row(target_idx).into_owned();

        let mut scores: Vec<(usize, f64)> = Vec::new();
        for idx in 0..self.project.segments.len() {
            if idx == target_idx {
                continue;
            }

            let doc_vector = matrix.row(idx);
            match cosine_similarity(target_vector.view(), doc_vector) {
                Ok(sim) => {
                    if sim > 0.0 {
                        scores.push((idx, sim));
                    }
                }
                Err(_) => continue,
            }
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(5);

        self.similar_popup = Some((target_idx, scores));
    }
}
