use super::*;

#[path = "comment_target_tests.rs"]
mod comment_target_tests;
#[path = "formation_target_tests.rs"]
mod formation_target_tests;
#[path = "mouse_menu_tests.rs"]
mod mouse_menu_tests;

fn app() -> (DecryptionApp, Context, String) {
    let ctx = Context::default();
    let mut app = DecryptionApp::default();
    assert!(app.import_text(
        "cat cats\ndog cat",
        "Fixture",
        &TokenizationRule::default_whitespace(),
        &ctx
    ));
    assert!(app.apply_command(
        Command::SetGloss {
            word: "cat".into(),
            meaning: "animal".into()
        },
        &ctx
    ));
    let saved = app.session.save_snapshot().expect("snapshot");
    assert!(app.session.acknowledge_saved(saved.token));
    app.sync_control_windows();
    let main = app.control_windows.borrow().main_handle().to_owned();
    (app, ctx, main)
}

fn execute(app: &mut DecryptionApp, ctx: &Context, command: &str, args: Value) -> Value {
    app.control_execute(ctx, command, args)
        .unwrap_or_else(|e| panic!("{command}: {}: {}", e.code, e.message))
}

fn open(app: &mut DecryptionApp, ctx: &Context, args: Value) -> String {
    execute(app, ctx, "open", args)["handle"]
        .as_str()
        .expect("handle")
        .into()
}

#[test]
fn drafts_round_trip_unicode_and_multiline_without_dirtying_until_save() {
    let (mut app, ctx, _) = app();
    let handle = open(
        &mut app,
        &ctx,
        json!({"kind":"segment-comment","segment":0}),
    );
    let text = "猫 🐈\nsecond line\r\n";
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":handle,"control":"comment","value":text}),
    );
    assert_eq!(
        execute(
            &mut app,
            &ctx,
            "get",
            json!({"handle":handle,"control":"comment"})
        )["value"],
        text
    );
    assert!(!app.session.is_dirty());
    assert_eq!(app.session.project().segments[0].comment, "");
    let reused = execute(
        &mut app,
        &ctx,
        "open",
        json!({"kind":"segment-comment","segment":0}),
    );
    assert_eq!(reused["handle"], handle);
    assert_eq!(reused["reused"], true);
    let conflict = app
        .control_execute(&ctx, "open", json!({"kind":"segment-comment","segment":1}))
        .expect_err("occupied singleton");
    assert_eq!(conflict.code, "already_open");
    assert_eq!(conflict.details.expect("existing handle")["handle"], handle);
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":handle,"action":"save"}),
    );
    assert!(app.session.is_dirty());
    assert_eq!(app.session.project().segments[0].comment, text);
    assert_eq!(
        app.control_execute(&ctx, "inspect", json!({"handle":handle}))
            .expect_err("closed")
            .code,
        "stale_handle"
    );
}

#[test]
fn cancel_discards_draft_and_reopen_uses_new_handle() {
    let (mut app, ctx, _) = app();
    let h = open(
        &mut app,
        &ctx,
        json!({"kind":"word-comment","segment":0,"token":0}),
    );
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":h,"control":"comment","value":"discard"}),
    );
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":h,"action":"cancel"}),
    );
    let next = open(
        &mut app,
        &ctx,
        json!({"kind":"word-comment","segment":0,"token":0}),
    );
    assert_ne!(h, next);
    assert_eq!(
        execute(
            &mut app,
            &ctx,
            "get",
            json!({"handle":next,"control":"comment"})
        )["value"],
        ""
    );
    assert!(!app.session.is_dirty());
}

