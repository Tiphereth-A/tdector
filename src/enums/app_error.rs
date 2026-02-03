use std::fmt;

/// Application error type combining all possible error scenarios.
///
/// This enum wraps errors from various subsystems into a single type
/// that can be easily converted to user-facing messages.
#[derive(Debug, Clone)]
pub enum AppError {
    /// File I/O error
    IoError(String),
    /// Invalid project format or corrupted data
    InvalidProjectFormat(String),
    /// Script execution (Rhai) failed
    ScriptExecutionError(String),
    /// Dialog or file picker was cancelled
    OperationCancelled,
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
        }
    }
}

impl std::error::Error for AppError {}
