use std::collections::HashMap;

use serde_json::Value;
use tdector_eval::{AppError, AppResult};

use super::models::{Project, SavedProjectV2, Segment, Token};
use super::update_v1::migrate_v1_to_v2;

const PROJECT_VERSION: u64 = 2;

/// Migrate a JSON value from any supported version to the current project format.
pub fn migrate_to_latest(mut value: Value) -> AppResult<SavedProjectV2> {
    let version = value.get("version").and_then(Value::as_u64).unwrap_or(0);
    if !(1..=PROJECT_VERSION).contains(&version) {
        return Err(AppError::InvalidProjectFormat(format!(
            "Unsupported project version: {version}"
        )));
    }
    if version == 1 {
        value = migrate_v1_to_v2(value)?;
    }
    serde_json::from_value(value).map_err(|error| {
        AppError::InvalidProjectFormat(format!("Failed to parse project: {error}"))
    })
}

/// Load a project, reporting invalid references and rule failures before returning any state.
pub fn load_project_from_json(value: Value) -> AppResult<Project> {
    try_convert_from_saved_project_v2(migrate_to_latest(value)?)
}

/// Compatibility wrapper for callers that only need success or failure.
/// New callers should use [`try_convert_from_saved_project_v2`] for diagnostics.
pub fn convert_from_saved_project_v2(saved: SavedProjectV2) -> Option<Project> {
    try_convert_from_saved_project_v2(saved).ok()
}

/// Validate and reconstruct a saved project without silently discarding invalid data.
pub fn try_convert_from_saved_project_v2(mut saved: SavedProjectV2) -> AppResult<Project> {
    if saved.version != PROJECT_VERSION {
        return Err(AppError::InvalidProjectFormat(format!(
            "Expected project version {PROJECT_VERSION}, got {}",
            saved.version
        )));
    }
    for rule in &mut saved.formation {
        rule.cached_ast = tdector_eval::default_cached_ast();
    }

    // Resolve each derived entry once, including entries not used by a sentence.
    let mut formatted_tokens = Vec::with_capacity(saved.vocabulary.formatted.len());
    let mut formatted_word_comments = HashMap::new();
    for (entry_idx, entry) in saved.vocabulary.formatted.iter().enumerate() {
        let Some((vocab_idx, rule_indices)) = entry.word.split_first() else {
            return Err(AppError::InvalidProjectFormat(format!(
                "Formatted word {entry_idx} has an empty index chain"
            )));
        };
        if rule_indices.is_empty() {
            return Err(AppError::InvalidProjectFormat(format!(
                "Formatted word {entry_idx} has no formation rule"
            )));
        }
        let base_word = saved.vocabulary.original.get(*vocab_idx).ok_or_else(|| {
            AppError::InvalidProjectFormat(format!(
                "Formatted word {entry_idx} references missing vocabulary index {vocab_idx}"
            ))
        })?;
        let mut original = base_word.word.clone();
        for rule_idx in rule_indices {
            let rule = saved.formation.get(*rule_idx).ok_or_else(|| {
                AppError::InvalidProjectFormat(format!(
                    "Formatted word {entry_idx} references missing formation rule {rule_idx}"
                ))
            })?;
            original = rule.apply(&original).map_err(|error| {
                AppError::ScriptExecutionError(format!(
                    "Formatted word {entry_idx}, formation rule {rule_idx}: {error}"
                ))
            })?;
        }
        formatted_word_comments.insert(original.clone(), entry.comment.clone());
        formatted_tokens.push(Token {
            original,
            base_word: Some(base_word.word.clone()),
            formation_rule_indices: rule_indices.to_vec(),
        });
    }

    let mut segments = Vec::with_capacity(saved.sentences.len());
    for (sentence_idx, sentence) in saved.sentences.into_iter().enumerate() {
        let mut tokens = Vec::with_capacity(sentence.words.len());
        for (token_idx, word_ref) in sentence.words.into_iter().enumerate() {
            let invalid_reference = || {
                AppError::InvalidProjectFormat(format!(
                    "Sentence {sentence_idx}, token {token_idx}: invalid word reference {word_ref}"
                ))
            };
            let token = if word_ref >= 0 {
                let vocab_idx = usize::try_from(word_ref).map_err(|_| invalid_reference())?;
                let base_word = saved
                    .vocabulary
                    .original
                    .get(vocab_idx)
                    .ok_or_else(invalid_reference)?;
                Token {
                    original: base_word.word.clone(),
                    base_word: Some(base_word.word.clone()),
                    formation_rule_indices: Vec::new(),
                }
            } else {
                // Add before negating so i64::MIN cannot overflow.
                let formatted_idx =
                    usize::try_from(-(word_ref + 1)).map_err(|_| invalid_reference())?;
                formatted_tokens
                    .get(formatted_idx)
                    .ok_or_else(invalid_reference)?
                    .clone()
            };
            tokens.push(token);
        }
        segments.push(Segment {
            tokens,
            translation: sentence.meaning,
            comment: sentence.comment,
        });
    }

    let vocabulary = saved
        .vocabulary
        .original
        .iter()
        .map(|entry| (entry.word.clone(), entry.meaning.clone()))
        .collect();
    let vocabulary_comments = saved
        .vocabulary
        .original
        .into_iter()
        .map(|entry| (entry.word, entry.comment))
        .collect();

    Ok(Project {
        project_name: saved.project_name,
        vocabulary,
        vocabulary_comments,
        formatted_word_comments,
        segments,
        formation_rules: saved.formation,
    })
}
