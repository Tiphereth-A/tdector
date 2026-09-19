use serde_json::{Value, json};
use tdector_app::Session;
use tdector_app::api::{
    ApiError, BatchStage, FormationKind, LookupKind, Mutation, Pagination, Query, RuleSelector,
    SegmentSort, apply_mutation, execute_batch, parse_batch, preview_script, preview_tokenization,
    query, resolve_rule,
};
use tdector_eval::TokenizationRule;

fn imported(text: &str) -> Session {
    let mut session = Session::default();
    session
        .import_text(text, "Example", &TokenizationRule::default_whitespace())
        .expect("import");
    session
}

fn response(session: &mut Session, request: Query) -> Value {
    serde_json::to_value(query(session, request).expect("query")).expect("serialize DTO")
}

fn add_rule(session: &mut Session, description: &str, script: &str) {
    apply_mutation(
        session,
        Mutation::CreateRule {
            description: description.into(),
            rule_type: FormationKind::Inflection,
            script: script.into(),
        },
    )
    .expect("create rule");
}

#[test]
fn paginated_records_keep_original_indices_and_matching_totals() {
    let mut session = imported("cat dog\nbird\ncat");
    let result = response(
        &mut session,
        Query::SegmentList {
            filter: "cat".into(),
            sort: SegmentSort::TokenCount,
            descending: false,
            pagination: Pagination {
                offset: 1,
                limit: Some(1),
            },
        },
    );
    assert_eq!(result["total"], 2);
    assert_eq!(result["offset"], 1);
    assert_eq!(result["limit"], 1);
    assert_eq!(result["items"][0]["segment_index"], 0);
    assert_eq!(result["items"][0]["token_count"], 2);
    let detail = response(&mut session, Query::SegmentShow { segment_index: 0 });
    assert_eq!(detail["segment_index"], 0);
    assert_eq!(detail["tokens"][1]["token_index"], 1);
    assert_eq!(detail["tokens"][1]["text"], "dog");
    let result = response(
        &mut session,
        Query::Lookup {
            word: "missing".into(),
            kind: LookupKind::Usage,
            pagination: Pagination::all(),
        },
    );
    assert_eq!(
        result,
        json!({"items": [], "offset": 0, "limit": null, "total": 0})
    );
}

#[test]
fn lists_use_shared_text_sort_and_filter_excludes_glosses_and_comments() {
    let mut session = imported("a z\naa\nz");
    apply_mutation(
        &mut session,
        Mutation::SetGloss {
            word: "z".into(),
            meaning: "needle".into(),
        },
    )
    .expect("gloss");
    apply_mutation(
        &mut session,
        Mutation::SetComment {
            segment_index: 1,
            token_index: None,
            comment: "needle".into(),
        },
    )
    .expect("comment");
    apply_mutation(
        &mut session,
        Mutation::SetTranslation {
            segment_index: 2,
            translation: "needle".into(),
        },
    )
    .expect("translation");
    let result = response(
        &mut session,
        Query::SegmentList {
            filter: String::new(),
            sort: SegmentSort::Text,
            descending: false,
            pagination: Pagination::all(),
        },
    );
    assert_eq!(
        result["items"][0]["segment_index"], 1,
        "aa sorts before concatenated az"
    );
    let result = response(
        &mut session,
        Query::SegmentList {
            filter: "needle".into(),
            sort: SegmentSort::Index,
            descending: false,
            pagination: Pagination::all(),
        },
    );
    assert_eq!(result["total"], 1);
    assert_eq!(result["items"][0]["segment_index"], 2);
}

