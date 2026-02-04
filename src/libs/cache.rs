use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use ndarray::Array2;

/// Caches lookup maps for quick token searches across the project.
/// Stores two separate lookup indices: one for headword (base word) lookups
/// and one for usage (all occurrences) lookups.
#[derive(Debug, Clone, Default)]
pub struct LookupCache {
    /// Maps headwords to the segment+token indices where they appear as base words
    headword_lookup: Option<HashMap<String, Vec<usize>>>,
    /// Maps words to all segment+token indices where they appear (including derived forms)
    usage_lookup: Option<HashMap<String, Vec<usize>>>,
}

impl LookupCache {
    /// Create a new empty lookup cache
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            headword_lookup: None,
            usage_lookup: None,
        }
    }

    /// Extract both lookup maps from the cache (ownership transfer).
    /// After calling this, the cache is empty until restored.
    pub fn take(
        &mut self,
    ) -> (
        Option<HashMap<String, Vec<usize>>>,
        Option<HashMap<String, Vec<usize>>>,
    ) {
        (self.headword_lookup.take(), self.usage_lookup.take())
    }

    /// Restore lookup maps to the cache
    pub fn restore(
        &mut self,
        headword: Option<HashMap<String, Vec<usize>>>,
        usage: Option<HashMap<String, Vec<usize>>>,
    ) {
        self.headword_lookup = headword;
        self.usage_lookup = usage;
    }

    /// Clear all cached lookup data
    pub fn invalidate(&mut self) {
        self.headword_lookup = None;
        self.usage_lookup = None;
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Caches the TF-IDF (Term Frequency-Inverse Document Frequency) matrix computed from project segments.
/// Used for similarity search to find semantically similar segments.
#[derive(Clone)]
pub struct CachedTfidf {
    /// The cached TF-IDF matrix (None means cache is invalid/dirty)
    matrix: Option<Array2<f64>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl CachedTfidf {
    /// Create a new empty TF-IDF cache
    pub fn new() -> Self {
        Self { matrix: None }
    }

    /// Store a computed TF-IDF matrix in the cache
    pub fn set_matrix(&mut self, matrix: Array2<f64>) {
        self.matrix = Some(matrix);
    }

    /// Retrieve a reference to the cached TF-IDF matrix, if available
    pub fn get_matrix(&self) -> Option<&Array2<f64>> {
        self.matrix.as_ref()
    }

    /// Check if the cache is invalid/dirty (no matrix cached)
    pub fn is_dirty(&self) -> bool {
        self.matrix.is_none()
    }

    /// Clear the cached matrix
    pub fn invalidate(&mut self) {
        self.matrix = None;
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for CachedTfidf {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for CachedTfidf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedTfidf")
            .field("matrix_valid", &self.matrix.is_some())
            .finish()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Default)]
pub struct CachedTfidf;

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
impl CachedTfidf {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }

    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        true
    }

    pub fn invalidate(&mut self) {}
}
