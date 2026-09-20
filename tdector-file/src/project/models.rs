use serde::{Deserialize, Serialize};

pub use tdector_core::project::{Project, Segment, Token};
pub use tdector_eval::FormationRule;

/// Serialization format for a single vocabulary entry. Used when saving projects to JSON in the compressed `SavedVocabularyV2` format.
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

/// Serialization format for a word created by applying formation rules. Represents a derived form as an index chain: [`base_word_idx`, `rule_idx_1`, `rule_idx_2`, ...]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormattedWordEntry {
    /// First element is base word vocabulary index; subsequent elements are formation rule indices. This chain allows reconstructing the derived word by applying rules sequentially.
    pub word: Vec<usize>,

    /// Optional comment/note about this derived word
    #[serde(default)]
    pub comment: String,
}

/// Vocabulary storage for project version 2 format. Separates original vocabulary from derived/formatted words for efficient storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedVocabularyV2 {
    /// Collection of base vocabulary words
    pub original: Vec<VocabEntry>,

    /// Collection of derived words created by applying formation rules
    #[serde(default)]
    pub formatted: Vec<FormattedWordEntry>,
}

/// Serialization format for a single segment (sentence/line of text). Word references use positive integers for base vocabulary and negative integers for formatted words.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSentenceV2 {
    /// Array of word references:
    /// - Positive i64: index into vocabulary.original array
    /// - Negative i64: -(`formatted_word_index` + 1) for derived words
    pub words: Vec<i64>,

    /// The target language translation for this segment
    pub meaning: String,

    /// User annotation/notes for this segment
    #[serde(default)]
    pub comment: String,
}

/// Complete project serialization format (version 2). This is the format used for saving and loading projects from JSON files.
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
