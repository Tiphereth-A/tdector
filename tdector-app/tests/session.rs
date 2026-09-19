use std::sync::Arc;
use tdector_app::{Command, Error, Session};
use tdector_core::enums::{CommentTarget, SortMode};
use tdector_eval::{FormationType, TokenizationRule};

fn imported(content: &str) -> Session {
    let mut session = Session::default();
    session
        .import_text(content, "Example", &TokenizationRule::default_whitespace())
        .expect("import text");
    session
}

fn gloss(session: &mut Session, word: &str, meaning: &str) {
    session
        .execute(Command::SetGloss {
            word: word.into(),
            meaning: meaning.into(),
        })
        .expect("set gloss");
}

fn rule(session: &mut Session, description: &str, command: &str) {
    session
        .execute(Command::CreateFormationRule {
            description: description.into(),
            rule_type: FormationType::Inflection,
            command: command.into(),
        })
        .expect("create rule");
}

fn apply(session: &mut Session, word: &str, base: &str, index: usize) {
    session
        .execute(Command::ApplyFormationRule {
            word: word.into(),
            base_word: base.into(),
            rule: index,
        })
        .expect("apply rule");
}

#[test]
fn mutation_revision_noops_and_sessions_are_independent() {
    let mut first = imported("cat");
    let second = Session::default();
    assert!(first.is_dirty());
    assert!(!second.is_dirty());
    let revision = first.revision();
    assert!(
        !first
            .execute(Command::SetTranslation {
                segment: 0,
                translation: String::new()
            })
            .expect("no-op")
    );
    assert_eq!(first.revision(), revision);
    assert!(
        first
            .execute(Command::SetTranslation {
                segment: 0,
                translation: "a cat".into()
            })
            .expect("edit")
    );
    assert_eq!(first.revision(), revision + 1);
    let saved = first.save_snapshot().expect("snapshot");
    assert!(first.acknowledge_saved(saved.token));
    assert!(!first.is_dirty());
    assert!(
        !first
            .execute(Command::SetTranslation {
                segment: 0,
                translation: "a cat".into()
            })
            .expect("no-op")
    );
    assert!(!first.is_dirty());
    assert_eq!(second.revision(), 0);
}

#[test]
fn failed_import_and_load_preserve_project_revision_dirty_and_lookups() {
    let mut session = imported("cat cat\ndog");
    let before = session.save_snapshot().expect("snapshot");
    session.acknowledge_saved(before.token);
    let maps = session.lookup_maps();
    let revision = session.revision();
    let failing = TokenizationRule {
        description: "Fails on second line".into(),
        command: r#"fn tokenize(line) { if line == "bad" { throw "bad input"; } [line] }"#.into(),
        cached_ast: tdector_eval::default_cached_ast(),
    };
    assert!(
        session
            .import_text("good\nbad", "Replacement", &failing)
            .is_err()
    );
    for json in [
        "not json",
        r#"{"version":999}"#,
        r#"{"version":2,"vocabulary":{"original":[]},"sentences":[{"words":[2],"meaning":""}]}"#,
        r#"{"version":2,"formation":[{"description":"throw","type":"inflection","command":"fn transform(word) { throw \"failure\"; }"}],"vocabulary":{"original":[{"word":"cat","meaning":"animal"}],"formatted":[{"word":[0,0],"comment":""}]},"sentences":[{"words":[-1],"meaning":""}]}"#,
    ] {
        assert!(session.load_json(json).is_err());
    }
    assert_eq!(
        session.save_snapshot().expect("snapshot").bytes,
        before.bytes
    );
    assert_eq!(session.lookup_maps(), maps);
    assert_eq!(session.revision(), revision);
    assert!(!session.is_dirty());
}

