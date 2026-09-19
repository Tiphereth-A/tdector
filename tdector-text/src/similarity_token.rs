use std::collections::HashSet;

use tdector_core::consts::domain::MAX_SIMILAR_TOKENS_RESULTS;
use tdector_core::libs::Project;

/// Represents a similar token with its similarity metrics
#[derive(Debug, Clone)]
pub struct SimilarToken {
    /// The token text
    pub word: String,
    /// Levenshtein distance (fewer edits = more similar)
    pub distance: usize,
    /// Length of longest common substring
    pub lcs_length: usize,
}

/// Find the most textually similar tokens to a target word from all tokens in the project.
///
/// Returns up to `MAX_SIMILAR_TOKENS_RESULTS` most similar unique tokens, sorted by:
/// 1. Levenshtein distance (primary) - fewer edits = more similar
/// 2. Length of longest common substring (secondary) - longer shared substring = more similar
/// 3. Token text (lexical) for deterministic ties, including the result limit.
///
/// # Arguments
/// * `project` - The project containing all segments and tokens
/// * `word` - The target word to find similar tokens for
///
/// # Returns
/// Vector of unique similar tokens with their metrics, sorted by similarity (most similar first)
pub fn find_similar_tokens(project: &Project, word: &str) -> Vec<SimilarToken> {
    // Collect all unique tokens in the project
    let mut unique_tokens = HashSet::new();

    for segment in &project.segments {
        for token in &segment.tokens {
            unique_tokens.insert(token.original.clone());
        }
    }

    // Calculate similarity for each unique token
    let mut similar_tokens: Vec<SimilarToken> = unique_tokens
        .into_iter()
        .filter(|token_word| token_word != word) // Exclude the target word itself
        .map(|token_word| {
            let distance = textdistance::str::levenshtein(word, &token_word);
            let lcs_length = textdistance::str::lcsstr(word, &token_word);
            SimilarToken {
                word: token_word,
                distance,
                lcs_length,
            }
        })
        .collect();

    // If we have more tokens than the limit, use partial sort to get only the top N
    let limit = MAX_SIMILAR_TOKENS_RESULTS.min(similar_tokens.len());

    if similar_tokens.len() > limit {
        // Partition so that the N most similar tokens are at the front
        similar_tokens.select_nth_unstable_by(limit, compare_tokens);
        similar_tokens.truncate(limit);
    }

    // Sort the top N results
    similar_tokens.sort_by(compare_tokens);

    similar_tokens
}

fn compare_tokens(a: &SimilarToken, b: &SimilarToken) -> std::cmp::Ordering {
    a.distance
        .cmp(&b.distance)
        .then_with(|| b.lcs_length.cmp(&a.lcs_length))
        .then_with(|| a.word.cmp(&b.word))
}
