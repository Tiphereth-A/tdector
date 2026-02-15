#[cfg(not(target_arch = "wasm32"))]
use ndarray::Array2;
#[cfg(not(target_arch = "wasm32"))]
use scirs2_text::{TfidfVectorizer, Vectorizer, WhitespaceTokenizer, cosine_similarity};

use crate::libs::Project;

/// Similarity search engine for finding semantically related segments using TF-IDF vectors.
/// Only available on native platforms; WASM version returns empty results.
pub struct SimilarityEngine;

#[cfg(not(target_arch = "wasm32"))]
impl SimilarityEngine {
    /// Compute the TF-IDF matrix from all segments in the project.
    /// Each segment is treated as a document with tokens separated by whitespace.
    /// Returns None if the project has no segments or computation fails.
    pub fn compute_tfidf_matrix(project: &Project) -> Option<Array2<f64>> {
        if project.segments.is_empty() {
            return None;
        }

        // Reconstruct document strings from token sequences
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

        // Create TF-IDF vectorizer with whitespace tokenization and L2 normalization
        let tokenizer = Box::new(WhitespaceTokenizer::new());
        let mut vectorizer =
            TfidfVectorizer::with_tokenizer(tokenizer, false, true, Some("l2".to_string()));

        let doc_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();
        vectorizer.fit_transform(&doc_refs).ok()
    }

    /// Find the most similar segments to a target segment using cosine similarity.
    /// Returns a vector of (`segment_index`, `similarity_score`) sorted by score in descending order.
    /// Scores are clamped to be > 0.0 to avoid near-zero or negative similarities.
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

        // Compute cosine similarity with all other segments
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

        // Sort by score descending and limit results
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(limit);
        similarities
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
impl SimilarityEngine {
    #[allow(dead_code)]
    pub fn compute_tfidf_matrix(_project: &Project) -> Option<()> {
        None
    }

    #[allow(dead_code)]
    pub fn find_similar(_matrix: &(), _target_idx: usize, _limit: usize) -> Vec<(usize, f32)> {
        Vec::new()
    }
}