#[test]
fn failed_formation_and_invalid_indices_do_not_mutate() {
    let mut session = imported("cat cats");
    gloss(&mut session, "cat", "animal");
    rule(
        &mut session,
        "Throw",
        "fn transform(word) { throw \"failure\"; }",
    );
    let before = session.save_snapshot().expect("snapshot");
    let revision = session.revision();
    assert!(
        session
            .execute(Command::ApplyFormationRule {
                word: "cats".into(),
                base_word: "cat".into(),
                rule: 0
            })
            .is_err()
    );
    for command in [
        Command::ApplyFormationRule {
            word: "cats".into(),
            base_word: "cat".into(),
            rule: 99,
        },
        Command::SetTranslation {
            segment: 99,
            translation: "wrong".into(),
        },
        Command::SetSegmentComment {
            segment: 99,
            comment: "wrong".into(),
        },
        Command::RemoveFormationRule {
            segment: 0,
            token: 99,
        },
    ] {
        assert!(matches!(
            session.execute(command),
            Err(Error::InvalidIndex { .. })
        ));
    }
    assert!(session.similar_segments(99, 10).is_err());
    assert!(session.formation_chain(99, 0).is_err());
    assert_eq!(
        session.save_snapshot().expect("snapshot").bytes,
        before.bytes
    );
    assert_eq!(session.revision(), revision);
}

#[test]
fn two_step_formation_removes_only_last_step_and_updates_matching_occurrences() {
    let mut session = imported("cats catsx catsx\ncat");
    gloss(&mut session, "cat", "animal");
    gloss(&mut session, "cats", "animals");
    gloss(&mut session, "catsx", "marked animals");
    rule(
        &mut session,
        "Plural",
        r#"fn transform(word) { word + "s" }"#,
    );
    rule(
        &mut session,
        "Marker",
        r#"fn transform(word) { word + "x" }"#,
    );
    apply(&mut session, "cats", "cat", 0);
    apply(&mut session, "catsx", "cats", 1);
    session
        .execute(Command::SetWordComment {
            target: CommentTarget::FormattedWord("catsx".into()),
            comment: "keep note".into(),
        })
        .expect("comment");
    let chain = session.formation_chain(0, 1).expect("chain");
    assert_eq!(
        chain
            .iter()
            .map(|step| step.result.as_str())
            .collect::<Vec<_>>(),
        ["cats", "catsx"]
    );
    let maps = session.lookup_maps().1.expect("usage map");
    assert_eq!(maps.get("catsx"), Some(&vec![0]));
    session
        .execute(Command::RemoveFormationRule {
            segment: 0,
            token: 1,
        })
        .expect("remove last step");
    for token in &session.project().segments[0].tokens {
        assert_eq!(token.original, "cats");
        assert_eq!(token.base_word.as_deref(), Some("cat"));
        assert_eq!(token.formation_rule_indices, [0]);
    }
    assert_eq!(
        session
            .project()
            .formatted_word_comments
            .get("cats")
            .map(String::as_str),
        Some("keep note")
    );
    let maps = session.lookup_maps().1.expect("usage map");
    assert!(!maps.contains_key("catsx"));
    assert_eq!(maps.get("cats"), Some(&vec![0]));
    assert!(
        session
            .filtered_indices("catsx", SortMode::DEFAULT)
            .is_empty()
    );
    session
        .execute(Command::RemoveFormationRule {
            segment: 0,
            token: 0,
        })
        .expect("remove final step");
    assert!(
        session.project().segments[0]
            .tokens
            .iter()
            .all(|token| token.original == "cat" && token.formation_rule_indices.is_empty())
    );
    assert_eq!(
        session
            .project()
            .vocabulary_comments
            .get("cat")
            .map(String::as_str),
        Some("keep note")
    );
    assert_eq!(
        session.project().vocabulary.get("cat").map(String::as_str),
        Some("animal")
    );
    assert!(session.save_snapshot().is_ok());
}

