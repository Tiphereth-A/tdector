//! Project persistence and format conversion.
//!
//! This module handles:
//! - Loading projects from JSON files (current and legacy formats)
//! - Saving projects with optimized vocabulary indexing
//! - Format migration between runtime and storage representations
//! - Font file auto-detection for custom scripts
//! - Text file import and tokenization

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::models::{Project, SavedProject, SavedSentence, Segment, Token, VocabEntry};

/// Attempts to auto-detect a matching font file in the specified directory.
///
/// Searches for a font file with the given basename (stem) and a supported
/// extension. This enables automatic font loading when a project file or
/// text file has an accompanying font file with the same name.
///
/// # Arguments
///
/// * `dir` - Directory to search for font files
/// * `stem` - Basename to match (without extension)
///
/// # Returns
///
/// The absolute path to the font file if found, otherwise `None`.
///
/// # Supported Font Formats
///
/// - `.ttf` - TrueType Font
/// - `.otf` - OpenType Font
/// - `.ttc` - TrueType Collection
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

/// Reads a text file and prepares it for import.
///
/// Loads the text content and automatically derives the project name from the
/// filename. Also attempts to detect an accompanying font file with the same
/// basename in the same directory.
///
/// # Arguments
///
/// * `path` - Path to the text file to import
///
/// # Returns
///
/// A tuple containing:
/// - File contents as a string
/// - Derived project name (from filename, defaults to "Untitled")
/// - Optional path to auto-detected font file
///
/// # Errors
///
/// Returns an error message string if the file cannot be read.
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

/// Loads a project from a JSON file with automatic format migration.
///
/// Attempts to load the project using multiple format parsers in order:
/// 1. Current indexed vocabulary format ([`SavedProject`])
/// 2. Legacy format with inline glosses
///
/// This enables seamless migration from older project file formats without
/// requiring manual conversion.
///
/// # Arguments
///
/// * `path` - Path to the project JSON file
///
/// # Returns
///
/// The loaded project in runtime format, or an error message describing
/// why the file could not be loaded.
///
/// # Errors
///
/// - File read errors
/// - JSON parse errors
/// - Corrupted data (e.g., invalid vocabulary indices)
/// - Unrecognized format
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

/// Converts from optimized storage format to runtime format.
///
/// Reconstructs the vocabulary HashMap and resolves all vocabulary indices
/// in sentences back to their token strings.
///
/// # Arguments
///
/// * `path` - Path to the project file (used for font detection)
/// * `saved` - The deserialized storage-format project
///
/// # Returns
///
/// `Some(Project)` if all vocabulary indices are valid, `None` if any
/// index references a non-existent vocabulary entry (corrupted data).
fn convert_from_saved_project(path: &Path, saved: SavedProject) -> Option<Project> {
    let vocabulary_map: HashMap<String, String> = saved
        .vocabulary
        .iter()
        .map(|entry| (entry.word.clone(), entry.meaning.clone()))
        .collect();

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

/// Persists a project to disk in optimized JSON format.
///
/// Converts the runtime [`Project`] representation to the space-efficient
/// [`SavedProject`] format before serialization. The resulting JSON file
/// uses vocabulary indexing to minimize redundancy.
///
/// # Arguments
///
/// * `project` - The project to save
/// * `path` - Destination file path
///
/// # Returns
///
/// `Ok(())` on success, or an error message string describing the failure.
///
/// # Errors
///
/// - Serialization failures
/// - File write errors
/// - Disk space issues
pub fn save_project_file(project: &Project, path: &Path) -> Result<(), String> {
    let saved_project = convert_to_saved_project(project)?;
    let content = serde_json::to_string_pretty(&saved_project)
        .map_err(|e| format!("Failed to serialize project: {}", e))?;

    fs::write(path, &content).map_err(|e| format!("Failed to save file: {}", e))?;

    Ok(())
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
/// 2. Sorts words alphabetically using BTreeSet
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
fn convert_to_saved_project(project: &Project) -> Result<SavedProject, String> {
    let mut all_words: std::collections::BTreeSet<&String> = project.vocabulary.keys().collect();

    for segment in &project.segments {
        for token in &segment.tokens {
            all_words.insert(&token.original);
        }
    }

    let mut word_to_idx: HashMap<&str, usize> = HashMap::with_capacity(all_words.len());
    let mut vocabulary: Vec<VocabEntry> = Vec::with_capacity(all_words.len());

    for word in all_words {
        let idx = vocabulary.len();
        word_to_idx.insert(word.as_str(), idx);

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
                    word_to_idx
                        .get(t.original.as_str())
                        .copied()
                        .ok_or_else(|| {
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
