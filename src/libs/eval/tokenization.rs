use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::sync::Arc;

use super::engine::with_engine;
use super::formation::default_cached_ast;
use crate::enums::{AppError, AppResult};

/// A tokenization rule that splits text into tokens using a Rhai script.
/// The script receives a line of text and returns an array of token strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizationRule {
    /// Human-readable description of what this tokenization strategy does
    pub description: String,

    /// Rhai script that implements the tokenization logic.
    /// Must define a `tokenize(line: String) -> Array` function that returns an array of strings.
    pub command: String,

    /// Compiled AST of the Rhai script, cached for performance.
    /// Lazily compiled on first execution and reused thereafter.
    #[serde(skip, default = "default_cached_ast")]
    pub cached_ast: Arc<OnceCell<rhai::AST>>,
}

impl TokenizationRule {
    /// Apply this tokenization rule to a line of text, returning a vector of token strings.
    /// The first call will compile and cache the Rhai script; subsequent calls reuse it.
    pub fn tokenize(&self, line: &str) -> AppResult<Vec<String>> {
        with_engine(|engine| {
            if self.cached_ast.get().is_none() {
                let ast = engine.compile(&self.command).map_err(|e| {
                    AppError::ScriptExecutionError(format!("Rhai compilation error: {e}"))
                })?;
                let _ = self.cached_ast.set(ast);
            }

            let ast = self.cached_ast.get().ok_or_else(|| {
                AppError::ScriptExecutionError("Failed to cache Rhai AST".to_string())
            })?;

            let result: rhai::Array = engine
                .call_fn(
                    &mut rhai::Scope::new(),
                    ast,
                    "tokenize",
                    (line.to_string(),),
                )
                .map_err(|e| {
                    AppError::ScriptExecutionError(format!("Tokenize function error: {e}"))
                })?;

            let tokens: Vec<String> = result
                .into_iter()
                .filter_map(|item| item.into_string().ok())
                .collect();

            Ok(tokens)
        })
    }

    pub fn default_whitespace() -> Self {
        Self {
            description: "Split by whitespace".to_string(),
            command: r#"
fn tokenize(line) {
    let tokens = line.split();
    tokens
}
"#
            .to_string(),
            cached_ast: default_cached_ast(),
        }
    }

    pub fn default_character() -> Self {
        Self {
            description: "Split by character".to_string(),
            command: r#"
fn tokenize(line) {
    let tokens = [];
    for ch in line {
        tokens.push(ch.to_string());
    }
    tokens
}
"#
            .to_string(),
            cached_ast: default_cached_ast(),
        }
    }
}