#[test]
fn formation_fields_refresh_preview_and_disabled_actions_are_rejected() {
    let (mut app, ctx, _) = app();
    let h = open(&mut app, &ctx, json!({"kind":"new-formation-rule"}));
    assert_eq!(
        app.control_execute(&ctx, "invoke", json!({"handle":h,"action":"create"}))
            .expect_err("empty description")
            .code,
        "action_disabled"
    );
    for (control, value) in [
        ("description", "Plural"),
        ("script", "fn transform(word) { word + \"s\" }"),
        ("test-word", "cat"),
    ] {
        execute(
            &mut app,
            &ctx,
            "set",
            json!({"handle":h,"control":control,"value":value}),
        );
    }
    assert_eq!(
        execute(
            &mut app,
            &ctx,
            "get",
            json!({"handle":h,"control":"preview"})
        )["value"],
        "cats"
    );
    assert!(!app.session.is_dirty());
    assert_eq!(
        app.control_execute(
            &ctx,
            "set",
            json!({"handle":h,"control":"preview","value":"override"})
        )
        .expect_err("readonly")
        .code,
        "read_only_control"
    );
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":h,"action":"create"}),
    );
    let h = open(
        &mut app,
        &ctx,
        json!({"kind":"word-formation","word":"cats"}),
    );
    assert_eq!(
        app.control_execute(&ctx, "invoke", json!({"handle":h,"action":"apply"}))
            .expect_err("no rule")
            .code,
        "action_disabled"
    );
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":h,"control":"base-word","value":"cat"}),
    );
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":h,"control":"rule","value":0}),
    );
    assert_eq!(
        execute(
            &mut app,
            &ctx,
            "get",
            json!({"handle":h,"control":"preview"})
        )["value"],
        "cats"
    );
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":h,"action":"apply"}),
    );
    let main = app.control_windows.borrow().main_handle().to_owned();
    let generation = app.control_windows.borrow().generation;
    assert_eq!(app.control_execute(&ctx,"set",json!({"handle":main,"control":format!("g{generation}:segment:0:token:1:gloss"),"value":"blocked"})).expect_err("derived gloss").code,"read_only_control");
    let chain = open(
        &mut app,
        &ctx,
        json!({"kind":"formation-chain","segment":0,"token":1}),
    );
    assert_eq!(
        execute(&mut app, &ctx, "inspect", json!({"handle":chain}))["results"]["steps"][0]["description"],
        "Plural"
    );
}

#[test]
fn immediate_edits_noops_sort_modes_and_project_generation() {
    let (mut app, ctx, main) = app();
    let generation = app.control_windows.borrow().generation;
    let id = format!("g{generation}:segment:0:translation");
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":main,"control":id,"value":"猫"}),
    );
    let revision = app.session.revision();
    let noop = execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":main,"control":id,"value":"猫"}),
    );
    assert_eq!(noop["changed"], false);
    assert_eq!(revision, app.session.revision());
    for sort in [
        "index-asc",
        "index-desc",
        "original-asc",
        "original-desc",
        "length-asc",
        "length-desc",
        "count-asc",
        "count-desc",
        "translated-ratio-asc",
        "translated-ratio-desc",
        "translated-count-asc",
        "translated-count-desc",
    ] {
        execute(
            &mut app,
            &ctx,
            "set",
            json!({"handle":main,"control":"sort","value":sort}),
        );
        assert_eq!(
            execute(
                &mut app,
                &ctx,
                "get",
                json!({"handle":main,"control":"sort"})
            )["value"],
            sort
        );
    }
    let popup = open(&mut app, &ctx, json!({"kind":"references","word":"cat"}));
    assert!(app.import_text(
        "replacement",
        "New",
        &TokenizationRule::default_whitespace(),
        &ctx
    ));
    assert_eq!(app.control_windows.borrow().main_handle(), main);
    assert_eq!(
        app.control_execute(&ctx, "inspect", json!({"handle":popup}))
            .expect_err("stale popup")
            .code,
        "stale_handle"
    );
    assert_eq!(
        app.control_execute(
            &ctx,
            "set",
            json!({"handle":main,"control":id,"value":"no"})
        )
        .expect_err("stale control")
        .code,
        "unknown_control"
    );
}

#[test]
fn pin_preserves_handles_and_sessions_reject_each_others_handles() {
    let (mut app, ctx, _) = app();
    let h = open(&mut app, &ctx, json!({"kind":"references","word":"cat"}));
    let pinned = execute(
        &mut app,
        &ctx,
        "open",
        json!({"kind":"references","word":"cat","pinned":true}),
    );
    assert_eq!(pinned["handle"], h);
    assert_eq!(
        execute(&mut app, &ctx, "inspect", json!({"handle":h}))["pinned"],
        true
    );
    let second = open(&mut app, &ctx, json!({"kind":"references","word":"dog"}));
    assert_ne!(h, second);
    let mut other = DecryptionApp::default();
    assert_eq!(
        other
            .control_execute(&ctx, "inspect", json!({"handle":h}))
            .expect_err("different instance")
            .code,
        "stale_handle"
    );
}