#[test]
fn comment_coordinates_persist_without_creating_glosses_or_copying_inheritance() {
    let mut session = imported("cat cats cats");
    let noop = apply_mutation(
        &mut session,
        Mutation::SetComment {
            segment_index: 0,
            token_index: Some(0),
            comment: String::new(),
        },
    )
    .expect("clear unset");
    assert!(!noop.changed);
    apply_mutation(
        &mut session,
        Mutation::SetComment {
            segment_index: 0,
            token_index: Some(0),
            comment: "base note\n".into(),
        },
    )
    .expect("plain token comment");
    assert!(!session.project().vocabulary.contains_key("cat"));
    let snapshot = session.save_snapshot().expect("save");
    session
        .load_json(std::str::from_utf8(&snapshot.bytes).expect("UTF-8"))
        .expect("reload");
    let plain = response(
        &mut session,
        Query::CommentGet {
            segment_index: 0,
            token_index: Some(0),
        },
    );
    assert_eq!(plain["editable_comment"], "base note\n");
    add_rule(
        &mut session,
        "Plural",
        r#"fn transform(word) { word + "s" }"#,
    );
    apply_mutation(
        &mut session,
        Mutation::ApplyFormation {
            word: "cats".into(),
            base: "cat".into(),
            rule: Some("Plural".into()),
            rule_index: None,
        },
    )
    .expect("form");
    let inherited = response(
        &mut session,
        Query::CommentGet {
            segment_index: 0,
            token_index: Some(1),
        },
    );
    assert_eq!(inherited["editable_comment"], "");
    assert_eq!(inherited["display_comment"], "base note\n");
    assert_eq!(
        inherited["target"],
        json!({"kind":"formatted_word","word":"cats"})
    );
    apply_mutation(
        &mut session,
        Mutation::SetComment {
            segment_index: 0,
            token_index: Some(1),
            comment: "derived note".into(),
        },
    )
    .expect("derived comment");
    let snapshot = session.save_snapshot().expect("save");
    session
        .load_json(std::str::from_utf8(&snapshot.bytes).expect("UTF-8"))
        .expect("reload");
    let shared = response(
        &mut session,
        Query::CommentGet {
            segment_index: 0,
            token_index: Some(2),
        },
    );
    assert_eq!(shared["editable_comment"], "derived note");
    assert_eq!(shared["display_comment"], "derived note");
}

#[test]
fn rule_descriptions_are_exact_and_duplicate_indices_disambiguate() {
    let mut session = Session::default();
    add_rule(&mut session, "Same", "fn transform(word) { word }");
    add_rule(&mut session, "Same", "fn transform(word) { word + word }");
    assert!(
        matches!(resolve_rule(&session, &RuleSelector { rule: Some("Same".into()), rule_index: None }),
        Err(ApiError::AmbiguousRule { rule_indices, .. }) if rule_indices == [0, 1])
    );
    assert!(matches!(
        resolve_rule(
            &session,
            &RuleSelector {
                rule: Some("same".into()),
                rule_index: None
            }
        ),
        Err(ApiError::RuleNotFound { .. })
    ));
    assert_eq!(
        resolve_rule(
            &session,
            &RuleSelector {
                rule: None,
                rule_index: Some(1)
            }
        )
        .expect("index"),
        1
    );
    for selector in [
        RuleSelector::default(),
        RuleSelector {
            rule: Some("Same".into()),
            rule_index: Some(0),
        },
    ] {
        assert!(matches!(
            resolve_rule(&session, &selector),
            Err(ApiError::InvalidInput(_))
        ));
    }
}

#[test]
fn rule_receipts_avoid_unstable_ids_and_selectors_resolve_after_sorting() {
    let mut session = Session::default();
    add_rule(&mut session, "Zulu", "fn transform(word) { word }");
    let receipt = apply_mutation(
        &mut session,
        Mutation::CreateRule {
            description: "Alpha".into(),
            rule_type: FormationKind::Derivation,
            script: "fn transform(word) { word }".into(),
        },
    )
    .expect("rule");
    let receipt = serde_json::to_value(receipt).expect("receipt JSON");
    assert_eq!(receipt["description"], "Alpha");
    assert_eq!(receipt["type"], "derivation");
    assert!(receipt.get("rule_index").is_none());
    assert_eq!(
        resolve_rule(
            &session,
            &RuleSelector {
                rule: Some("Alpha".into()),
                rule_index: None
            }
        )
        .expect("before"),
        1
    );
    let snapshot = session.save_snapshot().expect("save");
    session
        .load_json(std::str::from_utf8(&snapshot.bytes).expect("UTF-8"))
        .expect("reload");
    assert_eq!(
        resolve_rule(
            &session,
            &RuleSelector {
                rule: Some("Alpha".into()),
                rule_index: None
            }
        )
        .expect("after"),
        0
    );
}

#[test]
fn batch_decoding_rejects_unknown_fields_ops_versions_with_stage_and_index() {
    for input in [
        r#"{"schema_version":1,"commands":[{"op":"set_gloss","word":"cat","meaning":"animal","typo":1}]}"#,
        r#"{"schema_version":1,"commands":[{"op":"unknown"}]}"#,
        r#"{"schema_version":1,"commands":[{"op":"set_comment","segment_index":-1,"comment":"note"}]}"#,
    ] {
        assert!(matches!(
            parse_batch(input),
            Err(ApiError::Batch {
                stage: BatchStage::Input,
                command_index: Some(0),
                ..
            })
        ));
    }
    for input in [
        "not JSON",
        "\u{feff}{\"schema_version\":1,\"commands\":[]}",
        r#"{"schema_version":2,"commands":[]}"#,
        r#"{"schema_version":1,"commands":[],"unknown":1}"#,
    ] {
        assert!(matches!(
            parse_batch(input),
            Err(ApiError::Batch {
                stage: BatchStage::Input,
                command_index: None,
                ..
            })
        ));
    }
}

