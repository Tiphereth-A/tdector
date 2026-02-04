use crate::enums::AppResult;
use crate::libs::{Project, Segment, Token};

/// Text processing utility for tokenizing and analyzing text content.
pub struct TextProcessor;

impl TextProcessor {
    /// Split text into segments, where each segment contains tokens.
    /// Tokenization mode is determined by tokenize_by_word parameter.
    /// Empty lines are skipped; other lines become separate segments.
    pub fn segment_text(text: &str, tokenize_by_word: bool) -> AppResult<Vec<Segment>> {
        let lines: Vec<&str> = text.lines().collect();
        let mut segments = Vec::new();

        for line in lines {
            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Create a segment by tokenizing this line
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

    /// Split a line into word tokens using whitespace as delimiter
    fn segment_by_word(line: &str) -> Segment {
        let tokens = line
            .split_whitespace()
            .map(|word| Token {
                original: word.to_string(),
                base_word: None,
                formation_rule_indices: Vec::new(),
            })
            .collect();

        Segment {
            tokens,
            translation: String::new(),
            comment: String::new(),
        }
    }

    /// Split a line into character tokens, preserving each character as an individual token
    fn segment_by_character(line: &str) -> Segment {
        let tokens = line
            .chars()
            .map(|ch| Token {
                original: ch.to_string(),
                base_word: None,
                formation_rule_indices: Vec::new(),
            })
            .collect();

        Segment {
            tokens,
            translation: String::new(),
            comment: String::new(),
        }
    }

    /// Calculate what percentage of a segment has been translated.
    /// Returns 1.0 if translation is present and non-empty, 0.0 otherwise.
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

    /// Count how many tokens in a segment have vocabulary definitions.
    /// A token is considered translated if its original form exists in the project vocabulary
    /// and has a non-empty definition.
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
