//! Shared UI builders. Control requests are consumed only by the widget they address.
#[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
use crate::control::lifecycle::{WindowGeometry, WindowRegistry};
use crate::ui::states::state::DecryptionApp;
use eframe::egui;
#[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
use std::{cell::RefCell, rc::Rc};

#[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
#[derive(Clone)]
struct Scope {
    registry: Rc<RefCell<WindowRegistry>>,
    handle: String,
}
#[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
thread_local! { static SCOPES: RefCell<Vec<Scope>> = const { RefCell::new(Vec::new()) }; }
pub(crate) struct ScopeGuard;
impl Drop for ScopeGuard {
    fn drop(&mut self) {
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        SCOPES.with(|s| {
            s.borrow_mut().pop();
        });
    }
}

impl DecryptionApp {
    #[cfg(not(all(feature = "gui-control", not(target_arch = "wasm32"))))]
    pub(crate) fn sync_control_windows(&mut self) {}

    pub(crate) fn main_control_scope(&self) -> ScopeGuard {
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        SCOPES.with(|s| {
            s.borrow_mut().push(Scope {
                registry: self.control_windows.clone(),
                handle: self.control_windows.borrow().main_handle().into(),
            })
        });
        ScopeGuard
    }
    pub(crate) fn popup_window<'a>(
        &self,
        kind: &str,
        pin: Option<u64>,
        title: impl Into<egui::WidgetText>,
    ) -> ManagedWindow<'a> {
        let inner = egui::Window::new(title);
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        {
            let registry = self.control_windows.borrow();
            let window = registry
                .find(kind, pin)
                .expect("popup registered before rendering");
            ManagedWindow {
                inner: inner.id(window.egui_id()),
                scope: Scope {
                    registry: self.control_windows.clone(),
                    handle: window.handle.clone(),
                },
                explicit_position: window.explicit_position,
            }
        }
        #[cfg(not(all(feature = "gui-control", not(target_arch = "wasm32"))))]
        {
            let _ = (kind, pin);
            ManagedWindow { inner }
        }
    }
}

pub(crate) struct ManagedWindow<'a> {
    inner: egui::Window<'a>,
    #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
    scope: Scope,
    #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
    explicit_position: bool,
}
impl<'a> ManagedWindow<'a> {
    #[allow(unused_mut)]
    pub fn id(mut self, id: egui::Id) -> Self {
        #[cfg(not(all(feature = "gui-control", not(target_arch = "wasm32"))))]
        {
            self.inner = self.inner.id(id);
        }
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        {
            let _ = id;
        }
        self
    }
    pub fn open(mut self, open: &'a mut bool) -> Self {
        self.inner = self.inner.open(open);
        self
    }
    pub fn default_width(mut self, width: f32) -> Self {
        self.inner = self.inner.default_width(width);
        self
    }
    pub fn default_height(mut self, height: f32) -> Self {
        self.inner = self.inner.default_height(height);
        self
    }
    pub fn collapsible(mut self, value: bool) -> Self {
        self.inner = self.inner.collapsible(value);
        self
    }
    pub fn resizable(mut self, value: bool) -> Self {
        self.inner = self.inner.resizable(value);
        self
    }
    pub fn anchor(mut self, anchor: egui::Align2, offset: [f32; 2]) -> Self {
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        if self.explicit_position {
            return self;
        }
        self.inner = self.inner.anchor(anchor, offset);
        self
    }
    #[allow(unused_mut)]
    pub fn show<R>(
        mut self,
        ctx: &egui::Context,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::InnerResponse<Option<R>>> {
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        {
            let mut registry = self.scope.registry.borrow_mut();
            if let Some(window) = registry.lookup_mut(&self.scope.handle) {
                if let Some(pos) = window.requested_position.take() {
                    self.inner = self
                        .inner
                        .current_pos(pos)
                        .drag_area(egui::WindowDrag::Anywhere);
                }
                if let Some(geometry) = &window.geometry
                    && let Some(size) = window.requested_size.take()
                {
                    // Window sizes include title/frame chrome in egui 0.36. The protocol uses content size.
                    let outer = [
                        size[0] + geometry.outer_size[0] - geometry.size[0],
                        size[1] + geometry.outer_size[1] - geometry.size[1],
                    ];
                    // A one-pass constraint updates egui's resize state. Later frames and manual resizing remain free.
                    self.inner = self.inner.fixed_size(outer);
                }
                if window.focus.as_deref().is_some_and(|name| !name.is_empty()) {
                    let mut state =
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ctx,
                            window.egui_id().with("collapsing"),
                            true,
                        );
                    state.set_open(true);
                    state.store(ctx);
                } else if window.focus.as_deref() == Some("") {
                    window.focus = None;
                }
            }
        }
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        let _scope = {
            SCOPES.with(|s| s.borrow_mut().push(self.scope.clone()));
            ScopeGuard
        };
        let mut content_rect = None;
        let result = self.inner.show(ctx, |ui| {
            let result = content(ui);
            content_rect = Some(ui.min_rect());
            result
        });
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        if let Some(response) = &result {
            let outer = response.response.rect;
            let content = content_rect.unwrap_or(outer);
            if let Some(window) = self
                .scope
                .registry
                .borrow_mut()
                .lookup_mut(&self.scope.handle)
            {
                window.geometry = Some(WindowGeometry {
                    position: outer.min.into(),
                    size: content.size().into(),
                    outer_position: outer.min.into(),
                    outer_size: outer.size().into(),
                    units: "viewport-egui-points".into(),
                });
                if window.requested_size.is_some() {
                    ctx.request_discard("measure window chrome before requested content resize");
                    ctx.request_repaint();
                }
            }
        }
        result
    }
}

