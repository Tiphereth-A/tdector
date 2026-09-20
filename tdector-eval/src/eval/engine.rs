use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use crate::{AppError, AppResult};

const MAX_SCRIPT_DEPTH: usize = 500000;
const MAX_SCRIPT_OPERATIONS: u64 = 10000000;

thread_local! {
    /// Thread-local Rhai script engine for executing word formation rules and tokenization. Rhai provides a safe scripting language for transforming words and tokenizing text.
    static ENGINE: RefCell<rhai::Engine> = RefCell::new(build_engine());
    static EXECUTION: RefCell<Option<Rc<ExecutionState>>> = const { RefCell::new(None) };
}

/// Resource limits for a scoped request. Sizes limit individual Rhai values, not total process memory; cancellation and deadlines remain cooperative.
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    /// Total evaluator operations across all scripts within the scope.
    pub max_operations: u64,
    pub max_expr_depth: usize,
    pub max_call_depth: usize,
    pub max_string_bytes: usize,
    pub max_array_items: usize,
    pub max_map_entries: usize,
    pub max_variables: usize,
    pub max_functions: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_operations: 1_000_000,
            max_expr_depth: 64,
            max_call_depth: 16,
            max_string_bytes: 1_048_576,
            max_array_items: 16_384,
            max_map_entries: 4_096,
            max_variables: 1_024,
            max_functions: 256,
        }
    }
}

/// Optional execution bounds used by native adapters on their session owner thread. Ordinary CLI, GUI, and WASM callers retain their existing engine defaults.
#[derive(Debug, Clone)]
pub struct ExecutionPolicy {
    pub cancelled: Arc<AtomicBool>,
    pub deadline: Instant,
    pub limits: ExecutionLimits,
}

impl ExecutionPolicy {
    pub fn new(cancelled: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancelled,
            deadline,
            limits: ExecutionLimits::default(),
        }
    }

    /// Install this policy until the returned guard is dropped, including during unwinding. Enter on the owner thread, outside any `with_engine` closure, and drop nested guards in reverse order.
    #[must_use = "Keep the guard alive until the operation completes"]
    pub fn enter(self) -> ExecutionGuard {
        let mut engine = build_engine();
        let limits = &self.limits;
        // Rhai interprets zero as unlimited for several settings. A scoped policy must always remain bounded, even when a caller supplies zero.
        engine.set_max_operations(limits.max_operations.max(1));
        engine.set_max_expr_depths(limits.max_expr_depth.max(1), limits.max_expr_depth.max(1));
        engine.set_max_call_levels(limits.max_call_depth);
        engine.set_max_string_size(limits.max_string_bytes.max(1));
        engine.set_max_array_size(limits.max_array_items.max(1));
        engine.set_max_map_size(limits.max_map_entries.max(1));
        engine.set_max_variables(limits.max_variables);
        engine.set_max_functions(limits.max_functions);
        engine.set_max_modules(0);
        let state = Rc::new(ExecutionState {
            policy: self,
            operations: Cell::new(0),
            failure: RefCell::new(None),
        });
        let progress = Rc::clone(&state);
        engine.on_progress(move |_| progress.tick().err().map(|_| rhai::Dynamic::UNIT));
        let previous_engine = ENGINE.with(|slot| slot.replace(engine));
        let previous_execution = EXECUTION.with(|slot| slot.replace(Some(state)));
        ExecutionGuard {
            previous_engine: Some(previous_engine),
            previous_execution,
        }
    }
}

struct ExecutionState {
    policy: ExecutionPolicy,
    operations: Cell<u64>,
    failure: RefCell<Option<AppError>>,
}

impl ExecutionState {
    fn check(&self) -> AppResult<()> {
        if let Some(error) = self.failure.borrow().as_ref() {
            return Err(error.clone());
        }
        let error = if self.policy.cancelled.load(Ordering::Relaxed) {
            Some(AppError::OperationCancelled)
        } else if Instant::now() >= self.policy.deadline {
            Some(AppError::DeadlineExceeded)
        } else {
            None
        };
        if let Some(error) = error {
            self.failure.replace(Some(error.clone()));
            return Err(error);
        }
        Ok(())
    }

