//! Data models: Core runtime structures and persistence schemas.
//!
//! Defines all application data structures representing domain concepts.
//! Includes both runtime models and serializable schemas for persistence.

pub mod models;

pub use models::{Project, Segment, Token};
pub use crate::lib::formation::{FormationRule, default_cached_ast, with_engine};