pub(crate) fn control_id(ui: &egui::Ui, name: &str) -> egui::Id {
    #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
    if let Some(scope) = SCOPES.with(|s| s.borrow().last().cloned()) {
        let mut registry = scope.registry.borrow_mut();
        let generation = registry.generation;
        if let Some(window) = registry.lookup_mut(&scope.handle) {
            let id = window.egui_id().with(generation).with(name);
            if window.focus.as_deref() == Some(name) && !ui.is_sizing_pass() {
                ui.ctx().memory_mut(|memory| memory.request_focus(id));
                window.focus = None;
            }
            return id;
        }
    }
    ui.make_persistent_id(("control", name))
}

pub(crate) fn scroll_area(ui: &egui::Ui, name: &str, scroll: egui::ScrollArea) -> egui::ScrollArea {
    #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
    if let Some(scope) = SCOPES.with(|s| s.borrow().last().cloned()) {
        let mut registry = scope.registry.borrow_mut();
        let generation = registry.generation;
        if let Some(window) = registry.lookup_mut(&scope.handle) {
            let id = window.egui_id().with(generation).with(("scroll", name));
            let mut scroll = scroll.id_salt(id);
            if let Some(offset) = window.scroll.remove(name) {
                scroll = scroll.scroll_offset(offset.into());
            }
            return scroll;
        }
    }
    scroll.id_salt(ui.make_persistent_id(("scroll", name)))
}

