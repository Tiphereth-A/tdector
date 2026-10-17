use super::*;
use crate::enums::PopupRequest;

fn right_click(
    app: &mut DecryptionApp,
    ctx: &Context,
    kind: &str,
    segment: usize,
    position: egui::Pos2,
) {
    let request = if kind == "word-menu" {
        let word = app.session.project().segments[segment].tokens[0]
            .original
            .clone();
        PopupRequest::WordMenu(word, segment, 0, position)
    } else {
        PopupRequest::SentenceMenu(segment, position)
    };
    app.handle_popup_request(request, ctx);
}

fn render_menus(app: &mut DecryptionApp, ctx: &Context) {
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 900.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.sync_control_windows();
            app.render_popups(ui.ctx(), &None, &None, &mut None);
            app.sync_control_windows();
        });
        output.textures_delta.clear();
    }
}

fn menu_handle(app: &DecryptionApp, kind: &str) -> String {
    app.control_windows
        .borrow()
        .find(kind, None)
        .unwrap()
        .handle
        .clone()
}

fn assert_position(app: &mut DecryptionApp, ctx: &Context, handle: &str, position: egui::Pos2) {
    let inspected = execute(app, ctx, "inspect", json!({"handle":handle}));
    let actual = &inspected["geometry"]["position"];
    assert!(
        (actual[0].as_f64().unwrap() - f64::from(position.x)).abs() < 2.0
            && (actual[1].as_f64().unwrap() - f64::from(position.y)).abs() < 2.0,
        "requested {position:?}, observed {actual}"
    );
}

fn mouse_menu_retargets(kind: &str) {
    let (mut app, ctx, _) = app();
    right_click(&mut app, &ctx, kind, 0, egui::pos2(80.0, 100.0));
    render_menus(&mut app, &ctx);
    let original = menu_handle(&app, kind);
    assert_position(&mut app, &ctx, &original, egui::pos2(80.0, 100.0));

    let position = egui::pos2(400.0, 300.0);
    right_click(&mut app, &ctx, kind, 1, position);
    render_menus(&mut app, &ctx);
    assert!(
        app.error_message.is_none(),
        "right-click replacement failed: {:?}",
        app.error_message
    );
    let replacement = menu_handle(&app, kind);
    assert_ne!(original, replacement);
    let target =
        execute(&mut app, &ctx, "inspect", json!({"handle":replacement}))["target"].clone();
    assert_eq!(target["segment"], 1);
    if kind == "word-menu" {
        assert_eq!(target["token"], 0);
        assert_eq!(target["word"], "dog");
        assert_eq!(
            app.word_menu_popup.as_ref().unwrap(),
            &("dog".into(), 1, 0, position)
        );
    } else {
        assert_eq!(app.sentence_menu_popup.as_ref().unwrap(), &(1, position));
    }
    assert_position(&mut app, &ctx, &replacement, position);
    assert_eq!(
        app.control_execute(&ctx, "inspect", json!({"handle":original}))
            .unwrap_err()
            .code,
        "stale_handle"
    );
}

fn mouse_menu_repositions_same_target(kind: &str) {
    let (mut app, ctx, _) = app();
    right_click(&mut app, &ctx, kind, 0, egui::pos2(80.0, 100.0));
    render_menus(&mut app, &ctx);
    let original = menu_handle(&app, kind);
    let position = egui::pos2(500.0, 300.0);
    right_click(&mut app, &ctx, kind, 0, position);
    render_menus(&mut app, &ctx);
    assert!(app.error_message.is_none());
    assert_eq!(menu_handle(&app, kind), original);
    assert_position(&mut app, &ctx, &original, position);
}

#[test]
fn mouse_word_menu_replaces_the_previous_token_target() {
    mouse_menu_retargets("word-menu");
}

#[test]
fn mouse_segment_menu_replaces_the_previous_segment_target() {
    mouse_menu_retargets("segment-menu");
}

#[test]
fn mouse_word_menu_same_target_keeps_handle_and_moves_to_latest_click() {
    mouse_menu_repositions_same_target("word-menu");
}

#[test]
fn mouse_segment_menu_same_target_keeps_handle_and_moves_to_latest_click() {
    mouse_menu_repositions_same_target("segment-menu");
}

#[test]
fn direct_menu_open_retains_singleton_conflicts_and_same_target_reuse() {
    for kind in ["word-menu", "segment-menu"] {
        let (mut app, ctx, _) = app();
        let original = open(&mut app, &ctx, json!({"kind":kind,"segment":0,"token":0}));
        let error = app
            .control_execute(&ctx, "open", json!({"kind":kind,"segment":1,"token":0}))
            .unwrap_err();
        assert_eq!(error.code, "already_open");
        assert_eq!(error.details.unwrap()["handle"], original);
        assert_eq!(menu_handle(&app, kind), original);
        let reused = execute(
            &mut app,
            &ctx,
            "open",
            json!({"kind":kind,"segment":0,"token":0}),
        );
        assert_eq!(reused["handle"], original);
        assert_eq!(reused["reused"], true);
    }
}
