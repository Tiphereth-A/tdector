//! Domain caching layer for expensive computations.
//!
//! Provides specialized caches for frequently-accessed data:
//! - [`LookupCache`]: Pre-computed indices for vocabulary lookups
//! - [`CachedTfidf`]: TF-IDF matrix cache for similarity search operations

use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use ndarray::Array2;

/// Caches pre-computed lookup indices for vocabulary queries.
///
/// Stores two `HashMap` indices:
/// - **`headword_lookup`**: Maps headwords to segment indices containing them
/// - **`usage_lookup`**: Maps words to segment indices where they appear in translations
///
/// Uses a take/restore pattern for temporary ownership: temporarily extract maps,
/// use them, then restore them to avoid cloning.
#[derive(Debug, Clone, Default)]
pub struct LookupCache {
    headword_lookup: Option<HashMap<String, Vec<usize>>>,
    usage_lookup: Option<HashMap<String, Vec<usize>>>,
}

impl LookupCache {
    /// Creates a new empty lookup cache.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            headword_lookup: None,
            usage_lookup: None,
        }
    }

    /// Takes ownership of both lookup maps for temporary use.
    pub fn take(
        &mut self,
    ) -> (
        Option<HashMap<String, Vec<usize>>>,
        Option<HashMap<String, Vec<usize>>>,
    ) {
        (self.headword_lookup.take(), self.usage_lookup.take())
    }

    /// Restores both lookup maps after temporary use.
    pub fn restore(
        &mut self,
        headword: Option<HashMap<String, Vec<usize>>>,
        usage: Option<HashMap<String, Vec<usize>>>,
    ) {
        self.headword_lookup = headword;
        self.usage_lookup = usage;
    }

    /// Marks the cache as dirty, clearing all stored data.
    pub fn invalidate(&mut self) {
        self.headword_lookup = None;
        self.usage_lookup = None;
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Caches TF-IDF vectorization matrices for similarity search.
///
/// Avoids expensive recomputation when segments haven't changed.
#[derive(Clone)]
pub struct CachedTfidf {
    matrix: Option<Array2<f64>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl CachedTfidf {
    /// Creates a new empty TF-IDF cache.
    pub fn new() -> Self {
        Self { matrix: None }
    }

    /// Stores the computed TF-IDF matrix.
    pub fn set_matrix(&mut self, matrix: Array2<f64>) {
        self.matrix = Some(matrix);
    }

    /// Returns a reference to the cached matrix if available.
    pub fn get_matrix(&self) -> Option<&Array2<f64>> {
        self.matrix.as_ref()
    }

    /// Checks if the cache is marked as dirty (invalid).
    pub fn is_dirty(&self) -> bool {
        self.matrix.is_none()
    }

    /// Clears the cached matrix.
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
/// WASM version: TF-IDF computation is not available, so cache is a no-op.
#[derive(Debug, Clone, Default)]
pub struct CachedTfidf;

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
impl CachedTfidf {
    /// No-op for WASM.
#[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }

    /// No-op for WASM.
#[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        true
    }

    /// No-op for WASM.
    pub fn invalidate(&mut self) {}
}
