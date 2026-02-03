use serde::{Deserialize, Serialize};

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
