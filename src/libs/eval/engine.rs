use std::cell::RefCell;

use crate::consts::domain::{MAX_SCRIPT_DEPTH, MAX_SCRIPT_OPERATIONS};

thread_local! {
    /// Thread-local Rhai script engine for executing word formation rules and tokenization.
    /// Rhai provides a safe scripting language for transforming words and tokenizing text.
    static ENGINE: RefCell<rhai::Engine> = RefCell::new(build_engine());
}

/// Build a Rhai script engine with security constraints.
/// Disables dangerous operations like file I/O, network access, and system commands.
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

/// Execute a closure with access to the shared Rhai engine
pub fn with_engine<R>(f: impl FnOnce(&rhai::Engine) -> R) -> R {
    ENGINE.with(|engine| f(&engine.borrow()))
}