    fn tick(&self) -> AppResult<()> {
        self.check()?;
        if self.operations.get() >= self.policy.limits.max_operations {
            let error = AppError::LimitExceeded("total script operation budget".into());
            self.failure.replace(Some(error.clone()));
            return Err(error);
        }
        self.operations.set(self.operations.get() + 1);
        Ok(())
    }
}

/// Restores the prior thread-local engine and policy. The contained `Rc` makes this guard neither `Send` nor `Sync`, matching session ownership.
#[must_use]
pub struct ExecutionGuard {
    previous_engine: Option<rhai::Engine>,
    previous_execution: Option<Rc<ExecutionState>>,
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        if let Some(engine) = self.previous_engine.take() {
            ENGINE.with(|slot| slot.replace(engine));
            EXECUTION.with(|slot| slot.replace(self.previous_execution.take()));
        }
    }
}

/// Check cooperative cancellation and deadline at non-script checkpoints. This is a no-op unless an execution policy is active on the current thread.
pub fn check_execution() -> AppResult<()> {
    EXECUTION.with(|slot| match slot.borrow().as_ref() {
        Some(state) => state.check(),
        None => Ok(()),
    })
}

fn policy_active() -> bool {
    EXECUTION.with(|slot| slot.borrow().is_some())
}

fn parse_limit(error: &rhai::ParseErrorType) -> bool {
    matches!(
        error,
        rhai::ParseErrorType::ExprTooDeep
            | rhai::ParseErrorType::LiteralTooLarge(..)
            | rhai::ParseErrorType::TooManyFunctions
    )
}

pub(crate) fn compile_error(error: rhai::ParseError) -> AppError {
    if let Err(interruption) = check_execution() {
        return interruption;
    }
    if policy_active() && parse_limit(error.err_type()) {
        AppError::LimitExceeded(error.to_string())
    } else {
        AppError::ScriptExecutionError(format!("Rhai compilation error: {error}"))
    }
}

pub(crate) fn execution_error(context: &str, error: &rhai::EvalAltResult) -> AppError {
    if let Err(interruption) = check_execution() {
        return interruption;
    }
    let mut cause = error;
    while let rhai::EvalAltResult::ErrorInFunctionCall(_, _, inner, _)
    | rhai::EvalAltResult::ErrorInModule(_, inner, _) = cause
    {
        cause = inner;
    }
    let resource_limit = matches!(
        cause,
        rhai::EvalAltResult::ErrorTooManyOperations(..)
            | rhai::EvalAltResult::ErrorTooManyVariables(..)
            | rhai::EvalAltResult::ErrorTooManyModules(..)
            | rhai::EvalAltResult::ErrorStackOverflow(..)
            | rhai::EvalAltResult::ErrorDataTooLarge(..)
    ) || matches!(cause, rhai::EvalAltResult::ErrorParsing(kind, _) if parse_limit(kind));
    if policy_active() && resource_limit {
        AppError::LimitExceeded(format!("{context}: {error}"))
    } else {
        AppError::ScriptExecutionError(format!("{context}: {error}"))
    }
}

/// Build a Rhai script engine with security constraints. Disables dangerous operations like file I/O, network access, and system commands.
fn build_engine() -> rhai::Engine {
    let mut engine = rhai::Engine::new();

    // Script output must not corrupt a CLI's JSON or a stdio protocol stream.
    #[cfg(not(target_arch = "wasm32"))]
    {
        engine.on_print(|message| eprintln!("{message}"));
        engine.on_debug(|message, source, position| {
            eprintln!("{} @ {position:?} | {message}", source.unwrap_or("script"));
        });
    }
    // Browser hosts have no stderr; scripts communicate through their return values.
    #[cfg(target_arch = "wasm32")]
    {
        engine.on_print(|_| {});
        engine.on_debug(|_, _, _| {});
    }

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
