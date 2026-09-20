//! Versioned adapter-facing requests and explicit, cache-free query results.
//!
//! This module owns domain validation, not paths, standard streams, exit codes, or file commits. Indices refer only to the currently loaded session snapshot.

use std::fmt;

use serde::{Deserialize, Serialize};
use tdector_core::enums::{CommentTarget, SortDirection, SortField, SortMode};
use tdector_eval::{AppError, FormationRule, FormationType, TokenizationRule};

use crate::{Command, Session, annotate_token};

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Clone)]
pub enum ApiError {
    Application(crate::Error),
    InvalidInput(String),
    NotFound {
        kind: &'static str,
        value: String,
    },
    RuleNotFound {
        description: String,
    },
    AmbiguousRule {
        description: String,
        rule_indices: Vec<usize>,
    },
    Batch {
        stage: BatchStage,
        command_index: Option<usize>,
        source: Box<ApiError>,
    },
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application(error) => error.fmt(f),
            Self::InvalidInput(message) => f.write_str(message),
            Self::NotFound { kind, value } => write!(f, "No {kind} found for '{value}'"),
            Self::RuleNotFound { description } => write!(f, "No rule matches '{description}'"),
            Self::AmbiguousRule {
                description,
                rule_indices,
            } => write!(
                f,
                "Rule '{description}' is ambiguous; matching rule indices: {rule_indices:?}"
            ),
            Self::Batch {
                stage,
                command_index,
                source,
            } => {
                write!(f, "Batch {stage:?}")?;
                if let Some(index) = command_index {
                    write!(f, " at command {index}")?;
                }
                write!(f, ": {source}")
            }
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Application(error) => Some(error),
            Self::Batch { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<crate::Error> for ApiError {
    fn from(error: crate::Error) -> Self {
        Self::Application(error)
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        Self::Application(error.into())
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchStage {
    Input,
    Command,
    Serialize,
    Commit,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FormationKind {
    Derivation,
    Inflection,
    Nonmorphological,
}

impl From<FormationKind> for FormationType {
    fn from(kind: FormationKind) -> Self {
        match kind {
            FormationKind::Derivation => Self::Derivation,
            FormationKind::Inflection => Self::Inflection,
            FormationKind::Nonmorphological => Self::Nonmorphological,
        }
    }
}

impl From<FormationType> for FormationKind {
    fn from(kind: FormationType) -> Self {
        match kind {
            FormationType::Derivation => Self::Derivation,
            FormationType::Inflection => Self::Inflection,
            FormationType::Nonmorphological => Self::Nonmorphological,
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSelector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<usize>,
}

/// Resolve exactly one selector against this snapshot; descriptions are not IDs.
pub fn resolve_rule(session: &Session, selector: &RuleSelector) -> Result<usize> {
    match (&selector.rule, selector.rule_index) {
        (None, Some(index)) => {
            session
                .project()
                .formation_rules
                .get(index)
                .ok_or(crate::Error::InvalidIndex {
                    kind: "rule",
                    index,
                })?;
            Ok(index)
        }
        (Some(description), None) => {
            let matches: Vec<_> = session
                .project()
                .formation_rules
                .iter()
                .enumerate()
                .filter(|(_, rule)| rule.description == *description)
                .map(|(index, _)| index)
                .collect();
            match matches.as_slice() {
                [] => Err(ApiError::RuleNotFound {
                    description: description.clone(),
                }),
                [index] => Ok(*index),
                _ => Err(ApiError::AmbiguousRule {
                    description: description.clone(),
                    rule_indices: matches,
                }),
            }
        }
        _ => Err(ApiError::InvalidInput(
            "Exactly one of rule and rule_index is required".into(),
        )),
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pagination {
    pub offset: usize,
    pub limit: Option<usize>,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: Some(50),
        }
    }
}

impl Pagination {
    pub fn all() -> Self {
        Self {
            offset: 0,
            limit: None,
        }
    }

    fn validate(self) -> Result<()> {
        if let Some(limit) = self.limit {
            positive_limit(limit)?;
        }
        if self.limit.is_none() && self.offset != 0 {
            return Err(ApiError::InvalidInput(
                "An unpaginated query must have offset zero".into(),
            ));
        }
        Ok(())
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub offset: usize,
    pub limit: Option<usize>,
    pub total: usize,
}

fn paginate<T>(items: Vec<T>, pagination: Pagination) -> Result<Page<T>> {
    pagination.validate()?;
    let total = items.len();
    Ok(Page {
        items: items
            .into_iter()
            .skip(pagination.offset)
            .take(pagination.limit.unwrap_or(usize::MAX))
            .collect(),
        offset: pagination.offset,
        limit: pagination.limit,
        total,
    })
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SegmentSort {
    #[default]
    Index,
    Text,
    TokenCount,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LookupKind {
    #[default]
    Usage,
    Headword,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    Info,
    Validate,
    SegmentList {
        filter: String,
        sort: SegmentSort,
        descending: bool,
        pagination: Pagination,
    },
    SegmentShow {
        segment_index: usize,
    },
    VocabList {
        pagination: Pagination,
    },
    VocabGet {
        word: String,
    },
    VocabSearch {
        text: String,
        limit: usize,
    },
    CommentGet {
        segment_index: usize,
        token_index: Option<usize>,
    },
    Lookup {
        word: String,
        kind: LookupKind,
        pagination: Pagination,
    },
    SimilarSegments {
        segment_index: usize,
        limit: usize,
    },
    SimilarTokens {
        word: String,
        limit: usize,
    },
    RuleList {
        pagination: Pagination,
    },
    RuleShow {
        selector: RuleSelector,
    },
    FormationChain {
        segment_index: usize,
        token_index: usize,
    },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct InfoResult {
    pub project_name: String,
    pub segment_count: usize,
    pub token_count: usize,
    pub vocabulary_count: usize,
    pub rule_count: usize,
    pub project_format_version: u64,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    #[serde(flatten)]
    pub project: InfoResult,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct SegmentRecord {
    pub segment_index: usize,
    pub text: String,
    pub translation: String,
    pub token_count: usize,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommentTargetRecord {
    Segment { segment_index: usize },
    BaseWord { word: String },
    FormattedWord { word: String },
}

impl From<CommentTarget> for CommentTargetRecord {
    fn from(target: CommentTarget) -> Self {
        match target {
            CommentTarget::BaseWord(word) => Self::BaseWord { word },
            CommentTarget::FormattedWord(word) => Self::FormattedWord { word },
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct TokenRecord {
    pub token_index: usize,
    pub text: String,
    pub base_word: String,
    pub base_gloss: String,
    pub is_derived: bool,
    pub formation_rule_indices: Vec<usize>,
    pub formation_descriptions: Vec<String>,
    pub display_comment: String,
    pub editable_comment: String,
    pub comment_target: CommentTargetRecord,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct SegmentDetail {
    #[serde(flatten)]
    pub segment: SegmentRecord,
    pub comment: String,
    pub tokens: Vec<TokenRecord>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct VocabularyRecord {
    pub word: String,
    pub meaning: String,
    pub comment: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct CommentResult {
    pub segment_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_index: Option<usize>,
    pub target: CommentTargetRecord,
    pub editable_comment: String,
    pub display_comment: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct RuleRecord {
    pub rule_index: usize,
    pub description: String,
    #[serde(rename = "type")]
    pub rule_type: FormationKind,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct RuleDetail {
    #[serde(flatten)]
    pub rule: RuleRecord,
    pub script: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct SimilarSegmentRecord {
    #[serde(flatten)]
    pub segment: SegmentRecord,
    pub score: f64,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct SimilarTokenRecord {
    pub word: String,
    pub distance: usize,
    pub lcs_length: usize,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct FormationStepRecord {
    pub rule_index: usize,
    pub description: String,
    #[serde(rename = "type")]
    pub rule_type: FormationKind,
    pub result: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct FormationChainResult {
    pub segment_index: usize,
    pub token_index: usize,
    pub word: String,
    pub base_word: String,
    pub steps: Vec<FormationStepRecord>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct Items<T> {
    pub items: Vec<T>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum QueryResponse {
    Info(InfoResult),
    Validate(ValidationResult),
    Segments(Page<SegmentRecord>),
    Segment(SegmentDetail),
    Vocabulary(Page<VocabularyRecord>),
    VocabEntry(VocabularyRecord),
    VocabSearch(Items<VocabularyRecord>),
    Comment(CommentResult),
    SimilarSegments(Items<SimilarSegmentRecord>),
    SimilarTokens(Items<SimilarTokenRecord>),
    Rules(Page<RuleRecord>),
    Rule(RuleDetail),
    FormationChain(FormationChainResult),
}

fn info(session: &Session) -> InfoResult {
    let project = session.project();
    InfoResult {
        project_name: project.project_name.clone(),
        segment_count: project.segments.len(),
        token_count: project
            .segments
            .iter()
            .map(|segment| segment.tokens.len())
            .sum(),
        vocabulary_count: project.vocabulary.len(),
        rule_count: project.formation_rules.len(),
        project_format_version: tdector_core::consts::domain::PROJECT_VERSION,
    }
}

fn segment_record(session: &Session, segment_index: usize) -> Result<SegmentRecord> {
    let segment =
        session
            .project()
            .segments
            .get(segment_index)
            .ok_or(crate::Error::InvalidIndex {
                kind: "segment",
                index: segment_index,
            })?;
    Ok(SegmentRecord {
        segment_index,
        text: segment
            .tokens
            .iter()
            .map(|token| token.original.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        translation: segment.translation.clone(),
        token_count: segment.tokens.len(),
    })
}

fn vocabulary_record(session: &Session, word: &str) -> Result<VocabularyRecord> {
    let project = session.project();
    let meaning = project
        .vocabulary
        .get(word)
        .ok_or_else(|| ApiError::NotFound {
            kind: "vocabulary entry",
            value: word.to_owned(),
        })?;
    Ok(VocabularyRecord {
        word: word.to_owned(),
        meaning: meaning.clone(),
        comment: project
            .vocabulary_comments
            .get(word)
            .cloned()
            .unwrap_or_default(),
    })
}

fn rule_record(rule_index: usize, rule: &FormationRule) -> RuleRecord {
    RuleRecord {
        rule_index,
        description: rule.description.clone(),
        rule_type: rule.rule_type.into(),
    }
}

fn positive_limit(limit: usize) -> Result<()> {
    if limit == 0 {
        return Err(ApiError::InvalidInput("Limit must be positive".into()));
    }
    Ok(())
}

/// Query only explicit application DTOs; runtime project caches never serialize.
pub fn query(session: &mut Session, request: Query) -> Result<QueryResponse> {
    match request {
        Query::Info => Ok(QueryResponse::Info(info(session))),
        // Session loading has already parsed, migrated, validated references, and reconstructed every used formation. It intentionally does not execute otherwise unused rules on invented input.
        Query::Validate => Ok(QueryResponse::Validate(ValidationResult {
            valid: true,
            project: info(session),
        })),
        Query::SegmentList {
            filter,
            sort,
            descending,
            pagination,
        } => {
            pagination.validate()?;
            let mode = SortMode {
                field: match sort {
                    SegmentSort::Index => SortField::Index,
                    SegmentSort::Text => SortField::Original,
                    SegmentSort::TokenCount => SortField::Count,
                },
                direction: if descending {
                    SortDirection::Descending
                } else {
                    SortDirection::Ascending
                },
            };
            let records = session
                .filtered_indices(&filter, mode)
                .into_iter()
                .map(|index| segment_record(session, index))
                .collect::<Result<Vec<_>>>()?;
            Ok(QueryResponse::Segments(paginate(records, pagination)?))
        }
        Query::SegmentShow { segment_index } => {
            let record = segment_record(session, segment_index)?;
            let segment = &session.project().segments[segment_index];
            let tokens = segment
                .tokens
                .iter()
                .enumerate()
                .map(|(token_index, token)| {
                    let annotation = annotate_token(session.project(), token);
                    TokenRecord {
                        token_index,
                        text: token.original.clone(),
                        base_word: annotation.base_word,
                        base_gloss: annotation.base_gloss,
                        is_derived: annotation.is_derived,
                        formation_rule_indices: token.formation_rule_indices.clone(),
                        formation_descriptions: annotation.formation_descriptions,
                        display_comment: annotation.display_comment,
                        editable_comment: annotation.editable_comment,
                        comment_target: annotation.comment_target.into(),
                    }
                })
                .collect();
            Ok(QueryResponse::Segment(SegmentDetail {
                segment: record,
                comment: segment.comment.clone(),
                tokens,
            }))
        }
        Query::VocabList { pagination } => {
            pagination.validate()?;
            let mut words: Vec<_> = session.project().vocabulary.keys().collect();
            words.sort();
            let records = words
                .into_iter()
                .map(|word| vocabulary_record(session, word))
                .collect::<Result<Vec<_>>>()?;
            Ok(QueryResponse::Vocabulary(paginate(records, pagination)?))
        }
        Query::VocabGet { word } => Ok(QueryResponse::VocabEntry(vocabulary_record(
            session, &word,
        )?)),
        Query::VocabSearch { text, limit } => {
            positive_limit(limit)?;
            let items = session
                .related_words(&text, limit)
                .into_iter()
                .map(|word| vocabulary_record(session, &word))
                .collect::<Result<Vec<_>>>()?;
            Ok(QueryResponse::VocabSearch(Items { items }))
        }
        Query::CommentGet {
            segment_index,
            token_index,
        } => {
            let result = if let Some(token_index) = token_index {
                let (target, editable_comment) =
                    session.token_comment(segment_index, token_index)?;
                let token = crate::get_token(session.project(), segment_index, token_index)?;
                CommentResult {
                    segment_index,
                    token_index: Some(token_index),
                    target: target.into(),
                    editable_comment,
                    display_comment: annotate_token(session.project(), token).display_comment,
                }
            } else {
                let segment = session.project().segments.get(segment_index).ok_or(
                    crate::Error::InvalidIndex {
                        kind: "segment",
                        index: segment_index,
                    },
                )?;
                CommentResult {
                    segment_index,
                    token_index: None,
                    target: CommentTargetRecord::Segment { segment_index },
                    editable_comment: segment.comment.clone(),
                    display_comment: segment.comment.clone(),
                }
            };
            Ok(QueryResponse::Comment(result))
        }
        Query::Lookup {
            word,
            kind,
            pagination,
        } => {
            pagination.validate()?;
            let (headwords, usages) = session.lookup_maps();
            let map = match kind {
                LookupKind::Usage => usages,
                LookupKind::Headword => headwords,
            };
            let indices = map
                .as_ref()
                .and_then(|map| map.get(&word))
                .cloned()
                .unwrap_or_default();
            let records = indices
                .into_iter()
                .map(|index| segment_record(session, index))
                .collect::<Result<Vec<_>>>()?;
            Ok(QueryResponse::Segments(paginate(records, pagination)?))
        }
        Query::SimilarSegments {
            segment_index,
            limit,
        } => {
            positive_limit(limit)?;
            let results = session.similar_segments(segment_index, limit)?;
            let items = results
                .into_iter()
                .map(|(index, score)| {
                    Ok(SimilarSegmentRecord {
                        segment: segment_record(session, index)?,
                        score,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(QueryResponse::SimilarSegments(Items { items }))
        }
        Query::SimilarTokens { word, limit } => {
            positive_limit(limit)?;
            if limit > 20 {
                return Err(ApiError::InvalidInput(
                    "Token similarity limit must be between 1 and 20".into(),
                ));
            }
            let items = session
                .similar_tokens(&word)
                .into_iter()
                .take(limit)
                .map(|token| SimilarTokenRecord {
                    word: token.word,
                    distance: token.distance,
                    lcs_length: token.lcs_length,
                })
                .collect();
            Ok(QueryResponse::SimilarTokens(Items { items }))
        }
        Query::RuleList { pagination } => {
            let records = session
                .project()
                .formation_rules
                .iter()
                .enumerate()
                .map(|(index, rule)| rule_record(index, rule))
                .collect();
            Ok(QueryResponse::Rules(paginate(records, pagination)?))
        }
        Query::RuleShow { selector } => {
            let index = resolve_rule(session, &selector)?;
            let rule = &session.project().formation_rules[index];
            Ok(QueryResponse::Rule(RuleDetail {
                rule: rule_record(index, rule),
                script: rule.command.clone(),
            }))
        }
        Query::FormationChain {
            segment_index,
            token_index,
        } => {
            let token = crate::get_token(session.project(), segment_index, token_index)?;
            let steps = session
                .formation_chain(segment_index, token_index)?
                .into_iter()
                .zip(&token.formation_rule_indices)
                .map(|(step, &rule_index)| FormationStepRecord {
                    rule_index,
                    description: step.description,
                    rule_type: step.rule_type.into(),
                    result: step.result,
                })
                .collect();
            Ok(QueryResponse::FormationChain(FormationChainResult {
                segment_index,
                token_index,
                word: token.original.clone(),
                base_word: token.base_word.as_ref().unwrap_or(&token.original).clone(),
                steps,
            }))
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    SetGloss {
        word: String,
        meaning: String,
    },
    SetTranslation {
        segment_index: usize,
        translation: String,
    },
    SetComment {
        segment_index: usize,
        #[serde(default)]
        token_index: Option<usize>,
        comment: String,
    },
    CreateRule {
        description: String,
        #[serde(rename = "type")]
        rule_type: FormationKind,
        script: String,
    },
    ApplyFormation {
        word: String,
        base: String,
        #[serde(default)]
        rule: Option<String>,
        #[serde(default)]
        rule_index: Option<usize>,
    },
    PopFormation {
        segment_index: usize,
        token_index: usize,
    },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct MutationReceipt {
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub rule_type: Option<FormationKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Apply a domain operation, validating coordinate comment targets before writes.
pub fn apply_mutation(session: &mut Session, request: Mutation) -> Result<MutationReceipt> {
    let mut receipt = MutationReceipt {
        changed: false,
        description: None,
        rule_type: None,
        scope: None,
    };
    let command = match request {
        Mutation::SetGloss { word, meaning } => Command::SetGloss { word, meaning },
        Mutation::SetTranslation {
            segment_index,
            translation,
        } => Command::SetTranslation {
            segment: segment_index,
            translation,
        },
        Mutation::SetComment {
            segment_index,
            token_index,
            comment,
        } => {
            if let Some(token_index) = token_index {
                let (target, _) = session.token_comment(segment_index, token_index)?;
                // Plain token words are included by the project exporter even without an explicit gloss entry. Formed token targets were validated when loaded or formed, so both targets persist.
                receipt.scope = Some("shared word comment across matching occurrences".into());
                Command::SetWordComment { target, comment }
            } else {
                Command::SetSegmentComment {
                    segment: segment_index,
                    comment,
                }
            }
        }
        Mutation::CreateRule {
            description,
            rule_type,
            script,
        } => {
            receipt.description = Some(description.clone());
            receipt.rule_type = Some(rule_type);
            Command::CreateFormationRule {
                description,
                rule_type: rule_type.into(),
                command: script,
            }
        }
        Mutation::ApplyFormation {
            word,
            base,
            rule,
            rule_index,
        } => {
            let index = resolve_rule(session, &RuleSelector { rule, rule_index })?;
            receipt.scope = Some("all occurrences of the selected surface word".into());
            Command::ApplyFormationRule {
                word,
                base_word: base,
                rule: index,
            }
        }
        Mutation::PopFormation {
            segment_index,
            token_index,
        } => {
            receipt.scope = Some(
                "all occurrences matching the selected surface, base, and formation chain".into(),
            );
            Command::RemoveFormationRule {
                segment: segment_index,
                token: token_index,
            }
        }
    };
    receipt.changed = session.execute(command)?;
    Ok(receipt)
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRequest {
    pub schema_version: u32,
    pub commands: Vec<Mutation>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct BatchReceipt {
    pub changed: bool,
    pub commands: Vec<MutationReceipt>,
}

/// An opaque, single-use batch candidate. Receipts describe staged changes; preparing or dropping this value does not mutate the live session.
#[derive(Debug)]
pub struct PreparedBatch {
    project: tdector_core::libs::Project,
    origin: crate::SaveToken,
    origin_dirty: bool,
    text_changed: bool,
    receipt: BatchReceipt,
}

impl PreparedBatch {
    pub fn receipt(&self) -> &BatchReceipt {
        &self.receipt
    }

    pub fn projected_revision(&self) -> u64 {
        self.origin
            .revision
            .wrapping_add(u64::from(self.receipt.changed))
    }

    pub fn projected_dirty(&self) -> bool {
        self.origin_dirty || self.receipt.changed
    }
}

/// Validate a transaction against an independent runtime copy of project data.
///
/// Commands run in order and see preceding staged edits. Serialization is validated before returning, without reloading or reordering runtime rules. Adapters can inspect receipts and budget their exact response before commit.
pub fn prepare_batch(session: &Session, request: BatchRequest) -> Result<PreparedBatch> {
    tdector_eval::check_execution()
        .map_err(|error| batch_error(BatchStage::Input, None, error.into()))?;
    let mut staged = session.staging_session();
    let receipt = execute_batch(&mut staged, request)?;
    tdector_eval::check_execution()
        .map_err(|error| batch_error(BatchStage::Serialize, None, error.into()))?;
    staged
        .save_snapshot()
        .map_err(|error| batch_error(BatchStage::Serialize, None, error.into()))?;
    tdector_eval::check_execution()
        .map_err(|error| batch_error(BatchStage::Serialize, None, error.into()))?;
    Ok(PreparedBatch {
        project: staged.project,
        origin: session.save_token(),
        origin_dirty: session.is_dirty(),
        text_changed: staged.text_revision != 0,
        receipt,
    })
}

/// Consume a candidate after checking that its originating state is current. A changed batch advances the live revision once and preserves save authority. Annotation-only changes retain text-dependent query caches.
pub fn commit_batch(session: &mut Session, prepared: PreparedBatch) -> Result<BatchReceipt> {
    session
        .validate_preparation(prepared.origin, prepared.origin_dirty)
        .map_err(|error| batch_error(BatchStage::Commit, None, error.into()))?;
    tdector_eval::check_execution()
        .map_err(|error| batch_error(BatchStage::Commit, None, error.into()))?;
    if prepared.receipt.changed {
        session.project = prepared.project;
        session.changed(prepared.text_changed);
    }
    Ok(prepared.receipt)
}

fn batch_error(stage: BatchStage, command_index: Option<usize>, source: ApiError) -> ApiError {
    ApiError::Batch {
        stage,
        command_index,
        source: Box::new(source),
    }
}

/// Decode all commands first, retaining the index of a malformed command. The adapter owns UTF-8 BOM normalization, just as for `Session::load_json`.
pub fn parse_batch(json: &str) -> Result<BatchRequest> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawBatch {
        schema_version: u32,
        commands: Vec<serde_json::Value>,
    }

    let raw: RawBatch = serde_json::from_str(json).map_err(|error| {
        batch_error(
            BatchStage::Input,
            None,
            ApiError::InvalidInput(format!("Invalid batch JSON: {error}")),
        )
    })?;
    if raw.schema_version != 1 {
        return Err(batch_error(
            BatchStage::Input,
            None,
            ApiError::InvalidInput(format!(
                "Unsupported batch schema version: {}",
                raw.schema_version
            )),
        ));
    }
    let commands = raw
        .commands
        .into_iter()
        .enumerate()
        .map(|(index, command)| {
            serde_json::from_value(command).map_err(|error| {
                batch_error(
                    BatchStage::Input,
                    Some(index),
                    ApiError::InvalidInput(format!("Invalid batch command: {error}")),
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(BatchRequest {
        schema_version: 1,
        commands,
    })
}

/// Execute sequentially against a privately owned invocation session.
///
/// This is not an in-memory transaction: on failure earlier commands remain applied. Adapters must discard the private session and never commit a failed batch. Serialization and file commit happen only after this returns success.
pub fn execute_batch(session: &mut Session, request: BatchRequest) -> Result<BatchReceipt> {
    if request.schema_version != 1 {
        return Err(batch_error(
            BatchStage::Input,
            None,
            ApiError::InvalidInput(format!(
                "Unsupported batch schema version: {}",
                request.schema_version
            )),
        ));
    }
    let mut commands = Vec::with_capacity(request.commands.len());
    for (index, command) in request.commands.into_iter().enumerate() {
        tdector_eval::check_execution()
            .map_err(|error| batch_error(BatchStage::Command, Some(index), error.into()))?;
        commands.push(
            apply_mutation(session, command)
                .map_err(|error| batch_error(BatchStage::Command, Some(index), error))?,
        );
    }
    Ok(BatchReceipt {
        changed: commands.iter().any(|receipt| receipt.changed),
        commands,
    })
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct PreviewResult {
    pub word: String,
    pub result: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct TokenizeResult {
    pub tokens: Vec<String>,
}

pub fn preview_rule(
    session: &Session,
    selector: RuleSelector,
    word: &str,
) -> Result<PreviewResult> {
    let index = resolve_rule(session, &selector)?;
    let rule = &session.project().formation_rules[index];
    let result = Session::preview_rule(rule, word)?;
    Ok(PreviewResult {
        word: word.to_owned(),
        result,
    })
}

pub fn preview_script(script: &str, word: &str) -> Result<PreviewResult> {
    let rule = FormationRule {
        description: "Preview".into(),
        rule_type: FormationType::Nonmorphological,
        command: script.to_owned(),
        cached_ast: tdector_eval::default_cached_ast(),
    };
    let result = Session::preview_rule(&rule, word)?;
    Ok(PreviewResult {
        word: word.to_owned(),
        result,
    })
}

pub fn preview_tokenization(rule: &TokenizationRule, line: &str) -> Result<TokenizeResult> {
    if line.contains(['\r', '\n', '\u{85}', '\u{2028}', '\u{2029}']) {
        return Err(ApiError::InvalidInput(
            "Tokenizer preview accepts exactly one line".into(),
        ));
    }
    Ok(TokenizeResult {
        tokens: Session::preview_tokenization(rule, line)?,
    })
}