#[test]
fn batch_resolves_against_evolving_session_and_reports_command_failure() {
    let mut session = imported("cats cats");
    let request = parse_batch(r#"{
        "schema_version":1,
        "commands":[
          {"op":"set_gloss","word":"cat","meaning":"animal"},
          {"op":"create_rule","description":"Plural","type":"inflection","script":"fn transform(word) { word + \"s\" }"},
          {"op":"apply_formation","word":"cats","base":"cat","rule":"Plural"},
          {"op":"pop_formation","segment_index":0,"token_index":1}
        ]
    }"#).expect("decode");
    let receipt = execute_batch(&mut session, request).expect("batch");
    assert!(receipt.changed);
    assert_eq!(receipt.commands.len(), 4);
    assert!(
        receipt.commands[3]
            .scope
            .as_ref()
            .expect("scope")
            .contains("all occurrences")
    );
    assert!(
        session.project().segments[0]
            .tokens
            .iter()
            .all(|token| token.original == "cat")
    );
    let request = parse_batch(
        r#"{"schema_version":1,"commands":[
        {"op":"set_gloss","word":"cat","meaning":"changed"},
        {"op":"set_translation","segment_index":99,"translation":"bad"}
    ]}"#,
    )
    .expect("decode");
    assert!(matches!(
        execute_batch(&mut session, request),
        Err(ApiError::Batch {
            stage: BatchStage::Command,
            command_index: Some(1),
            ..
        })
    ));
    assert_eq!(
        session.project().vocabulary["cat"],
        "changed",
        "application batch explicitly requires its caller to discard failed private sessions"
    );
}

#[test]
fn previews_pass_quoted_unicode_as_data_and_reject_script_contract_errors() {
    let word = "猫\"\\'";
    assert_eq!(
        preview_script("fn transform(word) { word }", word)
            .expect("identity")
            .result,
        word
    );
    for script in [
        "fn transform() { \"x\" }",
        "fn transform(word) { [word] }",
        "bad script !",
    ] {
        assert!(matches!(
            preview_script(script, word),
            Err(ApiError::Application(tdector_app::Error::Operation(
                tdector_eval::AppError::ScriptExecutionError(_)
            )))
        ));
    }
    let tokenizer = TokenizationRule::default_whitespace();
    assert_eq!(
        preview_tokenization(&tokenizer, "猫 dog")
            .expect("tokens")
            .tokens,
        ["猫", "dog"]
    );
    for line in ["a\nb", "a\rb", "a\r\nb", "a\u{2028}b"] {
        assert!(matches!(
            preview_tokenization(&tokenizer, line),
            Err(ApiError::InvalidInput(_))
        ));
    }
}

#[test]
fn limits_are_validated_before_results_and_exact_gets_reject_missing_objects() {
    let mut session = imported("cat dog");
    for limit in [0, 21, usize::MAX] {
        assert!(matches!(
            query(
                &mut session,
                Query::SimilarTokens {
                    word: "cat".into(),
                    limit
                }
            ),
            Err(ApiError::InvalidInput(_))
        ));
    }
    assert!(
        query(
            &mut session,
            Query::SimilarTokens {
                word: "cat".into(),
                limit: 20
            }
        )
        .is_ok()
    );
    for request in [
        Query::VocabList {
            pagination: Pagination {
                offset: 0,
                limit: Some(0),
            },
        },
        Query::VocabList {
            pagination: Pagination {
                offset: 1,
                limit: None,
            },
        },
        Query::VocabSearch {
            text: "cat".into(),
            limit: 0,
        },
        Query::SimilarSegments {
            segment_index: 0,
            limit: 0,
        },
    ] {
        assert!(matches!(
            query(&mut session, request),
            Err(ApiError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        query(
            &mut session,
            Query::VocabGet {
                word: "absent".into()
            }
        ),
        Err(ApiError::NotFound { .. })
    ));
    assert!(matches!(
        query(&mut session, Query::SegmentShow { segment_index: 9 }),
        Err(ApiError::Application(tdector_app::Error::InvalidIndex {
            kind: "segment",
            index: 9
        }))
    ));
}
