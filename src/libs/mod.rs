/// Core business logic for the text decryption and translation tool
///
/// Key concepts:
/// - Project: The root container for all data (segments, vocabulary, rules)
/// - Segment: A logical unit of text containing tokens to be translated
/// - Token: Individual word or character, potentially derived from a base word
/// - `FormationRule`: Rhai script that transforms base words into derived forms
/// - `TokenizationRule`: Rhai script that splits text into tokens
///
/// The library provides:
/// - Text analysis: Tokenization and translation ratio calculations
/// - Caching: Lookup maps and TF-IDF matrices for performance
/// - Filtering: Full-text search across segments and translations
/// - Sorting: Multiple sort criteria for segment ordering
/// - Similarity: TF-IDF based semantic search (native only)
/// - Script evaluation: Safe Rhai-based execution for word transformations and tokenization
/// - Project I/O: Serialization with version migration support
pub mod cache;
pub mod eval;
pub mod filtering;
pub mod project;
pub mod similarity;
pub mod sorting;
pub mod text_analysis;
pub mod types;

pub use project::{Project, Segment, Token};
