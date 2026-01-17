//! Core data models for runtime and serialization.
//!
//! Two complementary model sets:
//! - **Runtime**: [`Project`], [`Segment`], [`Token`] — in-memory representation
//! - **Storage**: [`SavedProject`], [`SavedSentence`], [`VocabEntry`] — indexed JSON format

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Runtime Models
// =============================================================================

/// A single token (word or character) within a text segment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Token {
    /// The original text of this token.
    pub original: String,
}

/// A text segment containing tokens and its translation.
///
/// Each segment represents a line or sentence from the source text,
/// broken down into individual tokens with an associated translation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Segment {
    /// The tokens that compose this segment.
    pub tokens: Vec<Token>,
    /// The translation of this segment.
    pub translation: String,
}

/// The main project data structure used during runtime.
///
/// This structure maintains all project state including vocabulary mappings
/// and text segments. For disk serialization, this is converted to [`SavedProject`].
#[derive(Debug, Clone, Default)]
pub struct Project {
    /// Display name for the project.
    pub project_name: String,
    /// Path to the custom font file for this project.
    pub font_path: Option<String>,
    /// Vocabulary map: maps each word to its gloss (meaning).
    pub vocabulary: HashMap<String, String>,
    /// All text segments in the project.
    pub segments: Vec<Segment>,
}

// =============================================================================
// Serialization Models (JSON format)
// =============================================================================

/// A vocabulary entry for serialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VocabEntry {
    /// The word/token text.
    pub word: String,
    /// The meaning (gloss) of the word.
    pub meaning: String,
}

/// A sentence entry optimized for serialization.
///
/// Uses vocabulary indices instead of storing word strings directly,
/// eliminating redundancy in the saved file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSentence {
    /// Indices into the vocabulary array.
    pub words: Vec<usize>,
    /// The translation for this sentence.
    pub meaning: String,
}

/// The optimized project format for JSON serialization.
///
/// This format uses indexed vocabulary references to deduplicate repeated words,
/// significantly reducing file size for projects with extensive vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedProject {
    /// File format version.
    #[serde(default = "default_version")]
    pub version: u32,
    /// Display name for the project.
    #[serde(default)]
    pub project_name: String,
    /// Deduplicated vocabulary entries.
    pub vocabulary: Vec<VocabEntry>,
    /// Sentences with word indices referencing the vocabulary.
    pub sentences: Vec<SavedSentence>,
}

/// Default file format version for new projects.
const fn default_version() -> u32 {
    1
}
