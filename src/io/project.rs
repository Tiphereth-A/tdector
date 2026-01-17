//! Project file loading, saving, and format conversion.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::models::{Project, SavedProject, SavedSentence, Segment, Token, VocabEntry};

/// Tries to auto-detect a font file in `dir` whose basename matches `stem`.
///
/// Supported extensions: `.ttf`, `.otf`, `.ttc`.
fn detect_font_in_dir(dir: &Path, stem: &str) -> Option<String> {
    if stem.is_empty() {
        return None;
    }

    let font_extensions = ["ttf", "otf", "ttc"];

    for ext in font_extensions {
        let font_file = dir.join(format!("{}.{}", stem, ext));
        if font_file.exists() {
            if let Some(path_str) = font_file.to_str() {
                return Some(path_str.to_string());
            }
        }
    }

    None
}

/// Reads a text file, returning `(content, project_name, font_path)`.
///
/// The project name is derived from the filename. A matching font file
/// (same name, `.ttf`/`.otf`/`.ttc` extension) is auto-detected.
pub fn read_text_content(path: &Path) -> Result<(String, String, Option<String>), String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

    let project_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let font_path = path.parent().and_then(|parent| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|stem| detect_font_in_dir(parent, stem))
    });

    Ok((content, project_name, font_path))
}

/// Segments text content into tokens.
///
/// If `use_whitespace_split` is true, splits on whitespace (word-based);
/// otherwise treats each character as a token (character-based).
pub fn segment_content(content: &str, use_whitespace_split: bool) -> Vec<Segment> {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let tokens: Vec<Token> = if use_whitespace_split {
                line.split_whitespace()
                    .map(|s| Token {
                        original: s.to_string(),
                    })
                    .collect()
            } else {
                line.chars()
                    .filter(|c| !c.is_whitespace())
                    .map(|c| Token {
                        original: c.to_string(),
                    })
                    .collect()
            };

            if tokens.is_empty() {
                None
            } else {
                Some(Segment {
                    tokens,
                    translation: String::new(),
                })
            }
        })
        .collect()
}

/// Loads a project from JSON, supporting current and legacy formats.
pub fn load_project_file(path: &Path) -> Result<Project, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

    if let Ok(saved) = serde_json::from_str::<SavedProject>(&content) {
        if let Some(project) = convert_from_saved_project(path, saved) {
            return Ok(project);
        }
        return Err(
            "Project data is corrupted: Reference to non-existent vocabulary entry.".to_string(),
        );
    }

    #[derive(Deserialize)]
    struct LegacyToken {
        original: String,
        #[serde(default)]
        gloss: String,
    }
    #[derive(Deserialize)]
    struct LegacySegment {
        tokens: Vec<LegacyToken>,
        translation: String,
    }
    #[derive(Deserialize)]
    struct LegacyProject {
        #[serde(default)]
        project_name: String,
        segments: Vec<LegacySegment>,
    }

    if let Ok(legacy) = serde_json::from_str::<LegacyProject>(&content) {
        let mut vocabulary = HashMap::new();
        let segments = legacy
            .segments
            .into_iter()
            .map(|seg| {
                let tokens = seg
                    .tokens
                    .into_iter()
                    .map(|tok| {
                        if !tok.gloss.is_empty() {
                            vocabulary.insert(tok.original.clone(), tok.gloss);
                        }
                        Token {
                            original: tok.original,
                        }
                    })
                    .collect();
                Segment {
                    tokens,
                    translation: seg.translation,
                }
            })
            .collect();

        return Ok(Project {
            project_name: legacy.project_name,
            font_path: None,
            vocabulary,
            segments,
        });
    }

    match serde_json::from_str::<serde_json::Value>(&content) {
        Ok(_) => Err("Unknown file format: Missing required fields ('vocabulary'/'sentences' or 'segments').".to_string()),
        Err(e) => Err(format!("File is not valid JSON: {}", e)),
    }
}

