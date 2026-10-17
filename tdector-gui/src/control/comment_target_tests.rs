//! Comment dialogs must distinguish base and derived ownership even for equal text.
use super::*;
use tdector_core::enums::CommentTarget;

fn same_spelling_comments() -> (DecryptionApp, Context) {
    let (mut app, ctx, _) = app();
    assert!(app.import_text(
        "cat cats\ncat cats",
        "Same-spelling comments",
        &TokenizationRule::default_whitespace(),
        &ctx,
    ));
    // Apply identity to both original cats, then derive and remove a plural on
    // the other tokens. This creates a valid mixed project through production
    // commands: token 0 is identity-derived cat, token 1 is original cat.
    for command in [
        Command::CreateFormationRule {
            description: "Identity".into(),
            rule_type: tdector_eval::FormationType::Derivation,
            command: "fn transform(word) { word }".into(),
        },
        Command::ApplyFormationRule {
            word: "cat".into(),
            base_word: "cat".into(),
            rule: 0,
        },
        Command::CreateFormationRule {
            description: "Plural".into(),
            rule_type: tdector_eval::FormationType::Inflection,
            command: "fn transform(word) { word + \"s\" }".into(),
        },
        Command::ApplyFormationRule {
            word: "cats".into(),
            base_word: "cat".into(),
            rule: 1,
        },
        Command::RemoveFormationRule {
            segment: 0,
            token: 1,
        },
        Command::SetWordComment {
            target: CommentTarget::BaseWord("cat".into()),
            comment: "base comment".into(),
        },
        Command::SetWordComment {
            target: CommentTarget::FormattedWord("cat".into()),
            comment: "derived comment".into(),
        },
    ] {
        assert!(app.apply_command(command, &ctx));
    }
    for segment in 0..2 {
        let tokens = &app.session.project().segments[segment].tokens;
        assert_eq!(tokens[0].original, "cat");
        assert_eq!(tokens[0].formation_rule_indices, vec![0]);
        assert_eq!(tokens[1].original, "cat");
        assert!(tokens[1].formation_rule_indices.is_empty());
        assert!(matches!(
            app.session.token_comment(segment, 0).expect("derived target").0,
            CommentTarget::FormattedWord(word) if word == "cat"
        ));
        assert!(matches!(
            app.session.token_comment(segment, 1).expect("base target").0,
            CommentTarget::BaseWord(word) if word == "cat"
        ));
    }
    let saved = app.session.save_snapshot().expect("valid project snapshot");
    app.session
        .prepare_load_json(std::str::from_utf8(&saved.bytes).expect("UTF-8 project"))
        .expect("mixed same-spelling fixture must pass the production loader");
    assert!(app.session.acknowledge_saved(saved.token));
    app.sync_control_windows();
    (app, ctx)
}

fn comment_value(app: &mut DecryptionApp, ctx: &Context, handle: &str) -> Value {
    execute(
        app,
        ctx,
        "get",
        json!({"handle":handle,"control":"comment"}),
    )["value"]
        .clone()
}

#[test]
fn base_and_identity_derived_comments_do_not_reuse_or_save_each_others_target() {
    let (mut app, ctx) = same_spelling_comments();
    let base = open(
        &mut app,
        &ctx,
        json!({"kind":"word-comment","segment":0,"token":1}),
    );
    assert_eq!(comment_value(&mut app, &ctx, &base), "base comment");
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":base,"control":"comment","value":"edited base"}),
    );

    let conflict = app
        .control_execute(
            &ctx,
            "open",
            json!({"kind":"word-comment","segment":0,"token":0}),
        )
        .expect_err(
            "same spelling with different comment ownership occupies another singleton target",
        );
    assert_eq!(conflict.code, "already_open");
    assert_eq!(conflict.details.expect("occupied handle")["handle"], base);
    assert_eq!(comment_value(&mut app, &ctx, &base), "edited base");
    assert!(!app.session.is_dirty());
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":base,"action":"save"}),
    );
    assert_eq!(
        app.session.project().vocabulary_comments["cat"],
        "edited base"
    );
    assert_eq!(
        app.session.project().formatted_word_comments["cat"],
        "derived comment"
    );

    let derived = open(
        &mut app,
        &ctx,
        json!({"kind":"word-comment","segment":0,"token":0}),
    );
    assert_ne!(base, derived);
    assert_eq!(comment_value(&mut app, &ctx, &derived), "derived comment");
    let conflict = app
        .control_execute(
            &ctx,
            "open",
            json!({"kind":"word-comment","segment":1,"token":1}),
        )
        .expect_err("derived singleton must also reject a base target with the same spelling");
    assert_eq!(conflict.code, "already_open");
    assert_eq!(
        conflict.details.expect("occupied handle")["handle"],
        derived
    );
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":derived,"control":"comment","value":"edited derived"}),
    );
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":derived,"action":"save"}),
    );
    assert_eq!(
        app.session.project().vocabulary_comments["cat"],
        "edited base"
    );
    assert_eq!(
        app.session.project().formatted_word_comments["cat"],
        "edited derived"
    );
}

