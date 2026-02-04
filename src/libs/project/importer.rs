use std::collections::HashMap;

use serde_json::Value;

use crate::consts::domain::PROJECT_VERSION;

use super::models::{Project, SavedProjectV2, Segment, Token};

use super::update_v1::migrate_v1_to_v2;

/// Segment raw text content into logical units (segments) and tokens.
/// Tokens can be split by whitespace (words) or by character depending on use_whitespace_split.
pub fn segment_content(content: &str, use_whitespace_split: bool) -> Vec<Segment> {
    crate::libs::text_analysis::TextProcessor::segment_text(content, use_whitespace_split)
        .unwrap_or_else(|_| Vec::new())
}

/// Migrate a JSON value from any supported version to the current PROJECT_VERSION.
/// Currently supports v1 -> v2 migration. Returns an error if version is unsupported.
pub fn migrate_to_latest(mut value: Value) -> Result<SavedProjectV2, String> {
    let mut version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0);

    if version < 1 || version > PROJECT_VERSION {
        return Err(format!("Unsupported project version: {version}"));
    }

    // Migrate from v1 to v2 if needed
    if version == 1 {
        match migrate_v1_to_v2(value) {
            Ok(migrated) => {
                value = migrated;
                version = 2;
            }
            Err(e) => {
                return Err(format!("Failed to migrate project from v1 to v2: {e}"));
            }
        }
    }

    if version != PROJECT_VERSION {
        return Err(format!(
            "Migration failed: target version = {PROJECT_VERSION}, result version = {version}"
        ));
    }

    serde_json::from_value(value).map_err(|e| format!("Failed to parse migrated project: {e}"))
}

/// Load a complete Project from a JSON value, handling version migration and format conversion.
/// This is the main entry point for loading projects from saved JSON files.
pub fn load_project_from_json(value: Value) -> Result<Project, String> {
    let saved_project = migrate_to_latest(value)?;
    convert_from_saved_project_v2(saved_project)
        .ok_or_else(|| "Failed to convert project format".to_string())
}

pub fn convert_from_saved_project_v2(mut saved: SavedProjectV2) -> Option<Project> {
    // Initialize cached ASTs for all formation rules (needed for script execution)
    for rule in &mut saved.formation {
        rule.cached_ast = crate::libs::formation::default_cached_ast();
    }

    // Build a map of formatted words (derived words created by applying rules) to their comments
    let mut formatted_word_comments: HashMap<String, String> = HashMap::new();
    for entry in &saved.vocabulary.formatted {
        // Extract base vocabulary index and rule indices from the word index chain
        let Some((vocab_idx, rule_indices)) = entry.word.split_first() else {
            continue;
        };

        // Reconstruct the formatted word by applying rules sequentially to the base word
        let base_word = match saved.vocabulary.original.get(*vocab_idx) {
            Some(word) => word.word.clone(),
            None => continue,
        };

        if rule_indices.is_empty() {
            continue;
        }

        let mut formatted_word = base_word;
        for rule_idx in rule_indices {
            let Some(rule) = saved.formation.get(*rule_idx) else {
                formatted_word.clear();
                break;
            };
            formatted_word = rule.apply(&formatted_word).unwrap_or(formatted_word);
        }

        if !formatted_word.is_empty() {
            formatted_word_comments.insert(formatted_word, entry.comment.clone());
        }
    }

    // Build vocabulary maps from the saved format
    let vocabulary_map: HashMap<String, String> = saved
        .vocabulary
        .original
        .iter()
        .map(|entry| (entry.word.clone(), entry.meaning.clone()))
        .collect();

    let vocabulary_comments: HashMap<String, String> = saved
        .vocabulary
        .original
        .iter()
        .map(|entry| (entry.word.clone(), entry.comment.clone()))
        .collect();

    // Convert saved segments back to the runtime Segment format, resolving word references
    let segments: Option<Vec<Segment>> = saved
        .sentences
        .into_iter()
        .map(|sentence| {
            let tokens: Option<Vec<Token>> = sentence
                .words
                .iter()
                .map(|word_ref| {
                    if *word_ref >= 0 {
                        // Positive reference: base vocabulary word
                        let vocab_idx = *word_ref as usize;
                        let base_word = saved.vocabulary.original.get(vocab_idx)?;
                        Some(Token {
                            original: base_word.word.clone(),
                            base_word: Some(base_word.word.clone()),
                            formation_rule_indices: Vec::new(),
                        })
                    } else {
                        // Negative reference: derived word (-(index + 1))
                        let formatted_idx = (-*word_ref - 1) as usize;
                        let entry = saved.vocabulary.formatted.get(formatted_idx)?;
                        let vocab_idx = entry.word.first().copied()?;
                        let base_word = saved.vocabulary.original.get(vocab_idx)?;
                        let rule_indices: Vec<usize> = entry.word.iter().skip(1).copied().collect();

                        // Reconstruct the original derived form by applying rules
                        let mut original = base_word.word.clone();
                        for rule_idx in &rule_indices {
                            if let Some(rule) = saved.formation.get(*rule_idx) {
                                original = rule.apply(&original).unwrap_or(original);
                            }
                        }

                        Some(Token {
                            original,
                            base_word: Some(base_word.word.clone()),
                            formation_rule_indices: rule_indices,
                        })
                    }
                })
                .collect();

            tokens.map(|tokens| Segment {
                tokens,
                translation: sentence.meaning,
                comment: sentence.comment,
            })
        })
        .collect();

    Some(Project {
        project_name: saved.project_name,
        font_path: None,
        vocabulary: vocabulary_map,
        vocabulary_comments,
        formatted_word_comments,
        segments: segments?,
        formation_rules: saved.formation,
    })
}
