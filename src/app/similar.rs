//! TF-IDF-based similar segment search using cosine similarity.

use std::sync::Arc;

use tf_idf_vectorizer::{Corpus, Query, SimilarityAlgorithm, TFIDFVectorizer, TermFrequency};

use super::state::DecryptionApp;

impl DecryptionApp {
    /// Finds segments similar to `target_idx` using TF-IDF and cosine similarity.
    ///
    /// Stores results in `self.similar_popup`.
    pub(super) fn compute_similar_segments(&mut self, target_idx: usize) {
        if target_idx >= self.project.segments.len() {
            return;
        }

        let target_seg = match self.project.segments.get(target_idx) {
            Some(s) => s,
            None => return,
        };

        let target_terms: Vec<&str> = target_seg
            .tokens
            .iter()
            .map(|t| t.original.as_str())
            .collect();

        if target_terms.is_empty() {
            return;
        }

        let corpus = Arc::new(Corpus::new());
        let mut vectorizer: TFIDFVectorizer<f32, usize> = TFIDFVectorizer::new(corpus);

        for (idx, seg) in self.project.segments.iter().enumerate() {
            let terms: Vec<&str> = seg.tokens.iter().map(|t| t.original.as_str()).collect();
            if terms.is_empty() {
                continue;
            }
            let mut freq = TermFrequency::new();
            freq.add_terms(&terms);
            vectorizer.add_doc(idx, &freq);
        }

        let mut target_freq = TermFrequency::new();
        target_freq.add_terms(&target_terms);
        let query = Query::from_freq_or(&target_freq);

        let algorithm = SimilarityAlgorithm::CosineSimilarity;
        let mut hits = vectorizer.search(&algorithm, query);
        hits.sort_by_score_desc();

        let scores: Vec<(usize, f64)> = hits
            .list
            .iter()
            .filter(|e| e.key != target_idx)
            .map(|e| (e.key, e.score))
            .take(5)
            .collect();

        self.similar_popup = Some((target_idx, scores));
    }
}