#[test]
fn dirty_shutdown_keeps_a_real_confirmation_and_cancel_preserves_project() {
    let (mut app, ctx, _) = app();
    assert!(app.apply_command(
        Command::SetTranslation {
            segment: 0,
            translation: "changed".into()
        },
        &ctx
    ));
    let result = execute(&mut app, &ctx, "quit", json!({}));
    assert_eq!(result["closed"], false);
    let h = result["handle"].as_str().expect("confirmation");
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":h,"action":"cancel"}),
    );
    assert!(!app.closing);
    assert!(app.session.is_dirty());
    let result = execute(&mut app, &ctx, "quit", json!({}));
    let result = execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":result["handle"],"action":"confirm"}),
    );
    assert_eq!(result["closed"], true);
    assert!(app.closing);
}

#[test]
fn invalid_geometry_controls_and_scope_are_rejected() {
    let (mut app, ctx, main) = app();
    assert_eq!(
        app.control_execute(
            &ctx,
            "focus",
            json!({"handle":main,"control":"does-not-exist"})
        )
        .expect_err("unknown")
        .code,
        "unknown_control"
    );
    assert_eq!(
        app.control_execute(
            &ctx,
            "scroll",
            json!({"handle":main,"area":"g999:segment:0","x":0,"y":0})
        )
        .expect_err("stale")
        .code,
        "unknown_control"
    );
    let h = open(&mut app, &ctx, json!({"kind":"segment-menu","segment":0}));
    assert_eq!(
        app.control_execute(&ctx, "resize", json!({"handle":h,"width":300,"height":300}))
            .expect_err("not resizable")
            .code,
        "unsupported_operation"
    );
}

struct Files {
    directory: PathBuf,
}

#[test]
fn all_direct_popup_kinds_support_inspection() {
    let (mut app, ctx, _) = app();
    for args in [
        json!({"kind":"definition","word":"cat"}),
        json!({"kind":"references","word":"cat"}),
        json!({"kind":"similar-tokens","word":"cat"}),
        json!({"kind":"similar-segments","segment":0}),
        json!({"kind":"word-menu","segment":0,"token":0}),
        json!({"kind":"segment-menu","segment":0}),
        json!({"kind":"formation-chain","segment":0,"token":0}),
        json!({"kind":"word-formation","word":"cat"}),
        json!({"kind":"word-comment","segment":0,"token":0}),
        json!({"kind":"segment-comment","segment":0}),
        json!({"kind":"new-formation-rule"}),
    ] {
        let kind = args["kind"].clone();
        let handle = open(&mut app, &ctx, args);
        assert_eq!(
            execute(&mut app, &ctx, "inspect", json!({"handle":handle}))["kind"],
            kind
        );
        execute(&mut app, &ctx, "close", json!({"handle":handle}));
    }
}

