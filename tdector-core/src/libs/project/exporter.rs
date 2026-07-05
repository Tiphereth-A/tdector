use std::collections::HashMap;

use super::models::{
    FormattedWordEntry, Project, SavedProjectV2, SavedSentenceV2, SavedVocabularyV2, VocabEntry,
};
use crate::consts::domain::PROJECT_VERSION;
use crate::enums::{AppError, AppResult};

/// Convert a runtime Project to its serializable `SavedProjectV2` format for JSON export.
/// This handles:
/// 1. Deduplicating vocabulary across all segments
/// 2. Mapping token references to vocabulary indices
/// 3. Preserving word formation rules and their application chains
/// 4. Handling both base words and derived words with their rule histories
pub fn convert_to_saved_project(project: &Project) -> AppResult<SavedProjectV2> {
    // Collect all unique vocabulary words from the project (for deduplication)
    let mut all_words: std::collections::BTreeSet<&String> = project.vocabulary.keys().collect();

    // Add any tokens that have formation rules applied but aren't in main vocabulary
    for segment in &project.segments {
        for token in &segment.tokens {
            if token.formation_rule_indices.is_empty() {
                all_words.insert(&token.original);
            }
        }
    }

    // Build a mapping of word strings to vocabulary indices
    let mut word_to_idx: HashMap<&str, usize> = HashMap::with_capacity(all_words.len());
    let mut vocabulary: Vec<VocabEntry> = Vec::with_capacity(all_words.len());

    for word in all_words {
        let idx = vocabulary.len();
        word_to_idx.insert(word.as_str(), idx);

        let meaning = project.vocabulary.get(word).cloned().unwrap_or_default();
        let comment = project
            .vocabulary_comments
            .get(word)
            .cloned()
            .unwrap_or_default();
        vocabulary.push(VocabEntry {
            word: word.clone(),
            meaning,
            comment,
        });
    }

    // Clone formation rules for sorting
    let mut sorted_formation_rules = project.formation_rules.clone();

    // Build a mapping of old rule indices to new indices after sorting for deterministic output
    let mut old_to_new_idx: HashMap<usize, usize> = HashMap::new();
    let mut indexed_rules: Vec<(usize, _)> = project.formation_rules.iter().enumerate().collect();
    indexed_rules.sort_by(|(_, a), (_, b)| match a.rule_type.cmp(&b.rule_type) {
        std::cmp::Ordering::Equal => a.description.cmp(&b.description),
        other => other,
    });

    for (new_idx, (old_idx, _)) in indexed_rules.iter().enumerate() {
        old_to_new_idx.insert(*old_idx, new_idx);
    }

    sorted_formation_rules.sort_by(|a, b| match a.rule_type.cmp(&b.rule_type) {
        std::cmp::Ordering::Equal => a.description.cmp(&b.description),
        other => other,
    });

    // Collect all unique formatted (derived) words and their rule chains
    let mut formatted_word_entries: Vec<FormattedWordEntry> = Vec::new();
    let mut seen_formatted_words: HashMap<Vec<usize>, bool> = HashMap::new();

    for segment in &project.segments {
        for token in &segment.tokens {
            if token.formation_rule_indices.is_empty() {
                continue;
            }

            // Get the base word for this derived token
            let base_word = token.base_word.as_ref().unwrap_or(&token.original).as_str();
            let vocab_idx = match word_to_idx.get(base_word).copied() {
                Some(idx) => idx,
                None => continue,
            };

            // Build the rule index chain: [vocab_idx, rule_idx_1, rule_idx_2, ...]
            let mut indices = Vec::with_capacity(1 + token.formation_rule_indices.len());
            indices.push(vocab_idx);

            for rule_idx in &token.formation_rule_indices {
                let new_rule_idx = match old_to_new_idx.get(rule_idx).copied() {
                    Some(idx) => idx,
                    None => {
                        indices.clear();
                        break;
                    }
                };
                indices.push(new_rule_idx);
            }

            if indices.is_empty() || indices.len() < 2 {
                continue;
            }

            let indices_clone = indices.clone();
            // Avoid duplicate formatted word entries
            seen_formatted_words.entry(indices).or_insert_with(|| {
                let comment = project
                    .formatted_word_comments
                    .get(&token.original)
                    .cloned()
                    .unwrap_or_default();

                formatted_word_entries.push(FormattedWordEntry {
                    word: indices_clone.clone(),
                    comment,
                });
                true
            });
        }
    }

    formatted_word_entries.sort_by(|a, b| a.word.cmp(&b.word));

    // Build a mapping of formatted word index chains to their position in the formatted_word_entries
    let mut formatted_word_map: HashMap<Vec<usize>, usize> = HashMap::new();
    for (idx, entry) in formatted_word_entries.iter().enumerate() {
        formatted_word_map.insert(entry.word.clone(), idx);
    }

    // Convert segments to the serializable format, resolving token references
    let sentences: Vec<SavedSentenceV2> = project
        .segments
        .iter()
        .map(|segment| {
            let words: Vec<i64> = segment
                .tokens
                .iter()
                .map(|t| {
                    let lookup_word = t.base_word.as_ref().unwrap_or(&t.original);

                    let vocab_idx = word_to_idx.get(lookup_word.as_str()).copied().ok_or_else(
                        || {
                            AppError::InvalidProjectFormat(
                                format!(
                                    "Token '{lookup_word}' missing from vocabulary index during save"
                                )
                            )
                        },
                    )?;

                    // Convert token to word reference: positive for base words, negative for derived
                    let word_ref = if !t.formation_rule_indices.is_empty() {
                        let mut indices = Vec::with_capacity(1 + t.formation_rule_indices.len());
                        indices.push(vocab_idx);

                        for rule_idx in &t.formation_rule_indices {
                            let new_rule_idx = old_to_new_idx.get(rule_idx).copied().ok_or_else(
                                || {
                                    AppError::InvalidProjectFormat(
                                        format!(
                                            "Formation rule index {rule_idx} not found in mapping during save"
                                        )
                                    )
                                },
                            )?;
                            indices.push(new_rule_idx);
                        }

                        // Negative encoding for formatted words: -(index + 1)
                        let formatted_idx = formatted_word_map.get(&indices).copied().ok_or_else(
                            || {
                                AppError::InvalidProjectFormat(
                                    format!(
                                        "Formatted word with indices {indices:?} not found in formatted_word_map during save"
                                    )
                                )
                            },
                        )?;
                        -((formatted_idx as i64) + 1)
                    } else {
                        // Positive encoding for base words
                        vocab_idx as i64
                    };

                    Ok(word_ref)
                })
                .collect::<AppResult<Vec<i64>>>()?;
            Ok(SavedSentenceV2 {
                words,
                meaning: segment.translation.clone(),
                comment: segment.comment.clone(),
            })
        })
        .collect::<AppResult<Vec<SavedSentenceV2>>>()?;

    Ok(SavedProjectV2 {
        version: PROJECT_VERSION,
        project_name: project.project_name.clone(),
        formation: sorted_formation_rules,
        vocabulary: SavedVocabularyV2 {
            original: vocabulary,
            formatted: formatted_word_entries,
        },
        sentences,
    })
}
