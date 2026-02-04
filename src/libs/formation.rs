use serde::{Deserialize, Serialize};
use std::cell::{OnceCell, RefCell};
use std::sync::Arc;

use crate::consts::domain::{MAX_SCRIPT_DEPTH, MAX_SCRIPT_OPERATIONS};
use crate::enums::{AppError, AppResult, FormationType};

thread_local! {
    /// Thread-local Rhai script engine for executing word formation rules.
    /// Rhai provides a safe scripting language for transforming words based on rules.
    static ENGINE: RefCell<rhai::Engine> = RefCell::new(build_engine());
}

/// Build a Rhai script engine with security constraints.
/// Disables dangerous operations like file I/O, network access, and system commands.
fn build_engine() -> rhai::Engine {
    let mut engine = rhai::Engine::new();

    // Set maximum expression depth and operation count to prevent infinite loops
    engine.set_max_expr_depths(MAX_SCRIPT_DEPTH, MAX_SCRIPT_DEPTH);
    engine.set_max_operations(MAX_SCRIPT_OPERATIONS);

    // Disable dangerous file I/O operations
    engine.disable_symbol("eval");
    engine.disable_symbol("load");
    engine.disable_symbol("save");
    engine.disable_symbol("read");
    engine.disable_symbol("write");
    engine.disable_symbol("append");
    engine.disable_symbol("delete");
    engine.disable_symbol("copy");

    // Disable network operations
    engine.disable_symbol("http");
    engine.disable_symbol("request");
    engine.disable_symbol("fetch");
    engine.disable_symbol("socket");
    engine.disable_symbol("tcp");
    engine.disable_symbol("udp");

    // Disable system commands
    engine.disable_symbol("system");
    engine.disable_symbol("exec");
    engine.disable_symbol("spawn");
    engine.disable_symbol("command");

    engine
}

/// Execute a closure with access to the shared Rhai engine
pub fn with_engine<R>(f: impl FnOnce(&rhai::Engine) -> R) -> R {
    ENGINE.with(|engine| f(&engine.borrow()))
}

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

    /// Category of rule: Derivation, Inflection, or NonMorphological
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
            // Compile and cache the script if not already done
            if self.cached_ast.get().is_none() {
                let ast = engine.compile(&self.command).map_err(|e| {
                    AppError::ScriptExecutionError(format!("Rhai compilation error: {e}"))
                })?;
                let _ = self.cached_ast.set(ast);
            }

            let ast = self.cached_ast.get().ok_or_else(|| {
                AppError::ScriptExecutionError("Failed to cache Rhai AST".to_string())
            })?;

            // Execute the transform function with the word as parameter
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