#[test]
fn mismatched_formation_is_rejected_and_reapplying_same_rule_is_a_noop() {
    let mut session = imported("cats dog");
    gloss(&mut session, "cat", "animal");
    rule(
        &mut session,
        "Plural",
        r#"fn transform(word) { word + "s" }"#,
    );
    let revision = session.revision();
    assert!(
        session
            .execute(Command::ApplyFormationRule {
                word: "dog".into(),
                base_word: "cat".into(),
                rule: 0
            })
            .is_err()
    );
    assert_eq!(session.revision(), revision);
    apply(&mut session, "cats", "cat", 0);
    let revision = session.revision();
    assert!(
        !session
            .execute(Command::ApplyFormationRule {
                word: "cats".into(),
                base_word: "cat".into(),
                rule: 0
            })
            .expect("repeat")
    );
    assert_eq!(session.revision(), revision);
}

#[test]
fn save_acknowledgment_rejects_edits_replacements_and_other_sessions() {
    let mut session = imported("cat");
    let mut other = imported("cat");
    let first = session.save_snapshot().expect("snapshot");
    assert!(!other.acknowledge_saved(first.token));
    gloss(&mut session, "cat", "animal");
    assert!(!session.acknowledge_saved(first.token));
    assert!(session.is_dirty());
    let edited = session.save_snapshot().expect("snapshot");
    session
        .import_text(
            "dog",
            "Replacement",
            &TokenizationRule::default_whitespace(),
        )
        .expect("replacement");
    assert!(!session.acknowledge_saved(edited.token));
    let replacement = session.save_snapshot().expect("snapshot");
    session
        .load_json(std::str::from_utf8(&replacement.bytes).expect("json"))
        .expect("load");
    assert!(!session.acknowledge_saved(replacement.token));
    assert!(!session.is_dirty());
    gloss(&mut session, "dog", "canine");
    assert!(!session.acknowledge_saved(replacement.token));
    let current = session.save_snapshot().expect("snapshot");
    assert!(session.acknowledge_saved(current.token));
    assert!(!session.is_dirty());
}

#[test]
fn queries_run_without_rendering_and_rebuild_after_replacement() {
    let mut session = imported("cat dog\ncat dog\nbird fly");
    let related = session.similar_segments(0, 10).expect("similar sentences");
    assert!(related.iter().any(|(index, _)| *index == 1));
    assert_eq!(session.filtered_indices("dog", SortMode::DEFAULT), [0, 1]);
    assert!(!session.similar_tokens("cat").is_empty());
    session
        .import_text(
            "bird fly\ncat dog\nbird fly",
            "Replacement",
            &TokenizationRule::default_whitespace(),
        )
        .expect("replace");
    let related = session
        .similar_segments(0, 10)
        .expect("recomputed similarity");
    assert!(related.iter().any(|(index, _)| *index == 2));
    assert!(!related.iter().any(|(index, _)| *index == 1));
    assert_eq!(
        session.lookup_maps().1.expect("usages").get("cat"),
        Some(&vec![1])
    );
    session
        .execute(Command::SetTranslation {
            segment: 2,
            translation: "avian".into(),
        })
        .expect("translation");
    assert_eq!(session.filtered_indices("avian", SortMode::DEFAULT), [2]);
}

#[test]
fn persistence_and_export_are_available_without_a_ui() {
    let mut session = imported("cat cats");
    gloss(&mut session, "cat", "animal");
    rule(
        &mut session,
        "Plural",
        r#"fn transform(word) { word + "s" }"#,
    );
    apply(&mut session, "cats", "cat", 0);
    session
        .execute(Command::SetTranslation {
            segment: 0,
            translation: "a cat and cats".into(),
        })
        .expect("translation");
    session
        .execute(Command::SetSegmentComment {
            segment: 0,
            comment: "sentence note".into(),
        })
        .expect("comment");
    let snapshot = session.save_snapshot().expect("snapshot");
    let mut restored = Session::default();
    restored
        .load_json(std::str::from_utf8(&snapshot.bytes).expect("JSON"))
        .expect("load");
    assert!(!restored.is_dirty());
    assert_eq!(
        restored.save_snapshot().expect("snapshot").bytes,
        snapshot.bytes
    );
    assert_eq!(
        restored.formation_chain(0, 1).expect("chain")[0].result,
        "cats"
    );
    assert_eq!(restored.project().segments[0].comment, "sentence note");
    assert!(restored.export_typst().contains("a cat and cats"));
}

