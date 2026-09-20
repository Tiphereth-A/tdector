/// Core business logic for the text decryption and translation tool
///
/// Key concepts:
/// - Project: The root container for all data (segments, vocabulary, rules)
/// - Segment: A logical unit of text containing tokens to be translated
/// - Token: Individual word or character, potentially derived from a base word
/// - `FormationRule`: Rhai script that transforms base words into derived forms
/// - `TokenizationRule`: Rhai script that splits text into tokens
///
/// This crate provides domain models, filtering, sorting, index types, and cache containers. Application use cases live in `tdector-app`; text analysis, script evaluation, and persistence live in their respective headless crates.
pub mod cache;
pub mod filtering;
pub mod sorting;
pub mod types;

pub use crate::project;
pub use crate::project::{Project, Segment, Token};
