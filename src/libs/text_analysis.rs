//! Domain text processing and linguistic analysis.
//!
//! Implements text segmentation, tokenization, and linguistic analysis operations
//! like computing vocabulary statistics and translation ratios.

use crate::enums::AppResult;
use crate::libs::models::{Project, Segment, Token};

/// Text processing engine for tokenization and analysis.
pub struct TextProcessor;

impl TextProcessor {
    /// Segments text into individual units based on the specified strategy.
    ///
    /// # Arguments
    ///
    /// * `text` - The input text to segment
    /// * `tokenize_by_word` - If true, splits by whitespace; if false, by characters
    ///
    /// # Returns
    ///
    /// A vector of text segments ready for translation
    pub fn segment_text(text: &str, tokenize_by_word: bool) -> AppResult<Vec<Segment>> {
        let lines: Vec<&str> = text.lines().collect();
        let mut segments = Vec::new();

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let segment = if tokenize_by_word {
                Self::segment_by_word(line)
            } else {
                Self::segment_by_character(line)
            };

            if !segment.tokens.is_empty() {
                segments.push(segment);
            }
        }

        Ok(segments)
    }

    /// Creates a segment by splitting on whitespace (word-based tokenization).
    fn segment_by_word(line: &str) -> Segment {
        let tokens = line
            .split_whitespace()
            .map(|word| Token {
                original: word.to_string(),
                base_word: None,
                formation_rule_indices: Vec::new(),
                comment: String::new(),
            })
            .collect();

        Segment {
            tokens,
            translation: String::new(),
            comment: String::new(),
        }
    }

    /// Creates a segment by splitting into characters (character-based tokenization).
    fn segment_by_character(line: &str) -> Segment {
        let tokens = line
            .chars()
            .map(|ch| Token {
                original: ch.to_string(),
                base_word: None,
                formation_rule_indices: Vec::new(),
                comment: String::new(),
            })
            .collect();

        Segment {
            tokens,
            translation: String::new(),
            comment: String::new(),
        }
    }

    /// Calculates the translation completion ratio for a segment.
    ///
    /// Returns a value between 0.0 and 1.0 representing the percentage of
    /// tokens that have translations in the project vocabulary.
    pub fn calculate_translation_ratio(segment: &Segment) -> f32 {
        if segment.tokens.is_empty() {
            return 0.0;
        }
        if !segment.translation.is_empty() {
            1.0
        } else {
            0.0
        }
    }

    /// Counts the number of tokens with vocabulary entries in a segment.
    ///
    /// Returns the count of tokens that have non-empty vocabulary definitions.
    pub fn count_segment_translated_tokens(segment: &Segment, project: &Project) -> usize {
        segment
            .tokens
            .iter()
            .filter(|token| {
                project
                    .vocabulary
                    .get(&token.original)
                    .map(|def| !def.trim().is_empty())
                    .unwrap_or(false)
            })
            .count()
    }
}