#[test]
fn related_words_are_ordered_before_limit_is_applied() {
    let mut session = Session::default();
    for word in ["bobcat", "catfish", "cat", "catamaran", "muscat"] {
        gloss(&mut session, word, "meaning");
    }
    assert_eq!(session.related_words("CAT", 2), ["cat", "catamaran"]);
    assert!(session.related_words("", 2).is_empty());
}

#[test]
fn creating_invalid_or_duplicate_rules_is_atomic() {
    let mut session = Session::default();
    for command in ["invalid ! script", "fn unrelated(word) { word }"] {
        assert!(
            session
                .execute(Command::CreateFormationRule {
                    description: "Invalid".into(),
                    rule_type: FormationType::Derivation,
                    command: command.into()
                })
                .is_err()
        );
    }
    assert_eq!(session.revision(), 0);
    rule(&mut session, "Identity", "fn transform(word) { word }");
    let revision = session.revision();
    assert!(
        !session
            .execute(Command::CreateFormationRule {
                description: "Identity".into(),
                rule_type: FormationType::Inflection,
                command: "fn transform(word) { word }".into()
            })
            .expect("duplicate")
    );
    assert_eq!(session.revision(), revision);
}

#[test]
fn text_revision_ignores_metadata_and_tracks_text_changes() {
    let mut session = imported("cat cats");
    let imported_revision = session.text_revision();
    gloss(&mut session, "cat", "animal");
    session
        .execute(Command::SetTranslation {
            segment: 0,
            translation: "translated".into(),
        })
        .expect("translation");
    session
        .execute(Command::SetSegmentComment {
            segment: 0,
            comment: "note".into(),
        })
        .expect("comment");
    session
        .execute(Command::SetWordComment {
            target: CommentTarget::BaseWord("cat".into()),
            comment: "base note".into(),
        })
        .expect("word comment");
    rule(
        &mut session,
        "Plural",
        r#"fn transform(word) { word + "s" }"#,
    );
    apply(&mut session, "cats", "cat", 0);
    assert_eq!(session.text_revision(), imported_revision);
    session
        .execute(Command::RemoveFormationRule {
            segment: 0,
            token: 1,
        })
        .expect("remove");
    assert_eq!(session.text_revision(), imported_revision + 1);
    let snapshot = session.save_snapshot().expect("snapshot");
    session
        .load_json(std::str::from_utf8(&snapshot.bytes).expect("json"))
        .expect("load");
    assert_eq!(session.text_revision(), imported_revision + 2);
    session
        .import_text(
            "dog",
            "replacement",
            &TokenizationRule::default_whitespace(),
        )
        .expect("import");
    assert_eq!(session.text_revision(), imported_revision + 3);
}

#[test]
fn lookup_snapshots_share_unchanged_indices_and_preserve_prior_results() {
    let mut session = imported("cat cat\ndog");
    let (first_heads, first_usages) = session.lookup_maps();
    let first_heads = first_heads.expect("headword map");
    let first_usages = first_usages.expect("usage map");
    gloss(&mut session, "cat", "animal");
    let (second_heads, second_usages) = session.lookup_maps();
    assert!(Arc::ptr_eq(
        &first_heads,
        &second_heads.expect("headword map")
    ));
    assert!(Arc::ptr_eq(
        &first_usages,
        &second_usages.expect("usage map")
    ));
    session
        .import_text(
            "bird",
            "replacement",
            &TokenizationRule::default_whitespace(),
        )
        .expect("import");
    let (_, new_usages) = session.lookup_maps();
    let new_usages = new_usages.expect("new usage map");
    assert!(!Arc::ptr_eq(&first_usages, &new_usages));
    assert_eq!(first_usages.get("cat"), Some(&vec![0]));
    assert!(!new_usages.contains_key("cat"));
    assert_eq!(new_usages.get("bird"), Some(&vec![0]));
}

