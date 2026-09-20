//! Stable adapter errors, mapped from typed application and I/O failures.

use serde::Serialize;
use serde_json::{Value, json};
use tdector_app::api::{ApiError, BatchStage};
use tdector_eval::AppError;

use crate::io::IoError;

pub type Result<T> = std::result::Result<T, Failure>;

#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<BatchStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_index: Option<usize>,
    #[serde(skip)]
    pub exit_code: u8,
}

impl Failure {
    pub fn new(code: &'static str, message: impl Into<String>, exit_code: u8) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            stage: None,
            command_index: None,
            exit_code,
        }
    }
    pub fn syntax(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message, 2)
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal_error", message, 1)
    }
    pub fn at_stage(mut self, stage: BatchStage) -> Self {
        if self.stage.is_none() {
            self.stage = Some(stage);
        }
        self
    }
}

impl From<tdector_app::Error> for Failure {
    fn from(error: tdector_app::Error) -> Self {
        use tdector_app::Error;
        let message = error.to_string();
        match error {
            Error::InvalidInput(_) => Self::new("invalid_input", message, 3),
            Error::InvalidIndex { kind, index } => {
                let mut failure = Self::new("invalid_index", message, 3);
                failure.details = Some(json!({"kind": kind, "index": index}));
                failure
            }
            Error::Operation(AppError::InvalidProjectFormat(_)) => {
                Self::new("invalid_project", message, 3)
            }
            Error::Operation(AppError::ScriptExecutionError(_)) => {
                Self::new("script_error", message, 4)
            }
            Error::Operation(AppError::IoError(_)) => Self::new("io_error", message, 5),
            Error::Operation(AppError::OperationCancelled) => Self::internal(message),
            Error::Operation(AppError::DeadlineExceeded) => {
                Self::new("deadline_exceeded", message, 4)
            }
            Error::Operation(AppError::LimitExceeded(_)) => Self::new("limit_exceeded", message, 4),
        }
    }
}

impl From<ApiError> for Failure {
    fn from(error: ApiError) -> Self {
        let message = error.to_string();
        match error {
            ApiError::Application(error) => error.into(),
            ApiError::InvalidInput(_) => Self::new("invalid_input", message, 3),
            ApiError::NotFound { kind, value } => {
                let mut failure = Self::new("invalid_input", message, 3);
                failure.details = Some(json!({"kind": kind, "value": value}));
                failure
            }
            ApiError::RuleNotFound { description } => {
                let mut failure = Self::new("rule_not_found", message, 3);
                failure.details = Some(json!({"description": description}));
                failure
            }
            ApiError::AmbiguousRule {
                description,
                rule_indices,
            } => {
                let mut failure = Self::new("ambiguous_rule", message, 3);
                failure.details =
                    Some(json!({"description": description, "rule_indices": rule_indices}));
                failure
            }
            ApiError::Batch {
                stage,
                command_index,
                source,
            } => {
                let mut failure = Self::from(*source);
                failure.stage = Some(stage);
                failure.command_index = command_index;
                failure
            }
        }
    }
}

impl From<IoError> for Failure {
    fn from(error: IoError) -> Self {
        let message = error.to_string();
        match error {
            IoError::Io(_) => Self::new("io_error", message, 5),
            IoError::InvalidUtf8(_) => Self::new("invalid_input", message, 3),
            IoError::InvalidPath { .. } => Self::new("invalid_input", message, 3),
            IoError::LimitExceeded { .. } => Self::new("limit_exceeded", message, 3),
            IoError::CommittedState { .. } => Self::new("committed_state_error", message, 1),
            IoError::OutputExists { path } => {
                let mut failure = Self::new("output_exists", message, 6);
                failure.details = Some(json!({"path": path}));
                failure
            }
            IoError::InputChanged { path } => {
                let mut failure = Self::new("input_changed", message, 6);
                failure.details = Some(json!({"path": path}));
                failure
            }
        }
    }
}

impl From<serde_json::Error> for Failure {
    fn from(error: serde_json::Error) -> Self {
        Self::internal(format!("Cannot serialize report: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unexpected_failures_keep_the_internal_exit_category_and_public_envelope() {
        let failure = Failure::from(tdector_app::Error::Operation(AppError::OperationCancelled));
        assert_eq!(failure.exit_code, 1);
        let public = serde_json::to_value(failure).expect("serialize internal error");
        assert_eq!(public["code"], "internal_error");
        assert!(public.get("exit_code").is_none());
        assert!(public.get("command_index").is_none());
    }
}
