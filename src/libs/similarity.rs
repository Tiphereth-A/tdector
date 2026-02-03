//! Domain similarity search using TF-IDF ranking.
//!
//! Implements segment similarity ranking via TF-IDF (Term Frequency-Inverse Document Frequency)
//! and cosine similarity metrics. Useful for finding related or similar text passages.
//!
//! ## Algorithm
//! 1. Compute TF-IDF vectors for all segments (cached across updates)
//! 2. Calculate cosine similarity between target and all other vectors
//! 3. Rank results by similarity score and return top-N matches
//!
//! Note: Desktop only (requires `scirs2` with SIMD support; WASM version unavailable)

#[cfg(not(target_arch = "wasm32"))]
use ndarray::Array2;
#[cfg(not(target_arch = "wasm32"))]
use scirs2_text::{TfidfVectorizer, Vectorizer, WhitespaceTokenizer, cosine_similarity};

use crate::libs::Project;

/// Similarity search engine using TF-IDF vectors.
pub struct SimilarityEngine;

#[cfg(not(target_arch = "wasm32"))]
impl SimilarityEngine {
    /// Computes the TF-IDF matrix for all segments in a project.
    ///
    /// Uses whitespace tokenization to preserve pre-tokenized words and
    /// applies L2 normalization to the TF-IDF vectors.
    ///
    /// # Arguments
    ///
    /// * `project` - The project containing segments to vectorize
    ///
    /// # Returns
    ///
    /// The computed TF-IDF matrix, or None if computation fails
    pub fn compute_tfidf_matrix(project: &Project) -> Option<Array2<f64>> {
        if project.segments.is_empty() {
            return None;
        }

        let mut documents: Vec<String> = Vec::with_capacity(project.segments.len());
        for seg in &project.segments {
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
        vectorizer.fit_transform(&doc_refs).ok()
    }

    /// Computes the top-N most similar segments to a target segment.
    ///
    /// Uses the TF-IDF matrix to calculate cosine similarity scores between
    /// the target segment and all other segments.
    ///
    /// # Arguments
    ///
    /// * `matrix` - The TF-IDF matrix for all segments
    /// * `target_idx` - Index of the segment to find similarities for
    /// * `limit` - Maximum number of results to return (default 5)
    ///
    /// # Returns
    ///
    /// Vector of (`segment_index`, `similarity_score`) pairs sorted by descending score.
    /// Excludes segments with zero similarity.
    pub fn find_similar(
        matrix: &Array2<f64>,
        target_idx: usize,
        limit: usize,
    ) -> Vec<(usize, f64)> {
        if target_idx >= matrix.nrows() {
            return Vec::new();
        }

        let target_vector = matrix.row(target_idx);
        let mut similarities = Vec::new();

        for (idx, row) in matrix.rows().into_iter().enumerate() {
            if idx == target_idx {
                continue;
            }

            match cosine_similarity(target_vector, row) {
                Ok(score) => {
                    if score > 0.0 {
                        similarities.push((idx, score));
                    }
                }
                Err(_) => continue,
            }
        }

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(limit);
        similarities
    }
}

#[cfg(target_arch = "wasm32")]
impl SimilarityEngine {
    /// WASM version: TF-IDF computation is not available on WASM.
    pub fn compute_tfidf_matrix(_project: &Project) -> Option<()> {
        None
    }

    /// WASM version: No-op similarity search.
    pub fn find_similar(_matrix: &(), _target_idx: usize, _limit: usize) -> Vec<(usize, f32)> {
        Vec::new()
    }
}
