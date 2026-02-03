//! Type-safe wrappers for domain concepts.
//!
//! This module provides newtype wrappers that add compile-time safety
//! to prevent mixing different kinds of indices and IDs.

use serde::{Deserialize, Serialize};

/// Type-safe wrapper for segment indices.
///
/// Prevents accidentally mixing segment indices with token indices
/// or other integer values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SegmentIndex(pub usize);

impl SegmentIndex {
    /// Creates a new segment index.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Gets the raw index value.
    pub fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for SegmentIndex {
    fn from(index: usize) -> Self {
        Self(index)
    }
}

impl From<SegmentIndex> for usize {
    fn from(idx: SegmentIndex) -> Self {
        idx.0
    }
}

/// Type-safe wrapper for token indices within a segment.
///
/// Prevents accidentally using segment indices where token indices
/// are expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenIndex(pub usize);

impl TokenIndex {
    /// Creates a new token index.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Gets the raw index value.
    pub fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for TokenIndex {
    fn from(index: usize) -> Self {
        Self(index)
    }
}

impl From<TokenIndex> for usize {
    fn from(idx: TokenIndex) -> Self {
        idx.0
    }
}

/// Type-safe wrapper for formation rule indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuleIndex(pub usize);

impl RuleIndex {
    /// Creates a new rule index.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Gets the raw index value.
    pub fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for RuleIndex {
    fn from(index: usize) -> Self {
        Self(index)
    }
}

impl From<RuleIndex> for usize {
    fn from(idx: RuleIndex) -> Self {
        idx.0
    }
}
