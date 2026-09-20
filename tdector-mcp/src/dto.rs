//! Transport-owned contracts. Project strings remain data, never instructions.

use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tdector_app::api;

pub const EDIT_LIFECYCLE: &str = "Edits are in memory only. Call project_save explicitly; unsaved changes are lost on reload with discard_changes or process exit.";

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Limits {
    pub project_bytes: usize,
    pub message_bytes: usize,
    pub result_bytes: usize,
    pub queued_operations: usize,
    pub batch_commands: usize,
    pub page_size: usize,
    pub default_page_size: usize,
    pub operation_timeout_ms: u64,
    pub output_write_timeout_ms: u64,
    pub shutdown_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SessionMeta {
    pub session_id: String,
    #[schemars(regex(pattern = "^[0-9]+$"))]
    pub revision: String,
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    InvalidIndex,
    NotFound,
    ReadOnly,
    SessionExpired,
    RevisionConflict,
    UnsavedChanges,
    InputChanged,
    IoError,
    ScriptError,
    ResultTooLarge,
    LimitExceeded,
    DeadlineExceeded,
    InternalError,
    CommittedStateError,
    ServerBusy,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ToolError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<api::BatchStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committed: Option<bool>,
}

impl ToolError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            stage: None,
            command_index: None,
            committed: None,
        }
    }

    pub fn committed(message: impl Into<String>) -> Self {
        Self {
            committed: Some(true),
            ..Self::new(ErrorCode::CommittedStateError, message)
        }
    }
}

impl From<tdector_eval::AppError> for ToolError {
    fn from(error: tdector_eval::AppError) -> Self {
        use tdector_eval::AppError;
        let code = match &error {
            AppError::InvalidProjectFormat(_) => ErrorCode::InvalidInput,
            AppError::ScriptExecutionError(_) => ErrorCode::ScriptError,
            AppError::IoError(_) => ErrorCode::IoError,
            AppError::DeadlineExceeded | AppError::OperationCancelled => {
                ErrorCode::DeadlineExceeded
            }
            AppError::LimitExceeded(_) => ErrorCode::LimitExceeded,
        };
        Self::new(code, error.to_string())
    }
}

impl From<tdector_app::Error> for ToolError {
    fn from(error: tdector_app::Error) -> Self {
        match error {
            tdector_app::Error::InvalidInput(message) => {
                Self::new(ErrorCode::InvalidInput, message)
            }
            tdector_app::Error::InvalidIndex { kind, index } => Self::new(
                ErrorCode::InvalidIndex,
                format!("Invalid {kind} index: {index}"),
            ),
            tdector_app::Error::Operation(error) => error.into(),
        }
    }
}

impl From<api::ApiError> for ToolError {
    fn from(error: api::ApiError) -> Self {
        match error {
            api::ApiError::Application(error) => error.into(),
            api::ApiError::Batch {
                stage,
                command_index,
                source,
            } => {
                let mut error = Self::from(*source);
                error.stage = Some(stage);
                error.command_index = command_index;
                error
            }
            api::ApiError::NotFound { .. } | api::ApiError::RuleNotFound { .. } => {
                Self::new(ErrorCode::NotFound, error.to_string())
            }
            _ => Self::new(ErrorCode::InvalidInput, error.to_string()),
        }
    }
}

