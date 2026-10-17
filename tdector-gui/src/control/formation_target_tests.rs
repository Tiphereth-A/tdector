use super::*;

fn duplicate_surface_app() -> (DecryptionApp, Context) {
    let (mut app, ctx, _) = app();
    let project = json!({
        "version": 2,
        "project_name": "Same spelling, different formation",
        "formation": [{
            "description": "Identity inflection",
            "type": "inflection",
            "command": "fn transform(word) { word }"
        }],
        "vocabulary": {
            "original": [
                {"word": "cat", "meaning": "animal"},
                {"word": "dog", "meaning": "another animal"}
            ],
            "formatted": [{"word": [0, 0]}]
        },
        "sentences": [{"words": [0, -1, 1], "meaning": ""}]
    });
    app.session
        .load_json(&project.to_string())
        .expect("original and identity-derived forms are a valid project");
    app.sync_control_windows();
    let tokens = &app.session.project().segments[0].tokens;
    assert_eq!(tokens[0].original, "cat");
    assert_eq!(tokens[0].base_word.as_deref(), Some("cat"));
    assert!(tokens[0].formation_rule_indices.is_empty());
    assert_eq!(tokens[1].original, "cat");
    assert_eq!(tokens[1].base_word.as_deref(), Some("cat"));
    assert_eq!(tokens[1].formation_rule_indices, [0]);
    assert!(!app.session.is_dirty());
    (app, ctx)
}

fn assert_formation_draft(
    app: &mut DecryptionApp,
    ctx: &Context,
    handle: &str,
    base: &str,
    rule: Option<usize>,
    preview: &str,
) {
    for (name, value) in [
        ("base-word", json!(base)),
        ("rule", json!(rule)),
        ("preview", json!(preview)),
    ] {
        assert_eq!(
            execute(app, ctx, "get", json!({"handle":handle,"control":name}))["value"],
            value,
            "formation draft field {name}"
        );
    }
}

#[test]
fn mouse_formation_open_uses_the_clicked_tokens_chain_not_the_first_matching_word() {
    let (mut app, ctx) = duplicate_surface_app();
    let revision = app.session.revision();

    // This is the same mouse lifecycle entrypoint used by the word-menu button.
    assert!(app.open_from_mouse(&ctx, "word-formation", Some("cat"), Some(0), Some(1)));
    assert!(app.error_message.is_none());
    let windows = execute(&mut app, &ctx, "windows", json!({}));
    let handle = windows["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| window["kind"] == "word-formation")
        .expect("mouse-opened formation is discoverable")["handle"]
        .as_str()
        .unwrap();
    assert_formation_draft(&mut app, &ctx, handle, "cat", Some(0), "cat");
    assert_eq!(app.session.revision(), revision);
    assert!(!app.session.is_dirty());
}

#[test]
fn coordinate_formation_open_seeds_the_selected_token_while_word_only_remains_compatible() {
    let (mut app, ctx) = duplicate_surface_app();
    let selected = open(
        &mut app,
        &ctx,
        json!({"kind":"word-formation","word":"cat","segment":0,"token":1}),
    );
    assert_formation_draft(&mut app, &ctx, &selected, "cat", Some(0), "cat");
    execute(&mut app, &ctx, "close", json!({"handle":selected}));

    // A word-only request still uses the first matching occurrence, here the original.
    let word_only = open(
        &mut app,
        &ctx,
        json!({"kind":"word-formation","word":"cat"}),
    );
    assert_formation_draft(&mut app, &ctx, &word_only, "cat", None, "");
    assert!(!app.session.is_dirty());
}

#[test]
fn invalid_formation_coordinates_are_rejected_before_opening_a_draft() {
    for (args, code) in [
        (
            json!({"kind":"word-formation","word":"cat","segment":99,"token":1}),
            "invalid_index",
        ),
        (
            json!({"kind":"word-formation","word":"cat","segment":0,"token":99}),
            "invalid_index",
        ),
        (
            json!({"kind":"word-formation","word":"cat","segment":0}),
            "invalid_input",
        ),
        (
            json!({"kind":"word-formation","word":"cat","token":1}),
            "invalid_input",
        ),
        (
            json!({"kind":"word-formation","word":"cat","segment":-1,"token":1}),
            "invalid_input",
        ),
        (
            json!({"kind":"word-formation","word":"dog","segment":0,"token":1}),
            "invalid_input",
        ),
    ] {
        let (mut app, ctx) = duplicate_surface_app();
        let windows = execute(&mut app, &ctx, "windows", json!({}));
        let revision = app.session.revision();
        let snapshot = app.session.save_snapshot().unwrap().bytes;
        let error = app
            .control_execute(&ctx, "open", args.clone())
            .expect_err("invalid coordinates must not create a formation draft");
        assert_eq!(error.code, code, "request: {args}");
        assert!(app.word_formation_popup.is_none());
        assert_eq!(execute(&mut app, &ctx, "windows", json!({})), windows);
        assert_eq!(app.session.revision(), revision);
        assert_eq!(app.session.save_snapshot().unwrap().bytes, snapshot);
        assert!(!app.session.is_dirty());
    }
}

#[test]
fn invalid_formation_coordinates_do_not_reuse_or_overwrite_an_existing_draft() {
    let (mut app, ctx) = duplicate_surface_app();
    let handle = open(
        &mut app,
        &ctx,
        json!({"kind":"word-formation","word":"cat"}),
    );
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":handle,"control":"base-word","value":"user draft"}),
    );
    let before = execute(&mut app, &ctx, "inspect", json!({"handle":handle}));
    for (args, code) in [
        (
            json!({"kind":"word-formation","word":"cat","segment":0,"token":99}),
            "invalid_index",
        ),
        (
            json!({"kind":"word-formation","word":"cat","segment":0,"token":2}),
            "invalid_input",
        ),
    ] {
        let error = app
            .control_execute(&ctx, "open", args)
            .expect_err("coordinates must be validated before same-word draft reuse");
        assert_eq!(error.code, code);
        assert_eq!(
            execute(&mut app, &ctx, "inspect", json!({"handle":handle})),
            before
        );
        assert!(!app.session.is_dirty());
    }
}
