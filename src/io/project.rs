//! Project persistence and format conversion.
//!
//! This module handles:
//! - Converting projects between runtime and storage formats
//! - Format migration between runtime and storage representations
//! - Text file import and tokenization

use std::collections::HashMap;

use crate::consts::domain::PROJECT_VERSION;
use crate::enums::{AppError, AppResult, WordRef};
use crate::libs::models::{Project, SavedProject, SavedSentence, Segment, Token, VocabEntry};

/// Tokenizes text content into segments.
///
/// Processes the text line-by-line, creating one segment per non-empty line.
/// The tokenization strategy depends on the script type:
///
/// # Arguments
///
/// * `content` - Raw text content to tokenize
/// * `use_whitespace_split` - If `true`, uses word-based tokenization (splits on whitespace);
///   if `false`, uses character-based tokenization (each character is a token)
///
/// # Returns
///
/// A vector of segments, each representing one line of the input text.
/// Empty lines and lines containing only whitespace are skipped.
///
/// # Tokenization Strategies
///
/// - **Word-based** (whitespace split): Suitable for space-delimited scripts
///   like English, Spanish, etc.
/// - **Character-based**: Necessary for scripts without clear word boundaries
///   like Chinese, Japanese, etc.
pub fn segment_content(content: &str, use_whitespace_split: bool) -> Vec<Segment> {
    // Delegate to domain layer's TextProcessor for consistent segmentation logic
    crate::libs::text_analysis::TextProcessor::segment_text(content, use_whitespace_split)
        .unwrap_or_else(|_| Vec::new())
}

/// Loads project JSON content with automatic format migration.

/// Converts from optimized storage format to runtime format.
///
/// Reconstructs the vocabulary `HashMap` and resolves all vocabulary indices
/// in sentences back to their token strings.
///
/// # Arguments
///
/// * `saved` - The deserialized storage-format project
///
/// # Returns
///
/// `Some(Project)` if all vocabulary indices are valid, `None` if any
/// index references a non-existent vocabulary entry (corrupted data).
pub fn convert_from_saved_project(mut saved: SavedProject) -> Option<Project> {
    for rule in &mut saved.formation {
        rule.cached_ast = crate::libs::formation::default_cached_ast();
    }

    let vocabulary_map: HashMap<String, String> = saved
        .vocabulary
        .iter()
        .map(|entry| (entry.word.clone(), entry.meaning.clone()))
        .collect();

    let vocabulary_comments: HashMap<String, String> = saved
        .vocabulary
        .iter()
        .map(|entry| (entry.word.clone(), entry.comment.clone()))
        .collect();

    let segments: Option<Vec<Segment>> = saved
        .sentences
        .into_iter()
        .map(|sentence| {
            let tokens: Option<Vec<Token>> = sentence
                .words
                .iter()
                .map(|word_ref| {
                    let vocab_idx = word_ref.vocab_index()?;
                    let base_word = saved.vocabulary.get(vocab_idx)?;
                    let formation_rule_indices = word_ref.rule_indices();

                    let original = if formation_rule_indices.is_empty() {
                        base_word.word.clone()
                    } else {
                        let mut current = base_word.word.clone();
                        for rule_idx in &formation_rule_indices {
                            if let Some(rule) = saved.formation.get(*rule_idx) {
                                current = rule.apply(&current).unwrap_or(current);
                            }
                        }
                        current
                    };

                    Some(Token {
                        original,
                        comment: base_word.comment.clone(),
                        base_word: Some(base_word.word.clone()),
                        formation_rule_indices,
                    })
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
        segments: segments?,
        formation_rules: saved.formation,
    })
}

/// Converts from runtime format to optimized storage format.
///
/// Creates a deduplicated, sorted vocabulary array and replaces all token
/// strings with integer indices. This transformation typically reduces file
/// size by 50-80% for projects with significant vocabulary reuse.
///
/// # Process
///
/// 1. Collects all unique words from vocabulary and segments
/// 2. Sorts words alphabetically using `BTreeSet`
/// 3. Assigns sequential indices to each word
/// 4. Replaces token strings with vocabulary indices
///
/// # Arguments
///
/// * `project` - The runtime project to convert
///
/// # Returns
///
/// The converted storage-format project, or an error if any token cannot
/// be mapped to a vocabulary index (indicates data corruption or race condition).
///
/// # Errors
///
/// Returns an error if any token's original text is missing from the index.
/// This should never occur in normal operation but protects against data loss
/// if there's a bug in vocabulary management.
pub fn convert_to_saved_project(project: &Project) -> AppResult<SavedProject> {
    let mut all_words: std::collections::BTreeSet<&String> = project.vocabulary.keys().collect();

    for segment in &project.segments {
        for token in &segment.tokens {
            if token.formation_rule_indices.is_empty() {
                all_words.insert(&token.original);
            }
        }
    }

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

    let mut sorted_formation_rules = project.formation_rules.clone();

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

    let sentences: Vec<SavedSentence> = project
        .segments
        .iter()
        .map(|segment| {
            let words: Vec<WordRef> = segment
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

                        WordRef::WithRule(indices)
                    } else {
                        WordRef::Single(vocab_idx)
                    };

                    Ok(word_ref)
                })
                .collect::<AppResult<Vec<WordRef>>>()?;
            Ok(SavedSentence {
                words,
                meaning: segment.translation.clone(),
                comment: segment.comment.clone(),
            })
        })
        .collect::<AppResult<Vec<SavedSentence>>>()?;

    Ok(SavedProject {
        version: PROJECT_VERSION,
        project_name: project.project_name.clone(),
        formation: sorted_formation_rules,
        vocabulary,
        sentences,
    })
}
