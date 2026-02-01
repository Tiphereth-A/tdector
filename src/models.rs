//! Core data models for runtime operations and persistent storage.
//!
//! This module defines two complementary sets of data structures:
//!
//! # Runtime Models
//!
//! - [`Project`] - Complete in-memory project state with vocabulary map
//! - [`Segment`] - A text segment with tokens and translation
//! - [`Token`] - Individual word or character unit
//!
//! # Storage Models
//!
//! - [`SavedProject`] - Space-optimized format using indexed vocabulary
//! - [`SavedSentence`] - Sentence with vocabulary references instead of strings
//! - [`VocabEntry`] - Vocabulary entry for the deduplicated vocabulary list
//!
//! The storage models use integer indices to reference vocabulary entries,
//! significantly reducing file size for projects with extensive repeated vocabulary.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single token representing a word or character within a text segment.
///
/// Tokens are the atomic units of text analysis. The actual gloss (meaning)
/// is stored separately in the [`Project`] vocabulary map to avoid duplication
/// when the same token appears multiple times.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Token {
    /// The original text of this token.
    pub original: String,
    /// Optional comment for this token (stored at runtime, not in JSON).
    #[serde(skip)]
    #[allow(dead_code)]
    pub comment: String,
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
