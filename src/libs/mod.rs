//! Domain layer: Encapsulated business logic and algorithms.
//!
//! Pure business logic separated from UI and infrastructure concerns.
//! Implements linguistic algorithms and domain-driven validation:
//!
//! - **cache**: Memoization for expensive computations (LookupCache, CachedTfidf)
//! - **filtering**: Text pattern matching and segment filtering
//! - **menu**: Context menu generation for vocabulary and linguistic operations
//! - **similarity**: TF-IDF similarity ranking and nearest-neighbor search
//! - **text_analysis**: Tokenization, normalization, and linguistic processing
//! - **types**: Domain-specific type wrappers for type safety
//! - **validation**: Business rule enforcement and data constraint validation
//!
//! Note: Constants have been moved to the `consts` module.

pub mod cache;
pub mod filtering;
pub mod formation;
pub mod models;
pub mod similarity;
pub mod sorting;
pub mod text_analysis;
pub mod types;

pub use models::{Project, Segment, Token};
