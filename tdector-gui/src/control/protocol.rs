//! Owned wire messages. No application, Rhai, or egui state crosses the transport.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::PathBuf;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
pub const QUEUE_CAPACITY: usize = 64;
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub type Result<T> = std::result::Result<T, ControlError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    pub exit_code: u8,
}
impl ControlError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, exit_code: u8) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            exit_code,
        }
    }
    pub fn session(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, 7)
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message, 3)
    }
}
impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ControlError {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol_version: u32,
    pub token: String,
    pub command: String,
    pub args: Value,
    pub timeout_ms: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub protocol_version: u32,
    pub result: Result<Value>,
}
impl Response {
    pub fn new(result: Result<Value>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            result,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupConfig {
    pub project: Option<PathBuf>,
    pub font: Option<PathBuf>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}
