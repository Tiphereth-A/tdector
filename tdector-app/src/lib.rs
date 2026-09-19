//! Shared application operations for desktop, CLI, and protocol adapters.
//!
//! A session has one owner. Rhai rules contain local AST caches, so `Session` is
//! intentionally not `Send` or `Sync`. A concurrent adapter should send commands
//! to the owning thread instead of sharing a mutable project between workers.

pub mod api;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tdector_core::enums::{CommentTarget, SortMode};
use tdector_core::libs::cache::{CachedTfidf, LookupMap};
use tdector_core::libs::filtering::FilterOperation;
use tdector_core::libs::sorting::SortOperation;
use tdector_core::libs::{Project, Token};
use tdector_eval::{AppError, FormationRule, FormationType, TokenizationRule};
use tdector_text::similarity_sentence::SimilarityEngine;
pub use tdector_text::similarity_token::SimilarToken;
use tdector_text::text_analysis::TextProcessor;

static NEXT_SESSION_ID: AtomicUsize = AtomicUsize::new(1);

/// Errors that adapters can translate into their own presentation or protocol.
#[derive(Debug, Clone)]
pub enum Error {
    InvalidInput(String),
    InvalidIndex { kind: &'static str, index: usize },
    Operation(AppError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => f.write_str(message),
            Self::InvalidIndex { kind, index } => write!(f, "Invalid {kind} index: {index}"),
            Self::Operation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Operation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AppError> for Error {
    fn from(error: AppError) -> Self {
        Self::Operation(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// An immutable lookup index that is cheap to share between rendered views.
pub type LookupIndex = Option<Arc<LookupMap>>;
pub type LookupSnapshot = (LookupIndex, LookupIndex);

/// Project mutations shared by every adapter. UI state is deliberately absent.
#[derive(Debug, Clone)]
pub enum Command {
    SetGloss {
        word: String,
        meaning: String,
    },
    SetTranslation {
        segment: usize,
        translation: String,
    },
    SetWordComment {
        target: CommentTarget,
        comment: String,
    },
    SetSegmentComment {
        segment: usize,
        comment: String,
    },
    CreateFormationRule {
        description: String,
        rule_type: FormationType,
        command: String,
    },
    ApplyFormationRule {
        word: String,
        base_word: String,
        rule: usize,
    },
    RemoveFormationRule {
        segment: usize,
        token: usize,
    },
}

/// Identifies exactly the session, project generation, and revision serialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveToken {
    session: usize,
    generation: u64,
    revision: u64,
}

#[derive(Debug)]
pub struct SaveSnapshot {
    pub bytes: Vec<u8>,
    pub token: SaveToken,
}

#[derive(Debug, Clone)]
pub struct FormationStep {
    pub description: String,
    pub rule_type: FormationType,
    pub result: String,
}

/// Semantic token details shared by visual and headless adapters.
#[derive(Debug, Clone)]
pub struct TokenAnnotation {
    pub base_word: String,
    pub base_gloss: String,
    pub formation_descriptions: Vec<String>,
    pub is_derived: bool,
    /// The formatted-word comment when present, otherwise the base-word comment.
    pub display_comment: String,
    pub comment_target: CommentTarget,
    /// Only the target's own comment; an inherited display comment is not copied.
    pub editable_comment: String,
}

/// Resolve vocabulary, formation descriptions, and comment ownership without UI state.
pub fn annotate_token(project: &Project, token: &Token) -> TokenAnnotation {
    let base_word = token.base_word.as_ref().unwrap_or(&token.original);
    let base_comment = project
        .vocabulary_comments
        .get(base_word)
        .cloned()
        .unwrap_or_default();
    let is_derived = !token.formation_rule_indices.is_empty();
    let (comment_target, editable_comment) = if is_derived {
        (
            CommentTarget::FormattedWord(token.original.clone()),
            project
                .formatted_word_comments
                .get(&token.original)
                .cloned()
                .unwrap_or_default(),
        )
    } else {
        (
            CommentTarget::BaseWord(base_word.clone()),
            base_comment.clone(),
        )
    };
    let display_comment = if editable_comment.is_empty() {
        base_comment
    } else {
        editable_comment.clone()
    };
    TokenAnnotation {
        base_word: base_word.clone(),
        base_gloss: project
            .vocabulary
            .get(base_word)
            .cloned()
            .unwrap_or_default(),
        formation_descriptions: token
            .formation_rule_indices
            .iter()
            .filter_map(|index| project.formation_rules.get(*index))
            .map(|rule| rule.description.clone())
            .collect(),
        is_derived,
        display_comment,
        comment_target,
        editable_comment,
    }
}

/// Owns project data, mutation revisions, and query caches independently of UI.
pub struct Session {
    project: Project,
    identity: usize,
    generation: u64,
    revision: u64,
    text_revision: u64,
    dirty: bool,
    lookups: Option<LookupSnapshot>,
    tfidf: CachedTfidf,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            project: Project::default(),
            identity: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            generation: 0,
            revision: 0,
            text_revision: 0,
            dirty: false,
            lookups: None,
            tfidf: CachedTfidf::default(),
        }
    }
}

impl Session {
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Return the editable comment target and text for a token, without display fallback.
    pub fn token_comment(&self, segment: usize, token: usize) -> Result<(CommentTarget, String)> {
        let token = get_token(&self.project, segment, token)?;
        let annotation = annotate_token(&self.project, token);
        Ok((annotation.comment_target, annotation.editable_comment))
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Changes only when text-dependent lookups and similarity may have changed.
    pub fn text_revision(&self) -> u64 {
        self.text_revision
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Load atomically; parse, migration, and script errors preserve the session.
    pub fn load_json(&mut self, json: &str) -> Result<()> {
        let value = serde_json::from_str(json).map_err(|error| {
            AppError::InvalidProjectFormat(format!("Failed to parse project JSON: {error}"))
        })?;
        let project = tdector_file::project::load_project_from_json(value)?;
        self.replace_project(project, false);
        Ok(())
    }

    /// Replace text and name, preserving the current vocabulary and rules.
    pub fn import_text(
        &mut self,
        content: &str,
        name: &str,
        rule: &TokenizationRule,
    ) -> Result<()> {
        let segments = TextProcessor::segment_text_with_rule(content, Some(rule))?;
        self.project.segments = segments;
        self.project.project_name = name.to_owned();
        self.generation = self.generation.wrapping_add(1);
        self.changed(true);
        Ok(())
    }

    /// Serialize without choosing paths, writing files, or changing dirty state.
    pub fn save_snapshot(&self) -> Result<SaveSnapshot> {
        let saved = tdector_file::project::convert_to_saved_project(&self.project)?;
        let formatter = tdector_file::io::json_formatter::Formatter::new();
        let mut bytes = Vec::new();
        let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
        serde::Serialize::serialize(&saved, &mut serializer).map_err(|error| {
            AppError::InvalidProjectFormat(format!("Failed to serialize project: {error}"))
        })?;
        Ok(SaveSnapshot {
            bytes,
            token: self.save_token(),
        })
    }

    /// Acknowledge only after a successful write of this exact snapshot.
    pub fn acknowledge_saved(&mut self, token: SaveToken) -> bool {
        if token != self.save_token() {
            return false;
        }
        self.dirty = false;
        true
    }

    pub fn export_typst(&self) -> String {
        tdector_file::io::generate_typst_content(&self.project)
    }

    /// Apply one validated command. A no-op leaves revision and caches unchanged.
    pub fn execute(&mut self, command: Command) -> Result<bool> {
        let text_changed = matches!(&command, Command::RemoveFormationRule { .. });
        let changed = match command {
            Command::SetGloss { word, meaning } => {
                nonempty_word(&word)?;
                set_entry(&mut self.project.vocabulary, word, meaning, false)
            }
            Command::SetTranslation {
                segment,
                translation,
            } => {
                let value = self
                    .project
                    .segments
                    .get_mut(segment)
                    .ok_or_else(|| invalid_index("segment", segment))?;
                set_text(&mut value.translation, translation)
            }
            Command::SetWordComment { target, comment } => {
                let (map, word) = match target {
                    CommentTarget::BaseWord(word) => (&mut self.project.vocabulary_comments, word),
                    CommentTarget::FormattedWord(word) => {
                        (&mut self.project.formatted_word_comments, word)
                    }
                };
                nonempty_word(&word)?;
                set_entry(map, word, comment, true)
            }
            Command::SetSegmentComment { segment, comment } => {
                let value = self
                    .project
                    .segments
                    .get_mut(segment)
                    .ok_or_else(|| invalid_index("segment", segment))?;
                set_text(&mut value.comment, comment)
            }
            Command::CreateFormationRule {
                description,
                rule_type,
                command,
            } => {
                if description.trim().is_empty() {
                    return Err(Error::InvalidInput(
                        "A formation rule needs a description".into(),
                    ));
                }
                let ast = tdector_eval::with_engine(|engine| engine.compile(&command)).map_err(
                    |error| {
                        AppError::ScriptExecutionError(format!("Rhai compilation error: {error}"))
                    },
                )?;
                if !ast
                    .iter_functions()
                    .any(|function| function.name == "transform" && function.params.len() == 1)
                {
                    return Err(AppError::ScriptExecutionError(
                        "A formation rule must define transform(word)".into(),
                    )
                    .into());
                }
                if self.project.formation_rules.iter().any(|rule| {
                    rule.description == description
                        && rule.rule_type == rule_type
                        && rule.command == command
                }) {
                    false
                } else {
                    let cached_ast = tdector_eval::default_cached_ast();
                    let _ = cached_ast.set(ast);
                    self.project.formation_rules.push(FormationRule {
                        description,
                        rule_type,
                        command,
                        cached_ast,
                    });
                    true
                }
            }
            Command::ApplyFormationRule {
                word,
                base_word,
                rule,
            } => {
                let mut candidate = self.project.clone();
                let changed = apply_formation(&mut candidate, &word, &base_word, rule)?;
                if changed {
                    self.project = candidate;
                }
                changed
            }
            Command::RemoveFormationRule { segment, token } => {
                let mut candidate = self.project.clone();
                let changed = remove_formation(&mut candidate, segment, token)?;
                if changed {
                    self.project = candidate;
                }
                changed
            }
        };
        if changed {
            self.changed(text_changed);
        }
        Ok(changed)
    }

    pub fn filtered_indices(&self, query: &str, sort: SortMode) -> Vec<usize> {
        let mut indices = FilterOperation::apply_filter(&self.project, query);
        SortOperation::apply_sort(&self.project, &mut indices, sort);
        indices
    }

    /// Segment lookups: headwords are first tokens; usages deduplicate per segment.
    pub fn lookup_maps(&mut self) -> LookupSnapshot {
        if let Some(maps) = &self.lookups {
            return maps.clone();
        }
        let mut headwords: HashMap<String, Vec<usize>> = HashMap::new();
        let mut usages: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, segment) in self.project.segments.iter().enumerate() {
            if let Some(first) = segment.tokens.first() {
                headwords
                    .entry(first.original.clone())
                    .or_default()
                    .push(index);
            }
            let mut seen = HashSet::new();
            for token in &segment.tokens {
                if seen.insert(&token.original) {
                    usages
                        .entry(token.original.clone())
                        .or_default()
                        .push(index);
                }
            }
        }
        let maps = (Some(Arc::new(headwords)), Some(Arc::new(usages)));
        self.lookups = Some(maps.clone());
        maps
    }

    pub fn similar_segments(&mut self, target: usize, limit: usize) -> Result<Vec<(usize, f64)>> {
        if target >= self.project.segments.len() {
            return Err(invalid_index("segment", target));
        }
        if self.tfidf.is_dirty() {
            if let Some(matrix) = SimilarityEngine::compute_tfidf_matrix(&self.project) {
                self.tfidf.set_matrix(matrix);
            } else {
                return Err(Error::InvalidInput(
                    "Unable to compute sentence similarity for this project".into(),
                ));
            }
        }
        Ok(self
            .tfidf
            .get_matrix()
            .map(|matrix| SimilarityEngine::find_similar(matrix, target, limit))
            .unwrap_or_default())
    }

    pub fn similar_tokens(&self, word: &str) -> Vec<SimilarToken> {
        tdector_text::similarity_token::find_similar_tokens(&self.project, word)
    }

    pub fn related_words(&self, prefix: &str, limit: usize) -> Vec<String> {
        if prefix.is_empty() {
            return Vec::new();
        }
        let query = prefix.to_lowercase();
        let mut words: Vec<_> = self
            .project
            .vocabulary
            .keys()
            .filter(|word| word.to_lowercase().contains(&query))
            .cloned()
            .collect();
        words.sort_by_key(|word| {
            let lower = word.to_lowercase();
            (!lower.starts_with(&query), lower, word.clone())
        });
        words.truncate(limit);
        words
    }

    pub fn preview_rule(rule: &FormationRule, word: &str) -> Result<String> {
        Ok(rule.apply(word)?)
    }

    pub fn preview_tokenization(rule: &TokenizationRule, text: &str) -> Result<Vec<String>> {
        Ok(rule.tokenize(text)?)
    }

    pub fn formation_chain(&self, segment: usize, token: usize) -> Result<Vec<FormationStep>> {
        let token = get_token(&self.project, segment, token)?;
        let mut current = token
            .base_word
            .as_deref()
            .unwrap_or(&token.original)
            .to_owned();
        let mut steps = Vec::new();
        for &index in &token.formation_rule_indices {
            let rule = self
                .project
                .formation_rules
                .get(index)
                .ok_or_else(|| invalid_index("rule", index))?;
            current = rule.apply(&current)?;
            steps.push(FormationStep {
                description: rule.description.clone(),
                rule_type: rule.rule_type,
                result: current.clone(),
            });
        }
        Ok(steps)
    }

    fn save_token(&self) -> SaveToken {
        SaveToken {
            session: self.identity,
            generation: self.generation,
            revision: self.revision,
        }
    }

    fn replace_project(&mut self, project: Project, dirty: bool) {
        self.project = project;
        self.generation = self.generation.wrapping_add(1);
        self.changed(true);
        self.dirty = dirty;
    }

    fn changed(&mut self, text_changed: bool) {
        self.revision = self.revision.wrapping_add(1);
        self.dirty = true;
        if text_changed {
            self.text_revision = self.text_revision.wrapping_add(1);
            self.lookups = None;
            self.tfidf.invalidate();
        }
    }
}

fn invalid_index(kind: &'static str, index: usize) -> Error {
    Error::InvalidIndex { kind, index }
}

fn nonempty_word(word: &str) -> Result<()> {
    if word.is_empty() {
        Err(Error::InvalidInput("A word must not be empty".into()))
    } else {
        Ok(())
    }
}

fn set_text(current: &mut String, value: String) -> bool {
    if *current == value {
        return false;
    }
    *current = value;
    true
}

fn set_entry(
    map: &mut HashMap<String, String>,
    word: String,
    value: String,
    remove_empty: bool,
) -> bool {
    if remove_empty && value.is_empty() {
        return map.remove(&word).is_some();
    }
    if map.get(&word) == Some(&value) {
        return false;
    }
    map.insert(word, value);
    true
}

fn get_token(project: &Project, segment: usize, token: usize) -> Result<&Token> {
    project
        .segments
        .get(segment)
        .ok_or_else(|| invalid_index("segment", segment))?
        .tokens
        .get(token)
        .ok_or_else(|| invalid_index("token", token))
}

fn apply_formation(
    project: &mut Project,
    word: &str,
    base: &str,
    rule_index: usize,
) -> Result<bool> {
    nonempty_word(word)?;
    nonempty_word(base)?;
    let rule = project
        .formation_rules
        .get(rule_index)
        .ok_or_else(|| invalid_index("rule", rule_index))?;
    if rule.apply(base)? != word {
        return Err(Error::InvalidInput(
            "The formation result must match the selected word".into(),
        ));
    }
    if !project
        .segments
        .iter()
        .flat_map(|segment| &segment.tokens)
        .any(|token| token.original == word)
    {
        return Err(Error::InvalidInput(format!(
            "Word '{word}' has no token occurrences"
        )));
    }
    let (root, mut chain) = if project.vocabulary.contains_key(base) {
        (base.to_owned(), Vec::new())
    } else {
        let mut candidates = project
            .segments
            .iter()
            .flat_map(|segment| &segment.tokens)
            .filter(|token| token.original == base && !token.formation_rule_indices.is_empty());
        let first = candidates.next().ok_or_else(|| {
            Error::InvalidInput(format!(
                "Base word '{base}' is not in the vocabulary or derived words"
            ))
        })?;
        let root = first.base_word.as_deref().unwrap_or(&first.original);
        if candidates.any(|token| {
            token.base_word.as_deref().unwrap_or(&token.original) != root
                || token.formation_rule_indices != first.formation_rule_indices
        }) {
            return Err(Error::InvalidInput(format!(
                "Base word '{base}' has ambiguous formation chains"
            )));
        }
        if !project.vocabulary.contains_key(root) {
            return Err(Error::InvalidInput(format!(
                "Root word '{root}' is missing from the vocabulary"
            )));
        }
        (root.to_owned(), first.formation_rule_indices.clone())
    };
    chain.push(rule_index);
    let mut changed = false;
    for token in project
        .segments
        .iter_mut()
        .flat_map(|segment| &mut segment.tokens)
    {
        if token.original == word
            && (token.base_word.as_deref() != Some(&root) || token.formation_rule_indices != chain)
        {
            token.base_word = Some(root.clone());
            token.formation_rule_indices = chain.clone();
            changed = true;
        }
    }
    if !changed {
        return Ok(false);
    }
    let still_needed_as_root = project
        .segments
        .iter()
        .flat_map(|segment| &segment.tokens)
        .any(|token| token.base_word.as_deref() == Some(word));
    if word != root && !still_needed_as_root {
        project.vocabulary.remove(word);
        if let Some(comment) = project.vocabulary_comments.remove(word) {
            merge_comment(
                &mut project.formatted_word_comments,
                word.to_owned(),
                comment,
            );
        }
    }
    Ok(true)
}

fn remove_formation(project: &mut Project, segment: usize, index: usize) -> Result<bool> {
    let selected = get_token(project, segment, index)?.clone();
    if selected.formation_rule_indices.is_empty() {
        return Ok(false);
    }
    let root = selected
        .base_word
        .as_deref()
        .unwrap_or(&selected.original)
        .to_owned();
    let mut chain = selected.formation_rule_indices.clone();
    chain.pop();
    let mut surface = root.clone();
    for &rule_index in &chain {
        let rule = project
            .formation_rules
            .get(rule_index)
            .ok_or_else(|| invalid_index("rule", rule_index))?;
        surface = rule.apply(&surface)?;
    }
    for token in project
        .segments
        .iter_mut()
        .flat_map(|segment| &mut segment.tokens)
    {
        if token.original == selected.original
            && token.formation_rule_indices == selected.formation_rule_indices
            && token.base_word.as_deref().unwrap_or(&token.original) == root
        {
            token.original = surface.clone();
            token.base_word = if chain.is_empty() {
                None
            } else {
                Some(root.clone())
            };
            token.formation_rule_indices = chain.clone();
        }
    }
    if chain.is_empty() {
        project.vocabulary.entry(root).or_default();
    }
    if let Some(comment) = project
        .formatted_word_comments
        .get(&selected.original)
        .cloned()
    {
        let old_form_remains = project
            .segments
            .iter()
            .flat_map(|segment| &segment.tokens)
            .any(|token| {
                token.original == selected.original && !token.formation_rule_indices.is_empty()
            });
        if !old_form_remains {
            project.formatted_word_comments.remove(&selected.original);
        }
        if chain.is_empty() {
            merge_comment(&mut project.vocabulary_comments, surface, comment);
        } else {
            merge_comment(&mut project.formatted_word_comments, surface, comment);
        }
    }
    Ok(true)
}

fn merge_comment(map: &mut HashMap<String, String>, word: String, comment: String) {
    if comment.is_empty() {
        return;
    }
    let existing = map.entry(word).or_default();
    if existing.is_empty() {
        *existing = comment;
    } else if *existing != comment {
        existing.push_str("\n\n");
        existing.push_str(&comment);
    }
}
