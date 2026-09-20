use std::fmt;

/// Error types that can occur during application execution
#[derive(Debug, Clone)]
pub enum AppError {
    /// File system I/O errors (reading, writing, dialog cancellation)
    IoError(String),

    /// Project JSON format is invalid or corrupted
    InvalidProjectFormat(String),

    /// Rhai script compilation or execution failed
    ScriptExecutionError(String),

    /// User cancelled an operation (e.g., file dialog)
    OperationCancelled,

    /// A scoped operation exhausted its wall-clock deadline.
    DeadlineExceeded,

    /// A scoped operation exceeded a configured resource bound.
    LimitExceeded(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(msg) => write!(f, "File I/O error: {msg}"),
            Self::InvalidProjectFormat(msg) => {
                write!(f, "Invalid project format: {msg}")
            }
            Self::ScriptExecutionError(msg) => write!(f, "Script error: {msg}"),
            Self::OperationCancelled => write!(f, "Operation cancelled by user"),
            Self::DeadlineExceeded => write!(f, "Operation deadline exceeded"),
            Self::LimitExceeded(msg) => write!(f, "Operation limit exceeded: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}
