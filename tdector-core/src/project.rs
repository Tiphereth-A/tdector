use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use tdector_eval::FormationRule;

/// Represents a single token (word or character) within a segment. Tokens track their original form and can reference word formation rules for derived words.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Token {
    /// The actual text representation of the token as it appears in the source
    pub original: String,

    /// If this token is a derived form, `base_word` stores the root word. Skipped during serialization since it can be reconstructed from formation rules.
    #[serde(skip)]
    pub base_word: Option<String>,

    /// Indices into the Project's `formation_rules` that were applied to `base_word` to create original. Empty if this is an original vocabulary token (not derived).
    #[serde(skip)]
    pub formation_rule_indices: Vec<usize>,
}

/// Represents a logical unit of text containing tokens and its translation. Typically corresponds to a sentence or line from the original source.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Segment {
    /// Collection of tokens (words or characters) that make up this segment
    pub tokens: Vec<Token>,

    /// The target language translation or interpretation of this segment
    pub translation: String,

    /// User-provided notes or annotations for this entire segment
    #[serde(skip)]
    pub comment: String,
}

/// Root container for a translation/decryption project. Manages all segments, vocabulary, word formation rules, and metadata.
#[derive(Debug, Clone, Default)]
pub struct Project {
    /// User-assigned name for this project
    pub project_name: String,

    /// Map of word -> definition for the project vocabulary. Deduplicates words across all segments to minimize file size.
    pub vocabulary: HashMap<String, String>,

    /// Comments/notes for vocabulary words, separate from definitions
    pub vocabulary_comments: HashMap<String, String>,

    /// Comments for derived/formatted words created by applying formation rules
    pub formatted_word_comments: HashMap<String, String>,

    /// All text segments in the project
    pub segments: Vec<Segment>,

    /// Word formation rules (Rhai scripts) for generating derived forms from base words
    pub formation_rules: Vec<FormationRule>,
}
