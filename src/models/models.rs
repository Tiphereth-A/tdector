//! Data models: Runtime and serializable structures.
//!
//! Defines core domain concepts in two formats:
//!
//! **Runtime Models** (in-memory, full structure):
//! - [`Project`] - Complete project with all segments and metadata
//! - [`Segment`] - Text segment with tokens and annotations
//! - [`Token`] - Individual word/character with linked vocabulary entries
//!
//! **Storage Models** (optimized for persistence):
//! - [`SavedProject`] - Serialized format using vocabulary indices
//! - [`SavedSentence`] - Segment using integer references to vocab
//! - [`VocabEntry`] - Vocabulary entry with definitions and metadata
//!
//! Storage models significantly reduce file size by eliminating redundant strings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use crate::lib::formation::FormationRule;

/// A single token representing a word or character within a text segment.
///
/// Tokens are the atomic units of text analysis. The actual gloss (meaning)
/// is stored separately in the [`Project`] vocabulary map to avoid duplication
/// when the same token appears multiple times.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Token {
    /// The original text of this token (may be transformed by formation rules).
    pub original: String,
    /// Optional comment for this token (stored at runtime, not in JSON).
    #[serde(skip)]
    pub comment: String,
    /// The base word this token came from (before formation rule was applied).
    /// Stored at runtime only, used for saving the correct `WordRef`.
    #[serde(skip)]
    pub base_word: Option<String>,
    /// Indices of formation rules applied to this token (if any), stored at runtime only.
    /// This tracks that the token's text is the result of applying one or more formation rules.
    #[serde(skip)]
    pub formation_rule_indices: Vec<usize>,
}

/// A text segment containing tokens and its translation.
///
/// Segments represent logical units of text (typically lines or sentences)
/// that are analyzed and translated as cohesive units. Each segment maintains
/// its own token sequence and overall translation, enabling both word-level
/// and sentence-level analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Segment {
    /// The tokens that compose this segment.
    pub tokens: Vec<Token>,
    /// The translation of this segment.
    pub translation: String,
    /// Optional comment for this segment (stored at runtime, not in JSON).
    #[serde(skip)]
    pub comment: String,
}

/// The main project data structure used during runtime operations.
///
/// This is the primary in-memory representation that the application works with.
/// It maintains a centralized vocabulary map where each unique token's gloss
/// is stored exactly once, referenced by all occurrences of that token.
///
/// When persisting to disk, this structure is converted to [`SavedProject`]
/// for a more space-efficient representation.
#[derive(Debug, Clone, Default)]
pub struct Project {
    /// Display name for the project.
    pub project_name: String,
    /// Path to the custom font file for this project.
    pub font_path: Option<String>,
    /// Vocabulary map: maps each word to its gloss (meaning).
    pub vocabulary: HashMap<String, String>,
    /// Comments map: maps each word to its comment.
    pub vocabulary_comments: HashMap<String, String>,
    /// All text segments in the project.
    pub segments: Vec<Segment>,
    /// Word formation rules for deriving and inflecting words.
    pub formation_rules: Vec<FormationRule>,
}

/// A vocabulary entry for serialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VocabEntry {
    /// The word/token text.
    pub word: String,
    /// The meaning (gloss) of the word.
    pub meaning: String,
    /// Optional comment for the word.
    #[serde(default)]
    pub comment: String,
}

use crate::enums::WordRef;

/// A sentence entry optimized for serialization.
///
/// Uses vocabulary indices instead of storing word strings directly,
/// eliminating redundancy in the saved file format. Words can be single indices
/// or arrays of indices for word formation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSentence {
    /// References into the vocabulary array, supporting single or multiple indices per word position.
    pub words: Vec<WordRef>,
    /// The translation for this sentence.
    pub meaning: String,
    /// Optional comment for the sentence.
    #[serde(default)]
    pub comment: String,
}

/// Space-optimized project format for JSON serialization.
///
/// This format employs vocabulary indexing to eliminate string duplication:
/// each unique word is stored once in the vocabulary array, and sentences
/// reference words by their array index. This can reduce file size by 50-80%
/// for projects with significant vocabulary reuse.
///
/// The vocabulary array is automatically sorted during serialization to ensure
/// deterministic output and enable efficient lookups during deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedProject {
    /// File format version.
    #[serde(default = "default_version")]
    pub version: u32,
    /// Display name for the project.
    #[serde(default)]
    pub project_name: String,
    /// Word formation rules for deriving and inflecting words.
    #[serde(default)]
    pub formation: Vec<FormationRule>,
    /// Deduplicated vocabulary entries.
    pub vocabulary: Vec<VocabEntry>,
    /// Sentences with word indices referencing the vocabulary.
    pub sentences: Vec<SavedSentence>,
}

/// Returns the current file format version for new projects.
///
/// This version number is used for format compatibility checking and
/// enables graceful handling of future format migrations.
const fn default_version() -> u32 {
    1
}
