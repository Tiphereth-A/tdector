use crate::enums::AppResult;
use crate::libs::eval::TokenizationRule;
use crate::libs::{Project, Segment, Token};

/// Text processing utility for tokenizing and analyzing text content.
pub struct TextProcessor;

impl TextProcessor {
    /// Split text into segments using the provided tokenization rule.
    /// Empty lines are skipped; other lines become separate segments.
    pub fn segment_text_with_rule(
        text: &str,
        tokenization_rule: Option<&TokenizationRule>,
    ) -> AppResult<Vec<Segment>> {
        let lines: Vec<&str> = text.lines().collect();
        let mut segments = Vec::new();

        for line in lines {
            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Get tokenization rule (fail if none provided)
            let rule = tokenization_rule.ok_or_else(|| {
                crate::enums::AppError::InvalidProjectFormat(
                    "No tokenization rule provided".to_string(),
                )
            })?;

            // Tokenize the line using the Rhai script
            let token_strings = rule.tokenize(line)?;

            // Convert token strings to Token objects
            let tokens = token_strings
                .into_iter()
                .map(|text| Token {
                    original: text,
                    base_word: None,
                    formation_rule_indices: Vec::new(),
                })
                .collect();

            // Create and add segment if it has tokens
            let segment = Segment {
                tokens,
                translation: String::new(),
                comment: String::new(),
            };

            if !segment.tokens.is_empty() {
                segments.push(segment);
            }
        }

        Ok(segments)
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