/// Converts a [`SavedProject`] (optimized serialization format) to a [`Project`] (runtime format).
///
/// Returns `None` if any word index references a non-existent vocabulary entry,
/// indicating corrupted project data.
fn convert_from_saved_project(path: &Path, saved: SavedProject) -> Option<Project> {
    // Reconstruct vocabulary map
    let vocabulary_map: HashMap<String, String> = saved
        .vocabulary
        .iter()
        .map(|entry| (entry.word.clone(), entry.meaning.clone()))
        .collect();

    // Convert sentences to segments
    let segments: Option<Vec<Segment>> = saved
        .sentences
        .into_iter()
        .map(|sentence| {
            let tokens: Option<Vec<Token>> = sentence
                .words
                .iter()
                .map(|&idx| {
                    saved.vocabulary.get(idx).map(|entry| Token {
                        original: entry.word.clone(),
                    })
                })
                .collect();

            tokens.map(|tokens| Segment {
                tokens,
                translation: sentence.meaning,
            })
        })
        .collect();

    let font_path = path
        .parent()
        .and_then(|dir| detect_font_in_dir(dir, &saved.project_name));

    Some(Project {
        project_name: saved.project_name,
        font_path,
        vocabulary: vocabulary_map,
        segments: segments?,
    })
}

/// Saves the project to a JSON file.
///
/// Converts the runtime [`Project`] format to the optimized [`SavedProject`]
/// format before serialization.
pub fn save_project_file(project: &Project, path: &Path) -> Result<(), String> {
    let saved_project = convert_to_saved_project(project)?;
    let content = serde_json::to_string_pretty(&saved_project)
        .map_err(|e| format!("Failed to serialize project: {}", e))?;

    fs::write(path, &content).map_err(|e| format!("Failed to save file: {}", e))?;

    Ok(())
}

/// Converts a [`Project`] (runtime format) to [`SavedProject`] (optimized serialization format).
///
/// Creates a deduplicated, sorted vocabulary array and replaces word strings
/// with indices, significantly reducing file size for large projects.
///
/// Returns an error if any token is missing from the vocabulary index (logic
/// bug or concurrent mutation); avoids silently dropping token data on save.
fn convert_to_saved_project(project: &Project) -> Result<SavedProject, String> {
    // Collect all unique words from vocabulary and segments
    let mut all_words: HashSet<&String> = project.vocabulary.keys().collect();

    for segment in &project.segments {
        for token in &segment.tokens {
            all_words.insert(&token.original);
        }
    }

    let mut sorted_words: Vec<&String> = all_words.into_iter().collect();
    sorted_words.sort();

    let mut word_to_idx: HashMap<String, usize> = HashMap::new();
    let mut vocabulary: Vec<VocabEntry> = Vec::with_capacity(sorted_words.len());

    for word in sorted_words {
        let idx = vocabulary.len();
        word_to_idx.insert(word.clone(), idx);

        let meaning = project.vocabulary.get(word).cloned().unwrap_or_default();
        vocabulary.push(VocabEntry {
            word: word.clone(),
            meaning,
        });
    }

    let sentences: Vec<SavedSentence> = project
        .segments
        .iter()
        .map(|segment| {
            let words: Vec<usize> = segment
                .tokens
                .iter()
                .map(|t| {
                    word_to_idx.get(&t.original).copied().ok_or_else(|| {
                        format!(
                            "Token '{}' missing from vocabulary index during save",
                            t.original
                        )
                    })
                })
                .collect::<Result<Vec<usize>, String>>()?;
            Ok(SavedSentence {
                words,
                meaning: segment.translation.clone(),
            })
        })
        .collect::<Result<Vec<SavedSentence>, String>>()?;

    Ok(SavedProject {
        version: 1,
        project_name: project.project_name.clone(),
        vocabulary,
        sentences,
    })
}
