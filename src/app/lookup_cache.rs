//! Lookup cache manager for efficient vocabulary and usage queries.

use std::collections::HashMap;

/// Manages lookup map caching with RAII-style take/restore pattern.
///
/// This struct encapsulates the pattern of temporarily taking ownership
/// of lookup maps to pass immutable references, then restoring them.
pub struct LookupCache {
    headword_lookup: Option<HashMap<String, Vec<usize>>>,
    usage_lookup: Option<HashMap<String, Vec<usize>>>,
}

impl LookupCache {
    /// Creates a new empty lookup cache.
    pub fn new() -> Self {
        Self {
            headword_lookup: None,
            usage_lookup: None,
        }
    }

    /// Takes ownership of both lookup maps for temporary use.
    ///
    /// # Returns
    ///
    /// A tuple of (headword_lookup, usage_lookup) that can be passed
    /// as immutable references.
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

    /// Returns immutable references to both lookup maps.
    #[allow(dead_code)]
    pub fn as_refs(
        &self,
    ) -> (
        &Option<HashMap<String, Vec<usize>>>,
        &Option<HashMap<String, Vec<usize>>>,
    ) {
        (&self.headword_lookup, &self.usage_lookup)
    }
}

impl Default for LookupCache {
    fn default() -> Self {
        Self::new()
    }
}
