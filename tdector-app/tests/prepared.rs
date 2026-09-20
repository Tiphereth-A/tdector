use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use tdector_app::api::{self, ApiError, BatchRequest, BatchStage, Mutation};
use tdector_app::{Command, Session};
use tdector_core::enums::CommentTarget;
use tdector_eval::{FormationType, TokenizationRule};

fn batch(commands: Vec<Mutation>) -> BatchRequest {
    BatchRequest {
        schema_version: 1,
        commands,
    }
}

fn translation(value: &str) -> Mutation {
    Mutation::SetTranslation {
        segment_index: 0,
        translation: value.into(),
    }
}

fn fixture() -> Session {
    let mut session = Session::default();
    session
        .import_text(
            "cats cats dog\ncat dog",
            "Prepared operations",
            &TokenizationRule::default_whitespace(),
        )
        .expect("import");
    for (word, meaning) in [("cat", "animal"), ("cats", "animals"), ("dog", "animal")] {
        session
            .execute(Command::SetGloss {
                word: word.into(),
                meaning: meaning.into(),
            })
            .expect("gloss");
    }
    // Runtime order deliberately differs from the saved format's sorted order.
    for (description, command) in [
        ("Z plural", r#"fn transform(word) { word + "s" }"#),
        ("A identity", "fn transform(word) { word }"),
    ] {
        session
            .execute(Command::CreateFormationRule {
                description: description.into(),
                rule_type: FormationType::Inflection,
                command: command.into(),
            })
            .expect("rule");
    }
    session
        .execute(Command::ApplyFormationRule {
            word: "cats".into(),
            base_word: "cat".into(),
            rule: 0,
        })
        .expect("formation");
    session
        .execute(Command::SetWordComment {
            target: CommentTarget::BaseWord("cat".into()),
            comment: "inherited base note".into(),
        })
        .expect("base comment");
    let token = session.save_snapshot().expect("save").token;
    assert!(session.acknowledge_saved(token));
    session
}

#[test]
fn failed_batch_preserves_content_counters_rules_caches_and_save_authority() {
    let mut session = fixture();
    let snapshot = session.save_snapshot().expect("snapshot");
    let revision = session.revision();
    let text_revision = session.text_revision();
    let maps = session.lookup_maps();
    let similarity = session.similar_segments(0, 10).expect("similarity");
    let ast = session.project().formation_rules[0].cached_ast.clone();
    let error = api::prepare_batch(
        &session,
        batch(vec![
            translation("staged"),
            Mutation::SetComment {
                segment_index: 0,
                token_index: Some(0),
                comment: "shared staged note".into(),
            },
            Mutation::SetTranslation {
                segment_index: 99,
                translation: "invalid".into(),
            },
        ]),
    )
    .expect_err("invalid later command");
    assert!(matches!(
        error,
        ApiError::Batch {
            stage: BatchStage::Command,
            command_index: Some(2),
            ..
        }
    ));
    assert_eq!(
        session.save_snapshot().expect("snapshot").bytes,
        snapshot.bytes
    );
    assert_eq!(session.revision(), revision);
    assert_eq!(session.text_revision(), text_revision);
    assert!(!session.is_dirty());
    assert_eq!(session.project().formation_rules[0].description, "Z plural");
    assert_eq!(
        session.project().segments[0].tokens[0].formation_rule_indices,
        [0]
    );
    assert!(Rc::ptr_eq(
        &ast,
        &session.project().formation_rules[0].cached_ast
    ));
    let after = session.lookup_maps();
    assert!(Arc::ptr_eq(
        maps.0.as_ref().expect("headwords"),
        after.0.as_ref().expect("headwords")
    ));
    assert!(Arc::ptr_eq(
        maps.1.as_ref().expect("usages"),
        after.1.as_ref().expect("usages")
    ));
    assert_eq!(
        session.similar_segments(0, 10).expect("similarity"),
        similarity
    );
    assert!(session.acknowledge_saved(snapshot.token));
}

#[test]
fn preview_is_discardable_and_commit_advances_once_with_shared_comment_semantics() {
    let mut session = fixture();
    let revision = session.revision();
    let text_revision = session.text_revision();
    let snapshot = session.save_snapshot().expect("snapshot");
    let maps = session.lookup_maps();
    let request = batch(vec![
        translation("The cats."),
        Mutation::SetComment {
            segment_index: 0,
            token_index: Some(0),
            comment: "shared override".into(),
        },
        Mutation::SetComment {
            segment_index: 0,
            token_index: Some(1),
            comment: "shared override".into(),
        },
    ]);
    let preview = api::prepare_batch(&session, request.clone()).expect("preview");
    assert!(preview.receipt().changed);
    assert_eq!(
        preview
            .receipt()
            .commands
            .iter()
            .map(|r| r.changed)
            .collect::<Vec<_>>(),
        [true, true, false]
    );
    assert_eq!(preview.projected_revision(), revision + 1);
    assert!(preview.projected_dirty());
    drop(preview);
    assert_eq!(session.revision(), revision);
    assert!(!session.is_dirty());
    assert_eq!(
        session.save_snapshot().expect("snapshot").bytes,
        snapshot.bytes
    );
    assert!(session.acknowledge_saved(snapshot.token));

    let prepared = api::prepare_batch(&session, request).expect("prepare");
    let receipt = api::commit_batch(&mut session, prepared).expect("commit");
    assert!(receipt.changed);
    assert_eq!(session.revision(), revision + 1);
    assert_eq!(session.text_revision(), text_revision);
    assert!(session.is_dirty());
    assert!(!session.acknowledge_saved(snapshot.token));
    assert_eq!(session.project().formation_rules[0].description, "Z plural");
    assert_eq!(
        session.project().segments[0].tokens[0].formation_rule_indices,
        [0]
    );
    let after = session.lookup_maps();
    assert!(Arc::ptr_eq(
        maps.1.as_ref().expect("usages"),
        after.1.as_ref().expect("usages")
    ));
    for token in &session.project().segments[0].tokens[..2] {
        let annotation = tdector_app::annotate_token(session.project(), token);
        assert_eq!(annotation.editable_comment, "shared override");
        assert_eq!(annotation.display_comment, "shared override");
    }

    let clear = api::prepare_batch(
        &session,
        batch(vec![Mutation::SetComment {
            segment_index: 0,
            token_index: Some(1),
            comment: String::new(),
        }]),
    )
    .expect("clear preview");
    api::commit_batch(&mut session, clear).expect("clear commit");
    for token in &session.project().segments[0].tokens[..2] {
        let annotation = tdector_app::annotate_token(session.project(), token);
        assert_eq!(annotation.editable_comment, "");
        assert_eq!(annotation.display_comment, "inherited base note");
    }
}

#[test]
fn noop_preserves_clean_or_dirty_state_but_change_then_revert_counts_as_changed() {
    let mut session = fixture();
    for dirty in [false, true] {
        if dirty {
            session
                .execute(Command::SetSegmentComment {
                    segment: 0,
                    comment: "note".into(),
                })
                .expect("edit");
        }
        let revision = session.revision();
        let prepared = api::prepare_batch(&session, batch(vec![translation("")])).expect("noop");
        assert!(!prepared.receipt().changed);
        assert_eq!(prepared.projected_revision(), revision);
        assert_eq!(prepared.projected_dirty(), dirty);
        api::commit_batch(&mut session, prepared).expect("noop commit");
        assert_eq!(session.revision(), revision);
        assert_eq!(session.is_dirty(), dirty);
    }
    let snapshot = session.save_snapshot().expect("snapshot");
    assert!(session.acknowledge_saved(snapshot.token));
    let revision = session.revision();
    let prepared = api::prepare_batch(
        &session,
        batch(vec![translation("temporary"), translation("")]),
    )
    .expect("prepare");
    assert!(prepared.receipt().changed);
    api::commit_batch(&mut session, prepared).expect("commit");
    assert_eq!(
        session.save_snapshot().expect("snapshot").bytes,
        snapshot.bytes
    );
    assert_eq!(session.revision(), revision + 1);
    assert!(session.is_dirty());
    assert!(!session.acknowledge_saved(snapshot.token));
}

#[test]
fn candidates_reject_other_sessions_intervening_mutations_and_save_acknowledgments() {
    let mut first = fixture();
    let mut other = fixture();
    let candidate =
        api::prepare_batch(&first, batch(vec![translation("foreign")])).expect("prepare");
    assert!(matches!(
        api::commit_batch(&mut other, candidate),
        Err(ApiError::Batch {
            stage: BatchStage::Commit,
            ..
        })
    ));
    assert_eq!(other.project().segments[0].translation, "");

    let candidate = api::prepare_batch(&first, batch(vec![translation("stale")])).expect("prepare");
    first
        .execute(Command::SetSegmentComment {
            segment: 0,
            comment: "intervening".into(),
        })
        .expect("edit");
    assert!(api::commit_batch(&mut first, candidate).is_err());
    assert_eq!(first.project().segments[0].translation, "");
    let candidate =
        api::prepare_batch(&first, batch(vec![translation("stale dirty state")])).expect("prepare");
    let token = first.save_snapshot().expect("snapshot").token;
    assert!(first.acknowledge_saved(token));
    assert!(api::commit_batch(&mut first, candidate).is_err());
    assert!(!first.is_dirty());
}

#[test]
fn prepared_load_preserves_live_state_until_commit_and_invalidates_old_authority() {
    let mut session = fixture();
    let snapshot = session.save_snapshot().expect("snapshot");
    let old_revision = session.revision();
    let old_text_revision = session.text_revision();
    let maps = session.lookup_maps();
    let json = String::from_utf8(snapshot.bytes.clone()).expect("UTF-8");
    let preview = session.prepare_load_json(&json).expect("prepare");
    assert_eq!(preview.projected_revision(), old_revision + 1);
    assert!(!preview.projected_dirty());
    drop(preview);
    assert_eq!(session.revision(), old_revision);
    assert!(session.acknowledge_saved(snapshot.token));
    let after = session.lookup_maps();
    assert!(Arc::ptr_eq(
        maps.1.as_ref().expect("usages"),
        after.1.as_ref().expect("usages")
    ));

    let candidate = session.prepare_load_json(&json).expect("prepare");
    session.commit_load(candidate).expect("commit");
    assert_eq!(session.revision(), old_revision + 1);
    assert_eq!(session.text_revision(), old_text_revision + 1);
    assert!(!session.is_dirty());
    assert!(!session.acknowledge_saved(snapshot.token));
    let after = session.lookup_maps();
    assert!(!Arc::ptr_eq(
        maps.1.as_ref().expect("usages"),
        after.1.as_ref().expect("usages")
    ));
    assert_eq!(
        session.project().formation_rules[0].description,
        "A identity"
    );
    let current = session.save_snapshot().expect("snapshot");
    assert_eq!(current.bytes, snapshot.bytes);
    assert!(session.acknowledge_saved(current.token));

    let mut other = fixture();
    let foreign = session.prepare_load_json(&json).expect("prepare");
    assert!(other.commit_load(foreign).is_err());
    let stale = session.prepare_load_json(&json).expect("prepare");
    session
        .execute(Command::SetSegmentComment {
            segment: 0,
            comment: "retained".into(),
        })
        .expect("edit");
    assert!(session.commit_load(stale).is_err());
    assert_eq!(session.project().segments[0].comment, "retained");
    assert!(session.is_dirty());
}

#[test]
fn staged_script_execution_does_not_populate_the_live_ast_cache() {
    let json = r#"{"version":2,"formation":[{"description":"Plural","type":"inflection","command":"fn transform(word) { word + \"s\" }"}],"vocabulary":{"original":[{"word":"cat","meaning":"animal"},{"word":"cats","meaning":"animals"}]},"sentences":[{"words":[1],"meaning":""}]}"#;
    let mut session = Session::default();
    session.load_json(json).expect("load");
    assert!(
        session.project().formation_rules[0]
            .cached_ast
            .get()
            .is_none()
    );
    let candidate = api::prepare_batch(
        &session,
        batch(vec![Mutation::ApplyFormation {
            word: "cats".into(),
            base: "cat".into(),
            rule: None,
            rule_index: Some(0),
        }]),
    )
    .expect("prepare formation");
    assert!(candidate.receipt().changed);
    assert!(
        session.project().formation_rules[0]
            .cached_ast
            .get()
            .is_none()
    );
    drop(candidate);
    assert!(
        session.project().segments[0].tokens[0]
            .formation_rule_indices
            .is_empty()
    );
}

#[test]
fn committed_text_mutation_invalidates_text_caches_once() {
    let mut session = fixture();
    let old_text_revision = session.text_revision();
    let maps = session.lookup_maps();
    let prepared = api::prepare_batch(
        &session,
        batch(vec![
            Mutation::PopFormation {
                segment_index: 0,
                token_index: 0,
            },
            translation("A cat."),
        ]),
    )
    .expect("prepare");
    api::commit_batch(&mut session, prepared).expect("commit");
    assert_eq!(session.text_revision(), old_text_revision + 1);
    let usages = session.lookup_maps().1.expect("usages");
    assert!(!Arc::ptr_eq(maps.1.as_ref().expect("usages"), &usages));
    assert!(!usages.contains_key("cats"));
    assert_eq!(usages.get("cat"), Some(&vec![0, 1]));
}

#[test]
fn cancellation_and_deadlines_before_installation_preserve_live_state() {
    let mut session = fixture();
    let snapshot = session.save_snapshot().expect("snapshot");
    let revision = session.revision();
    let json = String::from_utf8(snapshot.bytes.clone()).expect("UTF-8");
    let batch_candidate =
        api::prepare_batch(&session, batch(vec![translation("cancelled")])).expect("prepare batch");
    let load_candidate = session.prepare_load_json(&json).expect("prepare load");
    {
        let _policy = tdector_eval::ExecutionPolicy::new(
            Arc::new(AtomicBool::new(true)),
            Instant::now() + Duration::from_secs(30),
        )
        .enter();
        assert!(matches!(
            api::commit_batch(&mut session, batch_candidate),
            Err(ApiError::Batch {
                stage: BatchStage::Commit,
                ..
            })
        ));
        assert!(matches!(
            session.commit_load(load_candidate),
            Err(tdector_app::Error::Operation(
                tdector_eval::AppError::OperationCancelled
            ))
        ));
        assert!(api::prepare_batch(&session, batch(vec![translation("cancelled")])).is_err());
        assert!(session.prepare_load_json(&json).is_err());
    }
    let batch_candidate =
        api::prepare_batch(&session, batch(vec![translation("too late")])).expect("prepare batch");
    let load_candidate = session.prepare_load_json(&json).expect("prepare load");
    {
        let _policy =
            tdector_eval::ExecutionPolicy::new(Arc::new(AtomicBool::new(false)), Instant::now())
                .enter();
        assert!(api::commit_batch(&mut session, batch_candidate).is_err());
        assert!(matches!(
            session.commit_load(load_candidate),
            Err(tdector_app::Error::Operation(
                tdector_eval::AppError::DeadlineExceeded
            ))
        ));
    }
    assert_eq!(session.revision(), revision);
    assert!(!session.is_dirty());
    assert_eq!(
        session.save_snapshot().expect("snapshot").bytes,
        snapshot.bytes
    );
    assert!(session.acknowledge_saved(snapshot.token));
}

#[cfg(feature = "schema")]
#[test]
fn reusable_schema_derives_preserve_existing_serialized_shapes() {
    let schema = serde_json::to_value(schemars::schema_for!(api::SegmentDetail)).expect("schema");
    assert!(schema["properties"]["segment_index"].is_object());
    assert!(schema["properties"]["tokens"].is_object());
    let schema = serde_json::to_value(schemars::schema_for!(api::BatchRequest)).expect("schema");
    assert_eq!(schema["additionalProperties"], false);
    let request = batch(vec![translation("value")]);
    assert_eq!(
        serde_json::to_value(request).expect("serialize"),
        serde_json::json!({
            "schema_version": 1,
            "commands": [{"op": "set_translation", "segment_index": 0, "translation": "value"}]
        })
    );
    let _ = schemars::schema_for!(api::Page<api::SegmentRecord>);
    let _ = schemars::schema_for!(api::QueryResponse);
}
