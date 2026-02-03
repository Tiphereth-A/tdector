use serde::{Deserialize, Serialize};

/// A word reference in a sentence - can be a single index or a word with applied formation rule.
///
/// This allows sentences to reference either:
/// - A single vocabulary entry (simple case): `4`
/// - A vocabulary entry with formation rules applied: `[vocab_idx, rule_idx, ...]`
///   where `vocab_idx` references the base word and following indices are formation rules to apply in order
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WordRef {
    /// Single vocabulary index
    Single(usize),
    /// Word with applied formation rules: [`vocabulary_index`, `rule_index`, ...]
    WithRule(Vec<usize>),
}

impl WordRef {
    /// Get the vocabulary index for this word reference
    pub fn vocab_index(&self) -> Option<usize> {
        match self {
            WordRef::Single(idx) => Some(*idx),
            WordRef::WithRule(indices) => indices.first().copied(),
        }
    }

    /// Get the rule indices if this word has formation rules applied
    pub fn rule_indices(&self) -> Vec<usize> {
        match self {
            WordRef::Single(_) => Vec::new(),
            WordRef::WithRule(indices) => indices.iter().skip(1).copied().collect(),
        }
    }
}

/// Comment target for a word or formatted word.
#[derive(Debug, Clone)]
pub enum CommentTarget {
    BaseWord(String),
    FormattedWord(String),
}