pub(crate) struct ManagedArea {
    inner: egui::Area,
    #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
    scope: Scope,
}
impl DecryptionApp {
    pub(crate) fn popup_area(&self, kind: &str, position: egui::Pos2) -> ManagedArea {
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        {
            let mut registry = self.control_windows.borrow_mut();
            let window = registry
                .find_mut(kind, None)
                .expect("menu registered before rendering");
            let mut inner = egui::Area::new(window.egui_id())
                .order(egui::Order::Foreground)
                .movable(true)
                .default_pos(position);
            if let Some(position) = window.requested_position.take() {
                inner = inner.current_pos(position);
            }
            ManagedArea {
                inner,
                scope: Scope {
                    registry: self.control_windows.clone(),
                    handle: window.handle.clone(),
                },
            }
        }
        #[cfg(not(all(feature = "gui-control", not(target_arch = "wasm32"))))]
        {
            ManagedArea {
                inner: egui::Area::new(egui::Id::unique(kind))
                    .order(egui::Order::Foreground)
                    .movable(false)
                    .fixed_pos(position),
            }
        }
    }
}
impl ManagedArea {
    pub fn show<R>(
        self,
        ctx: &egui::Context,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        let _scope = {
            SCOPES.with(|s| s.borrow_mut().push(self.scope.clone()));
            ScopeGuard
        };
        let response = self.inner.show(ctx, content);
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        if let Some(window) = self
            .scope
            .registry
            .borrow_mut()
            .lookup_mut(&self.scope.handle)
        {
            let rect = response.response.rect;
            window.geometry = Some(WindowGeometry {
                position: rect.min.into(),
                size: rect.size().into(),
                outer_position: rect.min.into(),
                outer_size: rect.size().into(),
                units: "viewport-egui-points".into(),
            });
            if window.focus.as_deref() == Some("") {
                window.focus = None;
            }
        }
        response
    }
}

pub(crate) fn focus_response(ui: &egui::Ui, name: &str, response: &egui::Response) {
    #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
    if let Some(scope) = SCOPES.with(|s| s.borrow().last().cloned())
        && let Some(window) = scope.registry.borrow_mut().lookup_mut(&scope.handle)
        && window.focus.as_deref() == Some(name)
        && !ui.is_sizing_pass()
    {
        response.request_focus();
        window.focus = None;
    }
    #[cfg(not(all(feature = "gui-control", not(target_arch = "wasm32"))))]
    let _ = (ui, name, response);
}

