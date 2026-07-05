use serde::{Deserialize, Serialize};

/// Strongly-typed wrapper for segment indices to prevent mixing with token or rule indices.
/// Segments are top-level containers of tokens (words/characters) that represent logical units of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SegmentIndex(pub usize);

impl SegmentIndex {
    /// Create a new segment index from a raw usize value
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Extract the underlying index value
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

/// Strongly-typed wrapper for token indices within a segment.
/// Tokens represent individual words or characters depending on segmentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenIndex(pub usize);

impl TokenIndex {
    /// Create a new token index from a raw usize value
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Extract the underlying index value
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

/// Strongly-typed wrapper for word formation rule indices.
/// Rules contain Rhai scripts that transform base words into derived forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuleIndex(pub usize);

impl RuleIndex {
    /// Create a new rule index from a raw usize value
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Extract the underlying index value
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
