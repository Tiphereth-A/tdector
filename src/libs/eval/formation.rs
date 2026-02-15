use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::sync::Arc;

use super::engine::with_engine;
use crate::enums::{AppError, AppResult, FormationType};

/// Create a new empty cached AST (Abstract Syntax Tree) placeholder
pub fn default_cached_ast() -> Arc<OnceCell<rhai::AST>> {
    Arc::new(OnceCell::new())
}

/// A word formation rule that transforms base words using a Rhai script.
/// Rules can be categorized as derivational, inflectional, or non-morphological transformations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationRule {
    /// Human-readable description of what this rule does
    pub description: String,

    /// Category of rule: Derivation, Inflection, or `NonMorphological`
    #[serde(rename = "type")]
    pub rule_type: FormationType,

    /// Rhai script that implements the transformation.
    /// Must define a `transform(word: String) -> String` function.
    pub command: String,

    /// Compiled AST of the Rhai script, cached for performance.
    /// Lazily compiled on first execution and reused thereafter.
    #[serde(skip, default = "default_cached_ast")]
    pub cached_ast: Arc<OnceCell<rhai::AST>>,
}

impl FormationRule {
    /// Apply this rule to a word, returning the transformed result or an error.
    /// The first call will compile and cache the Rhai script; subsequent calls reuse it.
    pub fn apply(&self, word: &str) -> AppResult<String> {
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

            let result: String = engine
                .call_fn(
                    &mut rhai::Scope::new(),
                    ast,
                    "transform",
                    (word.to_string(),),
                )
                .map_err(|e| {
                    AppError::ScriptExecutionError(format!("Transform function error: {e}"))
                })?;

            Ok(result)
        })
    }
}