impl From<tdector_io::IoError> for ToolError {
    fn from(error: tdector_io::IoError) -> Self {
        use tdector_io::IoError;
        let code = match &error {
            IoError::InputChanged { .. } => ErrorCode::InputChanged,
            IoError::InvalidPath { .. } | IoError::InvalidUtf8(_) => ErrorCode::InvalidInput,
            IoError::LimitExceeded { .. } => ErrorCode::LimitExceeded,
            IoError::CommittedState { .. } => return Self::committed(error.to_string()),
            _ => ErrorCode::IoError,
        };
        Self::new(code, error.to_string())
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Success<T> {
    #[schemars(range(min = 1, max = 1))]
    schema_version: u32,
    ok: bool,
    session: SessionMeta,
    data: T,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    #[schemars(range(min = 1, max = 1))]
    schema_version: u32,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<SessionMeta>,
    error: ToolError,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Envelope<T> {
    Success(Success<T>),
    Failure(Failure),
}

pub fn success<T: Serialize>(
    session: SessionMeta,
    data: T,
    limit: usize,
) -> Result<CallToolResult, ToolError> {
    let value = serde_json::to_value(Envelope::Success(Success {
        schema_version: 1,
        ok: true,
        session,
        data,
    }))
    .map_err(|e| ToolError::new(ErrorCode::InternalError, e.to_string()))?;
    let result = CallToolResult::structured(value);
    let size = serde_json::to_vec(&result)
        .map_err(|e| ToolError::new(ErrorCode::InternalError, e.to_string()))?
        .len();
    if size > limit {
        return Err(ToolError::new(
            ErrorCode::ResultTooLarge,
            format!(
                "Complete tool result is {size} bytes; limit is {limit}. Use a smaller page or a narrower query. No staged changes were committed."
            ),
        ));
    }
    Ok(result)
}

pub fn failure(session: Option<SessionMeta>, mut error: ToolError, limit: usize) -> CallToolResult {
    // Diagnostics may contain large user strings. Keep the typed error and coordinates while bounding the message (project text is never silently truncated in success).
    let mut length = error.message.len().min(limit / 8);
    while !error.message.is_char_boundary(length) {
        length -= 1;
    }
    error.message.truncate(length);
    loop {
        let value = serde_json::to_value(Envelope::<serde_json::Value>::Failure(Failure {
            schema_version: 1,
            ok: false,
            session: session.clone(),
            error: error.clone(),
        }))
        .expect("error envelope contains only JSON-compatible values");
        let result = CallToolResult::structured_error(value);
        if serde_json::to_vec(&result).expect("JSON result").len() <= limit {
            return result;
        }
        // Count JSON escaping in both copies, not only UTF-8 source bytes.
        let mut length = error.message.len() / 2;
        while !error.message.is_char_boundary(length) {
            length -= 1;
        }
        assert!(
            !error.message.is_empty(),
            "startup minimum result size fits an empty error envelope"
        );
        error.message.truncate(length);
    }
}

#[derive(Serialize, JsonSchema)]
pub struct ProjectInfo {
    #[serde(flatten)]
    pub project: api::InfoResult,
    pub writable: bool,
    pub limits: Limits,
    pub execution_limits: EvaluatorLimits,
    pub edit_lifecycle: String,
}

/// The fixed evaluator policy is reported separately from startup transport limits.
#[derive(Serialize, JsonSchema)]
pub struct EvaluatorLimits {
    pub max_operations: u64,
    pub max_expr_depth: usize,
    pub max_call_depth: usize,
    pub max_string_bytes: usize,
    pub max_array_items: usize,
    pub max_map_entries: usize,
    pub max_variables: usize,
    pub max_functions: usize,
}

impl Default for EvaluatorLimits {
    fn default() -> Self {
        let limits = tdector_eval::ExecutionLimits::default();
        Self {
            max_operations: limits.max_operations,
            max_expr_depth: limits.max_expr_depth,
            max_call_depth: limits.max_call_depth,
            max_string_bytes: limits.max_string_bytes,
            max_array_items: limits.max_array_items,
            max_map_entries: limits.max_map_entries,
            max_variables: limits.max_variables,
            max_functions: limits.max_functions,
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct ExportResult {
    pub content: String,
    pub mime_type: String,
}

#[derive(Serialize, JsonSchema)]
pub struct EditResult {
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub would_change: Option<bool>,
    pub saved: bool,
    pub dry_run: bool,
    pub commands: Vec<api::MutationReceipt>,
    pub edit_lifecycle: String,
}

#[derive(Serialize, JsonSchema)]
pub struct SaveResult {
    pub saved: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct ReloadResult {
    pub reloaded: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

macro_rules! read_args {
    ($name:ident { $($fields:tt)* }) => {
        #[derive(Debug, Deserialize, JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub session_id: String,
            #[serde(default)]
            #[schemars(regex(pattern = "^[0-9]+$"))]
            pub expected_revision: Option<String>,
            $($fields)*
        }
    };
}

fn default_limit() -> usize {
    50
}
read_args!(ReadArgs {});
read_args!(SegmentsArgs {
    #[serde(default)] pub filter: String,
    #[serde(default)] pub sort: api::SegmentSort,
    #[serde(default)] pub descending: bool,
    #[serde(default)] pub offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 200))] pub limit: usize,
});
read_args!(SegmentArgs { pub segment_index: usize, });
read_args!(PageArgs {
    #[serde(default)] pub offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 200))] pub limit: usize,
});
read_args!(WordArgs { pub word: String, });
read_args!(LookupArgs {
    pub word: String,
    #[serde(default)] pub kind: api::LookupKind,
    #[serde(default)] pub offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 200))] pub limit: usize,
});

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteArgs {
    pub session_id: String,
    #[schemars(regex(pattern = "^[0-9]+$"))]
    pub expected_revision: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReloadArgs {
    pub session_id: String,
    #[schemars(regex(pattern = "^[0-9]+$"))]
    pub expected_revision: String,
    #[serde(default)]
    pub discard_changes: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditArgs {
    pub session_id: String,
    #[schemars(regex(pattern = "^[0-9]+$"))]
    pub expected_revision: String,
    pub batch: AnnotationBatch,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotationBatch {
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    #[schemars(length(min = 1, max = 100))]
    pub commands: Vec<Annotation>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
#[expect(
    clippy::enum_variant_names,
    reason = "Annotation names mirror the existing versioned batch API"
)]
pub enum Annotation {
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
}

impl From<Annotation> for api::Mutation {
    fn from(value: Annotation) -> Self {
        match value {
            Annotation::SetGloss { word, meaning } => Self::SetGloss { word, meaning },
            Annotation::SetTranslation {
                segment_index,
                translation,
            } => Self::SetTranslation {
                segment_index,
                translation,
            },
            Annotation::SetComment {
                segment_index,
                token_index,
                comment,
            } => Self::SetComment {
                segment_index,
                token_index,
                comment,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_budget_counts_escaping_and_duplicate_text() {
        let error = ToolError {
            stage: Some(api::BatchStage::Command),
            command_index: Some(2),
            ..ToolError::new(ErrorCode::InvalidInput, "\0\u{1f}\"\\猫".repeat(10000))
        };
        let result = failure(
            Some(SessionMeta {
                session_id: uuid::Uuid::new_v4().to_string(),
                revision: "1".into(),
                dirty: true,
            }),
            error,
            4096,
        );
        assert!(serde_json::to_vec(&result).expect("encode result").len() <= 4096);
        assert_eq!(result.is_error, Some(true));
        let value = result.structured_content.expect("structured error");
        assert_eq!(value["error"]["code"], "invalid_input");
        assert_eq!(value["error"]["command_index"], 2);
    }
}