#[cfg(all(test, feature = "gui-control", not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::ui::states::state::UpdateSentenceCommentDialog;
    fn render(app: &mut DecryptionApp, ctx: &egui::Context) {
        render_events(app, ctx, vec![]);
    }
    fn render_events(app: &mut DecryptionApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 900.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.sync_control_windows();
            let _scope = app.main_control_scope();
            app.render_error_dialog(ui.ctx());
            app.render_popups(ui.ctx(), &None, &None, &mut None);
            app.render_pinned_popups(ui.ctx(), &None, &None, &mut None);
            app.sync_control_windows();
        });
        output.textures_delta.clear();
    }
    #[test]
    fn centered_dialog_can_move_and_does_not_snap_back() {
        let mut app = DecryptionApp::default();
        let ctx = egui::Context::default();
        app.error_message = Some("Example error".into());
        render(&mut app, &ctx);
        let handle = app
            .control_windows
            .borrow()
            .find("error", None)
            .unwrap()
            .handle
            .clone();
        app.control_geometry(
            &ctx,
            "move",
            &serde_json::json!({"handle":handle,"x":100,"y":120}),
        )
        .unwrap();
        render(&mut app, &ctx);
        render(&mut app, &ctx);
        let registry = app.control_windows.borrow();
        let window = registry.lookup(&handle).unwrap();
        let position = window.geometry.as_ref().unwrap().position;
        assert!(
            (position[0] - 100.0).abs() < 2.0 && (position[1] - 120.0).abs() < 2.0,
            "{position:?}"
        );
        assert!(window.requested_position.is_none());
    }
    #[test]
    fn resize_is_retained_after_one_pass_constraint_and_drafts_keep_handles() {
        let mut app = DecryptionApp::default();
        let ctx = egui::Context::default();
        app.reference_popup = Some("example".into());
        app.update_sentence_comment_popup = Some(UpdateSentenceCommentDialog {
            segment_idx: 0,
            comment: "Draft".into(),
        });
        app.sync_control_windows();
        let handle = app
            .control_windows
            .borrow()
            .find("references", None)
            .unwrap()
            .handle
            .clone();
        let draft = app
            .control_windows
            .borrow()
            .find("segment-comment", None)
            .unwrap()
            .handle
            .clone();
        render(&mut app, &ctx);
        app.control_geometry(
            &ctx,
            "resize",
            &serde_json::json!({"handle":handle,"width":420,"height":310}),
        )
        .unwrap();
        render(&mut app, &ctx);
        let initial = app
            .control_windows
            .borrow()
            .lookup(&handle)
            .unwrap()
            .geometry
            .clone()
            .unwrap();
        render(&mut app, &ctx);
        let registry = app.control_windows.borrow();
        let after = registry.lookup(&handle).unwrap().geometry.as_ref().unwrap();
        assert_eq!(initial.size, after.size);
        assert!(
            (after.size[0] - 420.0).abs() < 1.0 && (after.size[1] - 310.0).abs() < 1.0,
            "{after:?}"
        );
        assert!(registry.lookup(&handle).unwrap().requested_size.is_none());
        assert!(registry.lookup(&draft).is_some());
    }

    fn drag(app: &mut DecryptionApp, ctx: &egui::Context, start: egui::Pos2, delta: egui::Vec2) {
        let pointer = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        render_events(
            app,
            ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        render_events(
            app,
            ctx,
            vec![egui::Event::PointerMoved(start + delta * 0.5)],
        );
        render_events(app, ctx, vec![egui::Event::PointerMoved(start + delta)]);
        render_events(app, ctx, vec![pointer(start + delta, false)]);
        render(app, ctx);
    }
    #[test]
    fn programmatic_move_and_resize_release_control_for_later_mouse_drags() {
        let mut app = DecryptionApp::default();
        let ctx = egui::Context::default();
        app.reference_popup = Some("example".into());
        render(&mut app, &ctx);
        let handle = app
            .control_windows
            .borrow()
            .find("references", None)
            .unwrap()
            .handle
            .clone();
        app.control_geometry(
            &ctx,
            "move",
            &serde_json::json!({"handle":handle,"x":100,"y":120}),
        )
        .unwrap();
        app.control_geometry(
            &ctx,
            "resize",
            &serde_json::json!({"handle":handle,"width":420,"height":310}),
        )
        .unwrap();
        render(&mut app, &ctx);
        render(&mut app, &ctx);
        let before = app
            .control_windows
            .borrow()
            .lookup(&handle)
            .unwrap()
            .geometry
            .clone()
            .unwrap();
        let start = egui::pos2(before.position[0] + 80.0, before.position[1] + 15.0);
        drag(&mut app, &ctx, start, egui::vec2(70.0, 50.0));
        let moved = app
            .control_windows
            .borrow()
            .lookup(&handle)
            .unwrap()
            .geometry
            .clone()
            .unwrap();
        assert!(
            moved.position[0] > before.position[0] + 20.0
                && moved.position[1] > before.position[1] + 10.0,
            "before={before:?} moved={moved:?}"
        );
        let start = egui::pos2(
            moved.position[0] + moved.outer_size[0] - 1.0,
            moved.position[1] + moved.outer_size[1] - 1.0,
        );
        drag(&mut app, &ctx, start, egui::vec2(50.0, 40.0));
        let resized = app
            .control_windows
            .borrow()
            .lookup(&handle)
            .unwrap()
            .geometry
            .clone()
            .unwrap();
        assert!(
            resized.size[0] > moved.size[0] + 20.0 && resized.size[1] > moved.size[1] + 10.0,
            "before={moved:?} resized={resized:?}"
        );
    }
    #[test]
    fn focusing_a_field_expands_a_collapsed_window() {
        let mut app = DecryptionApp::default();
        let ctx = egui::Context::default();
        app.update_sentence_comment_popup = Some(UpdateSentenceCommentDialog {
            segment_idx: 0,
            comment: "Draft".into(),
        });
        render(&mut app, &ctx);
        render(&mut app, &ctx);
        let window = app
            .control_windows
            .borrow()
            .find("segment-comment", None)
            .unwrap()
            .clone();
        let collapse_id = window.egui_id().with("collapsing");
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            &ctx,
            collapse_id,
            true,
        );
        state.set_open(false);
        state.store(&ctx);
        render(&mut app, &ctx);
        assert!(
            !egui::collapsing_header::CollapsingState::load(&ctx, collapse_id)
                .unwrap()
                .is_open()
        );
        app.control_geometry(
            &ctx,
            "focus",
            &serde_json::json!({"handle":window.handle,"control":"comment"}),
        )
        .unwrap();
        render(&mut app, &ctx);
        render(&mut app, &ctx);
        assert!(
            egui::collapsing_header::CollapsingState::load(&ctx, collapse_id)
                .unwrap()
                .is_open()
        );
        let registry = app.control_windows.borrow();
        let id = window.egui_id().with(registry.generation).with("comment");
        assert_eq!(ctx.memory(|m| m.focused()), Some(id));
        assert!(registry.lookup(&window.handle).unwrap().focus.is_none());
    }
    #[test]
    fn focus_raises_the_actual_foreground_menu_layer() {
        let mut app = DecryptionApp::default();
        let ctx = egui::Context::default();
        app.word_menu_popup = Some(("cat".into(), 0, 0, egui::pos2(100.0, 100.0)));
        app.sentence_menu_popup = Some((0, egui::pos2(100.0, 100.0)));
        render(&mut app, &ctx);
        render(&mut app, &ctx);
        let window = app
            .control_windows
            .borrow()
            .find("word-menu", None)
            .unwrap()
            .clone();
        app.control_geometry(&ctx, "focus", &serde_json::json!({"handle":window.handle}))
            .unwrap();
        render(&mut app, &ctx);
        let position = app
            .control_windows
            .borrow()
            .lookup(&window.handle)
            .unwrap()
            .geometry
            .as_ref()
            .unwrap()
            .position;
        assert_eq!(
            ctx.layer_id_at(egui::pos2(position[0] + 10.0, position[1] + 10.0)),
            Some(egui::LayerId::new(
                egui::Order::Foreground,
                window.egui_id()
            ))
        );
        assert!(
            app.control_windows
                .borrow()
                .lookup(&window.handle)
                .unwrap()
                .focus
                .is_none()
        );
    }
    #[test]
    fn focus_and_scroll_are_consumed_once() {
        let mut app = DecryptionApp::default();
        let ctx = egui::Context::default();
        app.update_sentence_comment_popup = Some(UpdateSentenceCommentDialog {
            segment_idx: 0,
            comment: "Draft".into(),
        });
        app.reference_popup = Some("example".into());
        app.sync_control_windows();
        let comment = app
            .control_windows
            .borrow()
            .find("segment-comment", None)
            .unwrap()
            .handle
            .clone();
        let reference = app
            .control_windows
            .borrow()
            .find("references", None)
            .unwrap()
            .handle
            .clone();
        app.control_geometry(
            &ctx,
            "focus",
            &serde_json::json!({"handle":comment,"control":"comment"}),
        )
        .unwrap();
        app.control_geometry(
            &ctx,
            "scroll",
            &serde_json::json!({"handle":reference,"area":"content","x":0,"y":200}),
        )
        .unwrap();
        render(&mut app, &ctx);
        render(&mut app, &ctx);
        let registry = app.control_windows.borrow();
        assert!(registry.lookup(&comment).unwrap().focus.is_none());
        let control_id = registry
            .lookup(&comment)
            .unwrap()
            .egui_id()
            .with(registry.generation)
            .with("comment");
        assert_eq!(ctx.memory(|m| m.focused()), Some(control_id));
        assert!(registry.lookup(&reference).unwrap().scroll.is_empty());
    }
}

pub(crate) fn focus_requested(name: &str) -> bool {
    #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
    if let Some(scope) = SCOPES.with(|s| s.borrow().last().cloned()) {
        return scope
            .registry
            .borrow()
            .lookup(&scope.handle)
            .is_some_and(|window| window.focus.as_deref() == Some(name));
    }
    let _ = name;
    false
}