#[test]
fn a_new_mouse_operation_invalidates_the_previous_file_confirmation() {
    let files = Files::new();
    let input = files.path("incoming.json");
    let (mut app, ctx, _) = app();
    std::fs::write(&input, app.session.save_snapshot().expect("snapshot").bytes).expect("fixture");
    assert!(app.apply_command(
        Command::SetTranslation {
            segment: 0,
            translation: "dirty".into()
        },
        &ctx
    ));
    let old = execute(&mut app, &ctx, "load", json!({"path":input}))["handle"].clone();
    assert!(app.control_data.pending.is_some());
    app.trigger_action(AppAction::Import, &ctx);
    assert!(app.control_data.pending.is_none());
    assert_eq!(
        app.control_execute(&ctx, "inspect", json!({"handle":old}))
            .expect_err("old confirmation")
            .code,
        "stale_handle"
    );
    let active = app
        .control_windows
        .borrow()
        .find("confirmation", None)
        .expect("replacement")
        .handle
        .clone();
    assert_ne!(old, active);
}
impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "tdector-control-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).expect("test directory");
        Self { directory }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn atomic_save_conflicts_and_reload_acknowledge_the_exact_snapshot() {
    let files = Files::new();
    let output = files.path("project.json");
    let (mut app, ctx, main) = app();
    let first = execute(&mut app, &ctx, "save", json!({"output":output}));
    assert!(first["save_token"].as_str().is_some());
    assert!(!app.session.is_dirty());
    let generation = app.control_windows.borrow().generation;
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":main,"control":format!("g{generation}:segment:0:translation"),"value":"Saved 猫\nline"}),
    );
    execute(&mut app, &ctx, "save", json!({}));
    assert!(!app.session.is_dirty());
    assert!(app.apply_command(
        Command::SetSegmentComment {
            segment: 0,
            comment: "mouse save".into()
        },
        &ctx
    ));
    app.save_project(&ctx);
    assert!(!app.session.is_dirty());
    execute(&mut app, &ctx, "save", json!({}));
    let saved = std::fs::read(&output).expect("saved bytes");
    let mut reloaded = DecryptionApp::default();
    execute(&mut reloaded, &ctx, "load", json!({"path":output}));
    assert_eq!(
        reloaded.session.project().segments[0].translation,
        "Saved 猫\nline"
    );
    std::fs::write(&output, b"external writer").expect("external change");
    let error = app
        .control_execute(&ctx, "save", json!({}))
        .expect_err("conflict");
    assert_eq!(error.code, "input_changed");
    assert!(
        error.details.expect("dialog")["dialog_handle"]
            .as_str()
            .is_some()
    );
    assert_eq!(
        std::fs::read(&output).expect("preserved"),
        b"external writer"
    );
    assert_ne!(saved, b"external writer");
}

#[test]
fn dirty_load_confirmation_retains_its_prepared_path_operation() {
    let files = Files::new();
    let input = files.path("incoming.json");
    let (mut app, ctx, _) = app();
    let mut replacement = tdector_app::Session::default();
    replacement
        .import_text(
            "replacement",
            "Incoming",
            &TokenizationRule::default_whitespace(),
        )
        .expect("import fixture");
    std::fs::write(&input, replacement.save_snapshot().expect("snapshot").bytes)
        .expect("write fixture");
    assert!(app.apply_command(
        Command::SetTranslation {
            segment: 0,
            translation: "dirty".into()
        },
        &ctx
    ));
    let pending = execute(&mut app, &ctx, "load", json!({"path":input}));
    assert_eq!(pending["applied"], false);
    assert_eq!(app.session.project().project_name, "Fixture");
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":pending["handle"],"action":"confirm"}),
    );
    assert_eq!(app.session.project().project_name, "Incoming");
    assert_eq!(
        app.control_data.source.as_ref().expect("bound file").0,
        input
    );
    assert!(!app.session.is_dirty());
    assert!(app.confirmation.is_none());
}

#[test]
fn import_custom_tokenizer_test_apply_and_cancel_follow_draft_semantics() {
    let files = Files::new();
    let input = files.path("import.txt");
    std::fs::write(&input, "alpha beta\ngamma").expect("text");
    let (mut app, ctx, _) = app();
    let imported = execute(&mut app, &ctx, "import", json!({"path":input}));
    let custom = execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":imported["handle"],"action":"custom-script"}),
    );
    let h = custom["handle"].as_str().expect("custom handle");
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":h,"control":"script","value":"fn tokenize(line) { line.split(\" \") }"}),
    );
    execute(
        &mut app,
        &ctx,
        "set",
        json!({"handle":h,"control":"test-text","value":"alpha beta"}),
    );
    let tested = execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":h,"action":"test"}),
    );
    assert_eq!(tested["preview"], json!(["alpha", "beta"]));
    assert!(!app.session.is_dirty());
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":h,"action":"apply"}),
    );
    assert!(app.session.is_dirty());
    assert_eq!(
        app.session.project().segments[0].tokens[0].original,
        "alpha"
    );
    assert!(app.custom_tokenization_popup.is_none());
}

