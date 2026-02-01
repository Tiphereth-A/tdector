//! Core data models for runtime operations and persistent storage.
//!
//! This module defines two complementary sets of data structures:
//!
//! # Runtime Models
//!
//! - [`Project`] - Complete in-memory project state with vocabulary map
//! - [`Segment`] - A text segment with tokens and translation
//! - [`Token`] - Individual word or character unit
//! - [`FormationRule`] - A word formation rule (derivation, inflection, or nonmorphological)
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

/// The type of word formation rule.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum FormationType {
    /// Derivation: changing word class (e.g., verb to adjective)
    #[serde(rename = "derivation")]
    Derivation,
    /// Inflection: grammatical changes (e.g., pluralization)
    #[serde(rename = "inflection")]
    Inflection,
    /// Non-morphological changes (e.g., case conversion)
    #[serde(rename = "nonmorphological")]
    Nonmorphological,
}

/// Creates a configured Rhai engine with security constraints.
///
/// The engine is configured with:
/// - Expression depth limits to prevent deeply nested code
/// - Operation limits to prevent infinite loops
/// - Disabled I/O, network, and system operations for security
///
/// # Returns
///
/// A configured Rhai engine ready for script execution
pub fn get_engine() -> rhai::Engine {
    let mut engine = rhai::Engine::new();

    // Security constraints to prevent malicious scripts
    engine.set_max_expr_depths(5000, 5000);
    engine.set_max_operations(100000);

    // Disable all I/O operations
    engine.disable_symbol("eval");
    engine.disable_symbol("load");
    engine.disable_symbol("save");
    engine.disable_symbol("read");
    engine.disable_symbol("write");
    engine.disable_symbol("append");
    engine.disable_symbol("delete");
    engine.disable_symbol("copy");

    // Disable network operations
    engine.disable_symbol("http");
    engine.disable_symbol("request");
    engine.disable_symbol("fetch");
    engine.disable_symbol("socket");
    engine.disable_symbol("tcp");
    engine.disable_symbol("udp");

    // Disable system/process operations
    engine.disable_symbol("system");
    engine.disable_symbol("exec");
    engine.disable_symbol("spawn");
    engine.disable_symbol("command");

    engine
}

/// A word formation rule describing how to transform a word.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationRule {
    /// Human-readable description of the formation rule
    pub description: String,
    /// Type of formation (derivation, inflection, or nonmorphological)
    #[serde(rename = "type")]
    pub rule_type: FormationType,
    /// Rhai script command to apply the transformation
    pub command: String,
}

impl FormationRule {
    /// Execute the formation rule on a word using Rhai script engine.
    ///
    /// The Rhai script should define a `transform(word)` function that takes
    /// a string and returns the transformed word.
    ///
    /// # Arguments
    ///
    /// * `word` - The word to transform
    ///
    /// # Returns
    ///
    /// The transformed word if successful, or an error message if execution fails
    ///
    /// # Example
    ///
    /// For a rule with command: `fn transform(word) { word + "s" }`
    /// Calling `rule.apply("apple")` would return `Ok("apples".to_string())`
    pub fn apply(&self, word: &str) -> Result<String, String> {
        let engine = get_engine();

        // Compile and run the user's transformation script
        let ast = engine
            .compile(&self.command)
            .map_err(|e| format!("Rhai compilation error: {e}"))?;

        // Call the transform function with the word
        let result: String = engine
            .call_fn(
                &mut rhai::Scope::new(),
                &ast,
                "transform",
                (word.to_string(),),
            )
            .map_err(|e| format!("Transform function error: {e}"))?;

        Ok(result)
    }
}

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
    #[allow(dead_code)]
    pub comment: String,
    /// The base word this token came from (before formation rule was applied).
    /// Stored at runtime only, used for saving the correct WordRef.
    #[serde(skip)]
    pub base_word: Option<String>,
    /// Index of formation rule applied to this token (if any), stored at runtime only.
    /// This tracks that the token's text is the result of applying a formation rule.
    #[serde(skip)]
    pub formation_rule_idx: Option<usize>,
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

/// A word reference in a sentence - can be a single index or a word with applied formation rule.
///
/// This allows sentences to reference either:
/// - A single vocabulary entry (simple case): `4`
/// - A vocabulary entry with a formation rule applied: `[vocab_idx, rule_idx]`
///   where vocab_idx references the base word and rule_idx is the formation rule to apply
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WordRef {
    /// Single vocabulary index
    Single(usize),
    /// Word with applied formation rule: [vocabulary_index, rule_index]
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

    /// Get the rule index if this word has a formation rule applied
    pub fn rule_index(&self) -> Option<usize> {
        match self {
            WordRef::Single(_) => None,
            WordRef::WithRule(indices) => indices.get(1).copied(),
        }
    }
}

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
