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

#[cfg(not(target_arch = "wasm32"))]
use ndarray::Array2;
#[cfg(not(target_arch = "wasm32"))]
use scirs2_text::{TfidfVectorizer, Vectorizer, WhitespaceTokenizer, cosine_similarity};

use super::state::DecryptionApp;

#[cfg(not(target_arch = "wasm32"))]
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

        let mut documents: Vec<String> = Vec::with_capacity(self.project.segments.len());
        for seg in &self.project.segments {
            let estimated_len = seg.tokens.iter().map(|t| t.original.len()).sum::<usize>()
                + seg.tokens.len().saturating_sub(1);
            let mut doc = String::with_capacity(estimated_len);
            for (idx, token) in seg.tokens.iter().enumerate() {
                if idx > 0 {
                    doc.push(' ');
                }
                doc.push_str(&token.original);
            }
            documents.push(doc);
        }

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

        let target_vector = matrix.row(target_idx);

        let mut scores: Vec<(usize, f64)> =
            Vec::with_capacity(self.project.segments.len().saturating_sub(1));
        for idx in 0..self.project.segments.len() {
            if idx == target_idx {
                continue;
            }

            let doc_vector = matrix.row(idx);
            match cosine_similarity(target_vector, doc_vector) {
                Ok(sim) => {
                    if sim > 0.0 {
                        scores.push((idx, sim));
                    }
                }
                Err(_) => continue,
            }
        }

        if scores.len() > 5 {
            let nth = 5;
            scores.select_nth_unstable_by(nth, |a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            });
            scores.truncate(5);
        }
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        self.similar_popup = Some((target_idx, scores));
    }
}

#[cfg(target_arch = "wasm32")]
impl DecryptionApp {
    /// WASM version: Similarity search not supported due to upstream SIMD dependency.
    ///
    /// The scirs2-text library requires SIMD instructions which are not yet available
    /// in stable WASM. This feature will be enabled once SciRS2 v0.3.0 is released
    /// with WASM support.

    /// WASM version: Similarity search not supported due to upstream SIMD dependency.
    ///
    /// Shows an error message explaining that similarity features are not available
    /// in the web version due to waiting on SciRS2 v0.3.0 for WASM compatibility.

    pub(super) fn compute_similar_segments(&mut self, _target_idx: usize) {
        self.error_message = Some(
            "Similarity search is not yet supported in the web version. \
            This feature requires the SciRS2 library which depends on SIMD instructions. \
            Support will be added once SciRS2 v0.3.0 is released with WASM compatibility. \
            Please use the desktop application for similarity features."
                .to_string(),
        );
    }
}
