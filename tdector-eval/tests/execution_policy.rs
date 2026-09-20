use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use tdector_eval::{
    AppError, ExecutionPolicy, FormationRule, FormationType, TokenizationRule, check_execution,
    default_cached_ast, with_engine,
};

fn policy() -> ExecutionPolicy {
    ExecutionPolicy::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    )
}

fn formation(command: &str) -> FormationRule {
    FormationRule {
        description: "Scoped execution test".into(),
        rule_type: FormationType::Inflection,
        command: command.into(),
        cached_ast: default_cached_ast(),
    }
}

#[test]
fn cancelled_and_expired_scopes_fail_before_compilation() {
    let rule = formation("deliberately invalid script");
    let cancelled = policy();
    cancelled.cancelled.store(true, Ordering::Relaxed);
    {
        let _guard = cancelled.enter();
        assert!(matches!(
            rule.apply("cat"),
            Err(AppError::OperationCancelled)
        ));
    }
    {
        let _guard = ExecutionPolicy::new(Arc::new(AtomicBool::new(false)), Instant::now()).enter();
        assert!(matches!(rule.apply("cat"), Err(AppError::DeadlineExceeded)));
    }
    assert!(matches!(
        rule.apply("cat"),
        Err(AppError::ScriptExecutionError(_))
    ));
}

#[test]
fn a_running_script_observes_cancellation_from_another_thread() {
    let mut scope = policy();
    scope.limits.max_operations = u64::MAX;
    let cancelled = Arc::clone(&scope.cancelled);
    let cancel_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        cancelled.store(true, Ordering::Relaxed);
    });
    let _guard = scope.enter();
    let error = formation("fn transform(word) { loop {} word }")
        .apply("cat")
        .expect_err("cancel infinite script");
    cancel_thread.join().expect("cancellation thread");
    assert!(matches!(error, AppError::OperationCancelled), "{error}");
}

#[test]
fn a_running_script_observes_its_deadline() {
    let mut scope = ExecutionPolicy::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_millis(20),
    );
    scope.limits.max_operations = u64::MAX;
    let _guard = scope.enter();
    let error = formation("fn transform(word) { loop {} word }")
        .apply("cat")
        .expect_err("stop infinite script");
    assert!(matches!(error, AppError::DeadlineExceeded), "{error}");
}

#[test]
fn allocation_and_depth_failures_have_typed_limit_errors() {
    let mut scope = policy();
    scope.limits.max_string_bytes = 16;
    scope.limits.max_array_items = 8;
    scope.limits.max_map_entries = 4;
    scope.limits.max_expr_depth = 32;
    scope.limits.max_call_depth = 4;
    let _guard = scope.enter();
    for script in [
        "fn transform(word) { let s = word; for n in 0..8 { s += s; } s }".to_string(),
        "fn transform(word) { let m = #{a: 1, b: 2, c: 3, d: 4, e: 5}; word }".to_string(),
        "fn recurse(x) { recurse(x) } fn transform(word) { recurse(word) }".to_string(),
        format!(
            "fn transform(word) {{ {}word{} }}",
            "(".repeat(100),
            ")".repeat(100)
        ),
    ] {
        let error = formation(&script)
            .apply("cat")
            .expect_err("configured resource limit");
        assert!(matches!(error, AppError::LimitExceeded(_)), "{error}");
    }
    let tokenizer = TokenizationRule {
        description: "Large array".into(),
        command: "fn tokenize(line) { let a = []; for n in 0..9 { a.push(line); } a }".into(),
        cached_ast: default_cached_ast(),
    };
    let error = tokenizer.tokenize("cat").expect_err("array resource limit");
    assert!(matches!(error, AppError::LimitExceeded(_)), "{error}");
}

#[test]
fn script_operation_budget_is_shared_across_rule_calls() {
    let mut scope = policy();
    scope.limits.max_operations = 64;
    let _guard = scope.enter();
    let rule = formation("fn transform(word) { word + \"s\" }");
    let mut successful_calls = 0;
    loop {
        match rule.apply("cat") {
            Ok(_) => successful_calls += 1,
            Err(AppError::LimitExceeded(_)) => break,
            Err(error) => panic!("unexpected failure: {error}"),
        }
        assert!(successful_calls < 64, "budget must cover the entire scope");
    }
    assert!(successful_calls > 0);
}

#[test]
fn scoped_policy_restores_defaults_after_panics_and_on_other_threads() {
    let previous = with_engine(|engine| {
        (
            engine.max_expr_depth(),
            engine.max_operations(),
            engine.max_string_size(),
            engine.max_call_levels(),
        )
    });
    let caught = std::panic::catch_unwind(|| {
        let scope = policy();
        scope.cancelled.store(true, Ordering::Relaxed);
        let _guard = scope.enter();
        assert!(matches!(
            check_execution(),
            Err(AppError::OperationCancelled)
        ));
        std::thread::spawn(|| assert!(check_execution().is_ok()))
            .join()
            .expect("independent thread");
        with_engine(|_| panic!("exercise scope cleanup"));
    });
    assert!(caught.is_err());
    assert!(check_execution().is_ok());
    assert_eq!(
        previous,
        with_engine(|engine| (
            engine.max_expr_depth(),
            engine.max_operations(),
            engine.max_string_size(),
            engine.max_call_levels()
        ))
    );
    assert_eq!(
        formation("fn transform(word) { word + \"s\" }")
            .apply("cat")
            .expect("original engine"),
        "cats"
    );
}

#[test]
fn nested_scopes_restore_the_outer_policy() {
    let scope = policy();
    let cancelled = Arc::clone(&scope.cancelled);
    let _outer = scope.enter();
    {
        let _inner = policy().enter();
        cancelled.store(true, Ordering::Relaxed);
        assert!(check_execution().is_ok());
    }
    assert!(matches!(
        check_execution(),
        Err(AppError::OperationCancelled)
    ));
}
