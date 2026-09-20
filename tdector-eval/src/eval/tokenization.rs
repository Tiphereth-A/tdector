use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::rc::Rc;

use super::engine::{check_execution, compile_error, execution_error, with_engine};
use super::formation::default_cached_ast;
use crate::{AppError, AppResult};

/// A tokenization rule that splits text into tokens using a Rhai script. The script receives a line of text and returns an array of token strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizationRule {
    /// Human-readable description of what this tokenization strategy does
    pub description: String,

    /// Rhai script that implements the tokenization logic. Must define a `tokenize(line: String) -> Array` function that returns an array of strings.
    pub command: String,

    /// Compiled AST of the Rhai script, cached for performance. Lazily compiled on first execution and reused thereafter.
    #[serde(skip, default = "default_cached_ast")]
    pub cached_ast: Rc<OnceCell<rhai::AST>>,
}

impl TokenizationRule {
    /// Apply this tokenization rule to a line of text, returning a vector of token strings. The first call will compile and cache the Rhai script; subsequent calls reuse it.
    pub fn tokenize(&self, line: &str) -> AppResult<Vec<String>> {
        check_execution()?;
        with_engine(|engine| {
            if self.cached_ast.get().is_none() {
                let ast = engine.compile(&self.command).map_err(compile_error)?;
                check_execution()?;
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
                .map_err(|e| execution_error("Tokenize function error", &e))?;

            let tokens: Vec<String> = result
                .into_iter()
                .enumerate()
                .map(|(index, item)| {
                    check_execution()?;
                    item.into_string().map_err(|_| {
                        AppError::ScriptExecutionError(format!(
                            "Tokenize function returned a non-string token at index {index}"
                        ))
                    })
                })
                .collect::<AppResult<_>>()?;

            check_execution()?;
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
