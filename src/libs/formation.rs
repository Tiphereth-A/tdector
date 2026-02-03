//! Word formation rules and Rhai script execution.
//!
//! This module handles word formation transformations using Rhai scripting:
//! - **`FormationRule`**: Represents derivation, inflection, and other morphological rules
//! - **Rhai Engine**: Thread-local script execution engine with security constraints
//! - **Script Caching**: Compiled AST caching for performance optimization

use serde::{Deserialize, Serialize};
use std::cell::{OnceCell, RefCell};
use std::sync::Arc;

use crate::consts::domain::{MAX_SCRIPT_DEPTH, MAX_SCRIPT_OPERATIONS};
use crate::enums::{AppError, AppResult, FormationType};

thread_local! {
    static ENGINE: RefCell<rhai::Engine> = RefCell::new(build_engine());
}

/// Creates a configured Rhai engine with security constraints.
///
/// The engine is configured with:
/// - Expression depth limits to prevent deeply nested code
/// - Operation limits to prevent infinite loops
/// - Disabled I/O, network, and system operations for security
///
/// # Returns
///
/// A configured Rhai engine ready for script execution
fn build_engine() -> rhai::Engine {
    let mut engine = rhai::Engine::new();

    engine.set_max_expr_depths(MAX_SCRIPT_DEPTH, MAX_SCRIPT_DEPTH);
    engine.set_max_operations(MAX_SCRIPT_OPERATIONS);

    engine.disable_symbol("eval");
    engine.disable_symbol("load");
    engine.disable_symbol("save");
    engine.disable_symbol("read");
    engine.disable_symbol("write");
    engine.disable_symbol("append");
    engine.disable_symbol("delete");
    engine.disable_symbol("copy");

    engine.disable_symbol("http");
    engine.disable_symbol("request");
    engine.disable_symbol("fetch");
    engine.disable_symbol("socket");
    engine.disable_symbol("tcp");
    engine.disable_symbol("udp");

    engine.disable_symbol("system");
    engine.disable_symbol("exec");
    engine.disable_symbol("spawn");
    engine.disable_symbol("command");

    engine
}

/// Executes a function with access to the thread-local Rhai engine.
///
/// This provides safe access to the engine while maintaining thread-local isolation.
///
/// # Arguments
///
/// * `f` - A closure that receives a reference to the Rhai engine
///
/// # Returns
///
/// The result of the closure
pub fn with_engine<R>(f: impl FnOnce(&rhai::Engine) -> R) -> R {
    ENGINE.with(|engine| f(&engine.borrow()))
}

/// Creates a default cached AST cell for `FormationRule`.
///
/// This is used as the default value for the `cached_ast` field when deserializing.
pub fn default_cached_ast() -> Arc<OnceCell<rhai::AST>> {
    Arc::new(OnceCell::new())
}

/// A word formation rule describing how to transform a word.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationRule {
    /// Human-readable description of the formation rule
    pub description: String,
    /// Type of formation (derivation, inflection, or nonmorphological)
    #[serde(rename = "type")]
    pub rule_type: FormationType,
    /// Rhai script command to apply the transformation
    pub command: String,
    /// Cached AST for the compiled Rhai script
    #[serde(skip, default = "default_cached_ast")]
    pub cached_ast: Arc<OnceCell<rhai::AST>>,
}

impl FormationRule {
    /// Execute the formation rule on a word using Rhai script engine.
    ///
    /// The Rhai script should define a `transform(word)` function that takes
    /// a string and returns the transformed word.
    ///
    /// # Arguments
    ///
    /// * `word` - The word to transform
    ///
    /// # Returns
    ///
    /// The transformed word if successful, or an error if execution fails
    ///
    /// # Example
    ///
    /// For a rule with command: `fn transform(word) { word + "s" }`
    /// Calling `rule.apply("apple")` would return `Ok("apples".to_string())`
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
