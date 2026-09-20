use serde_json::{Value, json};
use tdector_eval::AppError;
use tdector_file::project::{convert_to_saved_project, load_project_from_json};

fn project_json() -> Value {
    json!({
        "version": 2,
        "project_name": "Plural forms",
        "formation": [{
            "description": "Plural",
            "type": "inflection",
            "command": "fn transform(word) { word + \"s\" }"
        }],
        "vocabulary": {
            "original": [{"word": "cat", "meaning": "animal", "comment": "base note"}],
            "formatted": [{"word": [0, 0], "comment": "plural note"}]
        },
        "sentences": [{"words": [0, -1], "meaning": "translation", "comment": "sentence note"}]
    })
}

#[test]
fn roundtrip_preserves_derived_tokens_and_annotations() {
    let project = load_project_from_json(project_json()).expect("valid project should load");
    let token = &project.segments[0].tokens[1];
    assert_eq!(token.original, "cats");
    assert_eq!(token.base_word.as_deref(), Some("cat"));
    assert_eq!(token.formation_rule_indices, [0]);
    assert_eq!(project.formatted_word_comments["cats"], "plural note");
    assert_eq!(project.vocabulary_comments["cat"], "base note");
    assert_eq!(project.segments[0].comment, "sentence note");

    let saved = convert_to_saved_project(&project).expect("valid project should save");
    assert_eq!(
        serde_json::to_value(saved).expect("saved project should serialize"),
        project_json()
    );
}

#[test]
fn invalid_sentence_references_return_context_including_minimum_i64() {
    for reference in [1, -2, i64::MIN] {
        let mut value = project_json();
        value["sentences"][0]["words"] = json!([reference]);
        let error = load_project_from_json(value).expect_err("invalid reference must fail");
        assert!(matches!(error, AppError::InvalidProjectFormat(_)));
        assert!(error.to_string().contains("Sentence 0, token 0"));
    }
}

#[test]
fn malformed_derived_chains_are_rejected_even_when_not_used() {
    for chain in [json!([]), json!([0]), json!([1, 0]), json!([0, 1])] {
        let mut value = project_json();
        value["sentences"] = json!([]);
        value["vocabulary"]["formatted"][0]["word"] = chain;
        let error = load_project_from_json(value).expect_err("invalid chain must fail");
        assert!(matches!(error, AppError::InvalidProjectFormat(_)));
        assert!(error.to_string().contains("Formatted word 0"));
    }
}

#[test]
fn failed_formation_scripts_do_not_fall_back_to_the_base_word() {
    for script in [
        "fn transform(word) { let = }",
        "fn transform(word) { throw \"cannot transform\"; }",
    ] {
        let mut value = project_json();
        value["formation"][0]["command"] = json!(script);
        let error = load_project_from_json(value).expect_err("invalid script must fail");
        assert!(matches!(error, AppError::ScriptExecutionError(_)));
        assert!(error.to_string().contains("formation rule 0"));
    }
}

#[test]
fn version_one_reference_cannot_wrap_into_a_derived_reference() {
    let value = json!({
        "version": 1,
        "vocabulary": [],
        "sentences": [{"words": [u64::MAX], "meaning": ""}]
    });
    let error = load_project_from_json(value).expect_err("unsigned overflow must fail");
    assert!(matches!(error, AppError::InvalidProjectFormat(_)));
    assert!(error.to_string().contains("Invalid word index"));
}

#[test]
fn both_sample_projects_reconstruct_with_default_scoped_limits() {
    use std::sync::{Arc, atomic::AtomicBool};
    use std::time::{Duration, Instant};
    use tdector_eval::ExecutionPolicy;

    for (name, source) in [
        ("ginger", include_str!("../../../../sample/ginger.json")),
        ("epigraph", include_str!("../../../../sample/epigraph.json")),
    ] {
        let _guard = ExecutionPolicy::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        )
        .enter();
        let value = serde_json::from_str(source).expect("sample JSON");
        let project =
            load_project_from_json(value).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!project.segments.is_empty(), "{name}");
        assert!(!project.vocabulary.is_empty(), "{name}");
        convert_to_saved_project(&project).expect("sample serialization candidate");
    }
}

#[test]
fn reconstruction_preserves_cancellation_deadline_and_resource_error_types() {
    use std::sync::{Arc, atomic::AtomicBool};
    use std::time::{Duration, Instant};
    use tdector_eval::ExecutionPolicy;

    {
        let _guard = ExecutionPolicy::new(
            Arc::new(AtomicBool::new(true)),
            Instant::now() + Duration::from_secs(30),
        )
        .enter();
        assert!(matches!(
            load_project_from_json(project_json()),
            Err(AppError::OperationCancelled)
        ));
    }
    {
        let _guard = ExecutionPolicy::new(Arc::new(AtomicBool::new(false)), Instant::now()).enter();
        assert!(matches!(
            load_project_from_json(project_json()),
            Err(AppError::DeadlineExceeded)
        ));
    }
    {
        let mut policy = ExecutionPolicy::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        policy.limits.max_operations = 32;
        let _guard = policy.enter();
        let mut value = project_json();
        value["formation"][0]["command"] = json!("fn transform(word) { loop {} word }");
        assert!(matches!(
            load_project_from_json(value),
            Err(AppError::LimitExceeded(_))
        ));
    }
    load_project_from_json(project_json()).expect("previous failure restored ordinary execution");
}