#[test]
fn removal_confirmation_tracks_live_rule_and_disables_after_last_removal() {
    let (mut app, ctx, _) = app();
    for description in ["Plural", "Alternative plural"] {
        assert!(app.apply_command(
            Command::CreateFormationRule {
                description: description.into(),
                rule_type: tdector_eval::FormationType::Inflection,
                command: "fn transform(word) { word + \"s\" }".into(),
            },
            &ctx
        ));
    }
    let apply = |rule| Command::ApplyFormationRule {
        word: "cats".into(),
        base_word: "cat".into(),
        rule,
    };
    assert!(app.apply_command(apply(0), &ctx));
    let handle = open(
        &mut app,
        &ctx,
        json!({"kind":"remove-formation-rule","segment":0,"token":1}),
    );
    assert_eq!(
        execute(
            &mut app,
            &ctx,
            "get",
            json!({"handle":handle,"control":"rule-description"})
        )["value"],
        "Plural"
    );
    assert!(app.apply_command(apply(1), &ctx));
    assert_eq!(
        execute(
            &mut app,
            &ctx,
            "get",
            json!({"handle":handle,"control":"rule-description"})
        )["value"],
        "Alternative plural"
    );
    let inspection = execute(&mut app, &ctx, "inspect", json!({"handle":handle}));
    assert!(
        inspection["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == "remove" && a["enabled"] == true)
    );
    assert!(app.apply_command(
        Command::RemoveFormationRule {
            segment: 0,
            token: 1
        },
        &ctx
    ));
    let inspection = execute(&mut app, &ctx, "inspect", json!({"handle":handle}));
    assert_eq!(inspection["target"]["word"], "cat");
    for (name, value) in [
        ("word", "cat"),
        ("base-word", "cat"),
        ("rule-description", ""),
    ] {
        assert_eq!(
            execute(
                &mut app,
                &ctx,
                "get",
                json!({"handle":handle,"control":name})
            )["value"],
            value
        );
    }
    assert!(
        inspection["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == "remove" && a["enabled"] == false)
    );
    let revision = app.session.revision();
    let error = app
        .control_execute(&ctx, "invoke", json!({"handle":handle,"action":"remove"}))
        .unwrap_err();
    assert_eq!(error.code, "action_disabled");
    assert_eq!(app.session.revision(), revision);
    assert_eq!(
        app.remove_formation_rule_popup
            .as_ref()
            .unwrap()
            .formatted_word,
        "cat"
    );
}

#[test]
fn word_menu_survives_rule_removal_and_opens_references_for_the_current_word() {
    let (mut app, ctx, _) = app();
    assert!(app.apply_command(
        Command::CreateFormationRule {
            description: "Plural".into(),
            rule_type: tdector_eval::FormationType::Inflection,
            command: "fn transform(word) { word + \"s\" }".into(),
        },
        &ctx
    ));
    assert!(app.apply_command(
        Command::ApplyFormationRule {
            word: "cats".into(),
            base_word: "cat".into(),
            rule: 0,
        },
        &ctx
    ));
    let menu = open(
        &mut app,
        &ctx,
        json!({"kind":"word-menu","segment":0,"token":1}),
    );
    let removal = open(
        &mut app,
        &ctx,
        json!({"kind":"remove-formation-rule","segment":0,"token":1}),
    );
    execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":removal,"action":"remove"}),
    );
    let inspected = execute(&mut app, &ctx, "inspect", json!({"handle":menu}));
    assert_eq!(
        inspected["target"],
        json!({"word":"cat","segment":0,"token":1})
    );
    assert_eq!(app.word_menu_popup.as_ref().unwrap().0, "cat");
    let reference = execute(
        &mut app,
        &ctx,
        "invoke",
        json!({"handle":menu,"action":"references"}),
    );
    let inspected = execute(
        &mut app,
        &ctx,
        "inspect",
        json!({"handle":reference["handle"]}),
    );
    assert_eq!(inspected["kind"], "references");
    assert_eq!(inspected["target"]["word"], "cat");
    assert_eq!(app.reference_popup.as_deref(), Some("cat"));
}