#[test]
fn equal_full_comment_targets_reuse_drafts_across_token_coordinates() {
    let (mut app, ctx) = same_spelling_comments();
    for (token, target_kind) in [(1, "base_word"), (0, "formatted_word")] {
        let handle = open(
            &mut app,
            &ctx,
            json!({"kind":"word-comment","segment":0,"token":token}),
        );
        execute(
            &mut app,
            &ctx,
            "set",
            json!({"handle":handle,"control":"comment","value":"preserved draft"}),
        );
        let reused = execute(
            &mut app,
            &ctx,
            "open",
            json!({"kind":"word-comment","segment":1,"token":token}),
        );
        assert_eq!(reused["handle"], handle);
        assert_eq!(reused["reused"], true);
        assert_eq!(comment_value(&mut app, &ctx, &handle), "preserved draft");
        let inspection = execute(&mut app, &ctx, "inspect", json!({"handle":handle}));
        assert_eq!(
            inspection["target"],
            json!({"word":"cat","comment_target":{"kind":target_kind,"word":"cat"}})
        );
        app.sync_control_windows();
        app.sync_control_windows();
        assert_eq!(
            execute(&mut app, &ctx, "inspect", json!({"handle":handle}))["target"],
            inspection["target"]
        );
        assert!(!app.session.is_dirty());
        execute(
            &mut app,
            &ctx,
            "invoke",
            json!({"handle":handle,"action":"cancel"}),
        );
    }
    assert_eq!(
        app.session.project().vocabulary_comments["cat"],
        "base comment"
    );
    assert_eq!(
        app.session.project().formatted_word_comments["cat"],
        "derived comment"
    );
}

#[test]
fn registry_reconciliation_replaces_identity_when_comment_ownership_changes() {
    let (mut app, ctx) = same_spelling_comments();
    let base = open(
        &mut app,
        &ctx,
        json!({"kind":"word-comment","segment":0,"token":1}),
    );
    // Model replacement of the typed GUI draft independently of control_open.
    // Registry reconciliation must use its actual ownership, not just its title.
    let (target, comment) = app.session.token_comment(1, 0).expect("derived comment");
    app.update_comment_popup = Some(UpdateCommentDialog {
        word: "cat".into(),
        target,
        comment,
    });
    app.sync_control_windows();
    let derived = app
        .control_windows
        .borrow()
        .find("word-comment", None)
        .expect("replacement comment window")
        .handle
        .clone();
    assert_ne!(
        base, derived,
        "changing comment ownership must invalidate the previous window identity"
    );
    assert_eq!(
        app.control_execute(&ctx, "inspect", json!({"handle":base}))
            .expect_err("replaced base target")
            .code,
        "stale_handle"
    );
    let inspection = execute(&mut app, &ctx, "inspect", json!({"handle":derived}));
    assert_eq!(
        inspection["target"],
        json!({"word":"cat","comment_target":{"kind":"formatted_word","word":"cat"}})
    );
    let reused = execute(
        &mut app,
        &ctx,
        "open",
        json!({"kind":"word-comment","segment":0,"token":0}),
    );
    assert_eq!(reused["handle"], derived);
    assert_eq!(reused["reused"], true);
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":derived,"control":"comment","value":"registry-selected derived"}),
    );
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":derived,"action":"save"}),
    );
    assert_eq!(
        app.session.project().vocabulary_comments["cat"],
        "base comment"
    );
    assert_eq!(
        app.session.project().formatted_word_comments["cat"],
        "registry-selected derived"
    );
}
