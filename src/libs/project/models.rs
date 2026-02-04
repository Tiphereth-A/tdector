use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use crate::libs::formation::FormationRule;

/// Represents a single token (word or character) within a segment.
/// Tokens track their original form and can reference word formation rules for derived words.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Token {
    /// The actual text representation of the token as it appears in the source
    pub original: String,

    /// If this token is a derived form, base_word stores the root word.
    /// Skipped during serialization since it can be reconstructed from formation rules.
    #[serde(skip)]
    pub base_word: Option<String>,

    /// Indices into the Project's formation_rules that were applied to base_word to create original.
    /// Empty if this is an original vocabulary token (not derived).
    #[serde(skip)]
    pub formation_rule_indices: Vec<usize>,
}

/// Represents a logical unit of text containing tokens and its translation.
/// Typically corresponds to a sentence or line from the original source.
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

/// Root container for a translation/decryption project.
/// Manages all segments, vocabulary, word formation rules, and metadata.
#[derive(Debug, Clone, Default)]
pub struct Project {
    /// User-assigned name for this project
    pub project_name: String,

    /// Optional path to a custom font file for rendering special scripts
    pub font_path: Option<String>,

    /// Map of word -> definition for the project vocabulary.
    /// Deduplicates words across all segments to minimize file size.
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

/// Serialization format for a single vocabulary entry.
/// Used when saving projects to JSON in the compressed SavedVocabularyV2 format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VocabEntry {
    /// The vocabulary word
    pub word: String,

    /// The definition or meaning of the word
    pub meaning: String,

    /// Optional comment/note about this vocabulary entry
    #[serde(default)]
    pub comment: String,
}

/// Serialization format for a word created by applying formation rules.
/// Represents a derived form as an index chain: [base_word_idx, rule_idx_1, rule_idx_2, ...]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormattedWordEntry {
    /// First element is base word vocabulary index; subsequent elements are formation rule indices.
    /// This chain allows reconstructing the derived word by applying rules sequentially.
    pub word: Vec<usize>,

    /// Optional comment/note about this derived word
    #[serde(default)]
    pub comment: String,
}

/// Vocabulary storage for project version 2 format.
/// Separates original vocabulary from derived/formatted words for efficient storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedVocabularyV2 {
    /// Collection of base vocabulary words (note: field named 'orignal' in JSON for historical reasons)
    #[serde(rename = "orignal")]
    pub original: Vec<VocabEntry>,

    /// Collection of derived words created by applying formation rules
    #[serde(default)]
    pub formatted: Vec<FormattedWordEntry>,
}

/// Serialization format for a single segment (sentence/line of text).
/// Word references use positive integers for base vocabulary and negative integers for formatted words.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSentenceV2 {
    /// Array of word references:
    /// - Positive i64: index into vocabulary.original array
    /// - Negative i64: -(formatted_word_index + 1) for derived words
    pub words: Vec<i64>,

    /// The target language translation for this segment
    pub meaning: String,

    /// User annotation/notes for this segment
    #[serde(default)]
    pub comment: String,
}

/// Complete project serialization format (version 2).
/// This is the format used for saving and loading projects from JSON files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedProjectV2 {
    /// Version number for format compatibility checking and migration
    pub version: u64,

    /// Project name/title
    #[serde(default)]
    pub project_name: String,

    /// All word formation rules in the project
    #[serde(default)]
    pub formation: Vec<FormationRule>,

    /// Vocabulary (original and derived/formatted words)
    pub vocabulary: SavedVocabularyV2,

    /// All text segments in the project
    pub sentences: Vec<SavedSentenceV2>,
}
