use serde::{Deserialize, Serialize};

/// Category of word formation rule being applied
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum FormationType {
    /// Rule produces related word forms through morphological derivation (e.g., noun -> verb)
    #[serde(rename = "derivation")]
    Derivation,

    /// Rule produces inflected forms of the same word (e.g., singular -> plural, tense changes)
    #[serde(rename = "inflection")]
    Inflection,

    /// Rule produces forms through non-morphological transformations (e.g., phonetic, script)
    #[serde(rename = "nonmorphological")]
    Nonmorphological,
}
