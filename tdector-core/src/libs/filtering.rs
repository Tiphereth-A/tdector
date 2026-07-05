use crate::libs::Project;

/// Text filtering and search operations for finding relevant segments.
pub struct FilterOperation;

impl FilterOperation {
    /// Case-insensitive substring search. Empty needle matches all haystacks.
    #[inline]
    pub fn contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
        if needle_lower.is_empty() {
            return true;
        }
        if needle_lower.len() > haystack.len() {
            return false;
        }

        haystack.to_lowercase().contains(needle_lower)
    }

    /// Filter segment indices to those matching the query string.
    /// A segment matches if the query appears in its translation text or in any of its tokens.
    /// Empty query returns all segment indices.
    pub fn apply_filter(project: &Project, query: &str) -> Vec<usize> {
        if query.is_empty() {
            (0..project.segments.len()).collect()
        } else {
            let query_lower = query.to_lowercase();
            project
                .segments
                .iter()
                .enumerate()
                .filter(|(_idx, seg)| {
                    // Match if translation contains query or any token contains query
                    Self::contains_ignore_case(&seg.translation, &query_lower)
                        || seg
                            .tokens
                            .iter()
                            .any(|t| Self::contains_ignore_case(&t.original, &query_lower))
                })
                .map(|(idx, _)| idx)
                .collect()
        }
    }
}
