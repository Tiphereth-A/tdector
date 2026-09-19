use tdector_eval::{AppError, TokenizationRule, default_cached_ast};

#[test]
fn tokenization_rejects_non_string_results_instead_of_dropping_them() {
    let rule = TokenizationRule {
        description: "Invalid token output".into(),
        command: "fn tokenize(line) { [line, 42] }".into(),
        cached_ast: default_cached_ast(),
    };
    let error = rule
        .tokenize("word")
        .expect_err("every token must be a string");
    assert!(matches!(error, AppError::ScriptExecutionError(_)));
    assert!(error.to_string().contains("index 1"));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn script_diagnostics_leave_stdout_available_for_protocol_output() {
    const CHILD_FLAG: &str = "TDECTOR_TEST_SCRIPT_OUTPUT_CHILD";
    const MARKER: &str = "tdector-script-diagnostic-marker";
    if std::env::var_os(CHILD_FLAG).is_some() {
        tdector_eval::with_engine(|engine| {
            engine.eval::<()>(
                "print(\"tdector-script-diagnostic-marker\"); debug(\"tdector-script-diagnostic-marker\");",
            )
        })
        .expect("diagnostic script should run");
        return;
    }

    let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "script_diagnostics_leave_stdout_available_for_protocol_output",
            "--nocapture",
        ])
        .env(CHILD_FLAG, "1")
        .output()
        .expect("diagnostic subprocess should run");
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains(MARKER));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .matches(MARKER)
            .count(),
        2
    );
}