#[test]
fn token_annotations_share_gloss_chain_and_inherited_comment_resolution() {
    let mut session = imported("cat cats catsx");
    gloss(&mut session, "cat", "animal");
    session
        .execute(Command::SetWordComment {
            target: CommentTarget::BaseWord("cat".into()),
            comment: "base note".into(),
        })
        .expect("base comment");
    rule(
        &mut session,
        "Plural",
        r#"fn transform(word) { word + "s" }"#,
    );
    rule(
        &mut session,
        "Marker",
        r#"fn transform(word) { word + "x" }"#,
    );
    apply(&mut session, "cats", "cat", 0);
    apply(&mut session, "catsx", "cats", 1);

    let base =
        tdector_app::annotate_token(session.project(), &session.project().segments[0].tokens[0]);
    assert_eq!(base.base_word, "cat");
    assert_eq!(base.base_gloss, "animal");
    assert!(!base.is_derived);
    assert!(base.formation_descriptions.is_empty());
    assert_eq!(base.display_comment, "base note");
    assert_eq!(base.editable_comment, "base note");
    assert!(matches!(base.comment_target, CommentTarget::BaseWord(word) if word == "cat"));

    let derived =
        tdector_app::annotate_token(session.project(), &session.project().segments[0].tokens[2]);
    assert_eq!(derived.base_word, "cat");
    assert_eq!(derived.base_gloss, "animal");
    assert!(derived.is_derived);
    assert_eq!(derived.formation_descriptions, ["Plural", "Marker"]);
    assert_eq!(derived.display_comment, "base note");
    assert!(derived.editable_comment.is_empty());
    let (target, text) = session.token_comment(0, 2).expect("derived comment editor");
    assert!(matches!(target, CommentTarget::FormattedWord(word) if word == "catsx"));
    assert!(
        text.is_empty(),
        "inherited text must not become a formatted comment"
    );

    session
        .execute(Command::SetWordComment {
            target: CommentTarget::FormattedWord("catsx".into()),
            comment: "derived note".into(),
        })
        .expect("derived comment");
    let derived =
        tdector_app::annotate_token(session.project(), &session.project().segments[0].tokens[2]);
    assert_eq!(derived.display_comment, "derived note");
    assert_eq!(
        session.token_comment(0, 2).expect("editor").1,
        "derived note"
    );
    session
        .execute(Command::SetWordComment {
            target: CommentTarget::FormattedWord("catsx".into()),
            comment: String::new(),
        })
        .expect("clear derived comment");
    assert_eq!(
        tdector_app::annotate_token(session.project(), &session.project().segments[0].tokens[2])
            .display_comment,
        "base note",
    );
    assert_eq!(
        session.token_comment(0, 0).expect("base editor").1,
        "base note"
    );
}

#[test]
fn unannotated_tokens_have_empty_metadata_and_invalid_comment_coordinates_fail() {
    let session = imported("unknown");
    let annotation =
        tdector_app::annotate_token(session.project(), &session.project().segments[0].tokens[0]);
    assert_eq!(annotation.base_word, "unknown");
    assert!(annotation.base_gloss.is_empty());
    assert!(annotation.display_comment.is_empty());
    assert!(annotation.editable_comment.is_empty());
    assert!(annotation.formation_descriptions.is_empty());
    assert!(!annotation.is_derived);
    assert!(matches!(
        session.token_comment(1, 0),
        Err(Error::InvalidIndex {
            kind: "segment",
            ..
        })
    ));
    assert!(matches!(
        session.token_comment(0, 1),
        Err(Error::InvalidIndex { kind: "token", .. })
    ));
}
