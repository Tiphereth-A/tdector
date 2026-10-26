//! Window identities and pending presentation requests. Drafts remain in typed UI state.
use super::protocol::ControlError;
use crate::enums::{CommentTarget, DictionaryPopupType, PinnedPopup};
use crate::ui::states::state::DecryptionApp;
use eframe::egui;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn word_comment_target(word: &str, target: &CommentTarget) -> Value {
    json!({
        "word": word,
        "comment_target": tdector_app::api::CommentTargetRecord::from(target.clone()),
    })
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct WindowGeometry {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub outer_position: [f32; 2],
    pub outer_size: [f32; 2],
    pub units: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct WindowInstance {
    pub handle: String,
    pub kind: String,
    pub target: Value,
    pub pinned: Option<u64>,
    pub resizable: bool,
    pub geometry: Option<WindowGeometry>,
    pub requested_position: Option<[f32; 2]>,
    pub requested_size: Option<[f32; 2]>,
    #[serde(skip)]
    pub explicit_position: bool,
    #[serde(skip)]
    pub focus: Option<String>,
    #[serde(skip)]
    pub scroll: BTreeMap<String, [f32; 2]>,
}
impl WindowInstance {
    pub fn egui_id(&self) -> egui::Id {
        egui::Id::unique(("tdector-window", &self.handle))
    }
}

pub(crate) struct WindowRegistry {
    pub generation: u64,
    pub rendered_frame: u64,
    pub windows: Vec<WindowInstance>,
    namespace: u128,
    next: u64,
}
impl Default for WindowRegistry {
    fn default() -> Self {
        static INSTANCE: AtomicU64 = AtomicU64::new(0);
        let clock = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut random = [0_u8; 16];
        let namespace = if getrandom::fill(&mut random).is_ok() {
            u128::from_le_bytes(random)
        } else {
            clock ^ u128::from(INSTANCE.fetch_add(1, Ordering::Relaxed))
        };
        let mut registry = Self {
            generation: 1,
            rendered_frame: 0,
            windows: vec![],
            namespace,
            next: 0,
        };
        registry.add("main", json!({}), None, true);
        registry
    }
}
impl WindowRegistry {
    pub fn lookup(&self, handle: &str) -> Option<&WindowInstance> {
        self.windows.iter().find(|w| w.handle == handle)
    }
    pub fn lookup_mut(&mut self, handle: &str) -> Option<&mut WindowInstance> {
        self.windows.iter_mut().find(|w| w.handle == handle)
    }
    pub fn find(&self, kind: &str, pinned: Option<u64>) -> Option<&WindowInstance> {
        self.windows
            .iter()
            .find(|w| w.kind == kind && w.pinned == pinned)
    }
    pub fn find_mut(&mut self, kind: &str, pinned: Option<u64>) -> Option<&mut WindowInstance> {
        self.windows
            .iter_mut()
            .find(|w| w.kind == kind && w.pinned == pinned)
    }
    pub fn main_handle(&self) -> &str {
        &self.windows[0].handle
    }
    pub fn invalidate_project(&mut self) {
        self.generation += 1;
        self.windows.retain(|w| w.kind == "main");
        if let Some(main) = self.windows.first_mut() {
            main.focus = None;
            main.scroll.clear();
        }
    }
    fn add(&mut self, kind: &str, target: Value, pinned: Option<u64>, resizable: bool) {
        let handle = format!("w-{:032x}-{:016x}", self.namespace, self.next);
        self.next += 1;
        self.windows.push(WindowInstance {
            handle,
            kind: kind.into(),
            target,
            pinned,
            resizable,
            geometry: None,
            requested_position: None,
            requested_size: None,
            explicit_position: false,
            focus: None,
            scroll: BTreeMap::new(),
        });
    }
    fn same_target(kind: &str, before: &Value, after: &Value) -> bool {
        if matches!(
            kind,
            "formation-chain" | "remove-formation-rule" | "word-menu"
        ) {
            // Token text can change while the same coordinate-addressed window stays open.
            before.get("segment") == after.get("segment")
                && before.get("token") == after.get("token")
        } else {
            before == after
        }
    }
    fn reconcile(&mut self, live: Vec<(&str, Value, Option<u64>, bool)>) {
        // Pinning changes the storage slot but preserves the existing instance.
        for (kind, target, pin, _) in &live {
            if pin.is_some()
                && self.find(kind, *pin).is_none()
                && !live.iter().any(|(k, _, p, _)| k == kind && p.is_none())
                && let Some(window) = self.windows.iter_mut().find(|w| {
                    w.kind == *kind
                        && Self::same_target(kind, &w.target, target)
                        && w.pinned.is_none()
                })
            {
                window.pinned = *pin;
            }
        }
        self.windows.retain(|w| {
            w.kind == "main"
                || live.iter().any(|(k, t, p, _)| {
                    w.kind == *k && Self::same_target(k, &w.target, t) && w.pinned == *p
                })
        });
        for (kind, target, pin, resizable) in live {
            if let Some(window) = self.find_mut(kind, pin) {
                window.target = target;
            } else {
                self.add(kind, target, pin, resizable);
            }
        }
    }
}

impl DecryptionApp {
    pub(crate) fn native_position_supported(&self) -> bool {
        self.control_data
            .native_position
            .unwrap_or_else(|| !super::api::is_wayland())
    }

    pub(crate) fn sync_control_windows(&mut self) {
        if self.confirmation.is_none() {
            self.control_data.pending = None;
        }
        let mut live = vec![];
        if let Some(word) = &self.definition_popup {
            live.push(("definition", json!({"word":word}), None, true));
        }
        if let Some(word) = &self.reference_popup {
            live.push(("references", json!({"word":word}), None, true));
        }
        if let Some((segment, _)) = &self.similar_popup {
            live.push(("similar-segments", json!({"segment":segment}), None, true));
        }
        if let Some((word, _)) = &self.similar_tokens_popup {
            live.push(("similar-tokens", json!({"word":word}), None, true));
        }
        if let Some((word, segment, token, _)) = &mut self.word_menu_popup {
            if let Some(current) = self
                .session
                .project()
                .segments
                .get(*segment)
                .and_then(|s| s.tokens.get(*token))
            {
                word.clone_from(&current.original);
            }
            live.push((
                "word-menu",
                json!({"word":word,"segment":segment,"token":token}),
                None,
                false,
            ));
        }
        if let Some((segment, _)) = &self.sentence_menu_popup {
            live.push(("segment-menu", json!({"segment":segment}), None, false));
        }
        if let Some(d) = &self.word_formation_popup {
            live.push((
                "word-formation",
                json!({"word":d.selected_word}),
                None,
                true,
            ));
        }
        if let Some(d) = &self.formatting_chain_popup {
            live.push(("formation-chain", json!({"segment":d.sentence_idx,"token":d.word_idx,"word":self.session.project().segments.get(d.sentence_idx).and_then(|s| s.tokens.get(d.word_idx)).map(|t|t.original.as_str()).unwrap_or_default()}), None, true));
        }
        if let Some(d) = &mut self.remove_formation_rule_popup {
            crate::ui::drafts::refresh_remove_formation(&self.session, d);
            live.push(("remove-formation-rule", json!({"segment":d.sentence_idx,"token":d.word_idx,"word":self.session.project().segments.get(d.sentence_idx).and_then(|s| s.tokens.get(d.word_idx)).map(|t|t.original.as_str()).unwrap_or_default()}), None, true));
        }
        if self.new_formation_rule_popup.is_some() {
            live.push(("new-formation-rule", json!({}), None, true));
        }
        if let Some(d) = &self.update_comment_popup {
            live.push((
                "word-comment",
                word_comment_target(&d.word, &d.target),
                None,
                true,
            ));
        }
        if let Some(d) = &self.update_sentence_comment_popup {
            live.push((
                "segment-comment",
                json!({"segment":d.segment_idx}),
                None,
                true,
            ));
        }
        if self.custom_tokenization_popup.is_some() {
            live.push(("custom-tokenization", json!({}), None, true));
        }
        if self.pending_import.is_some() {
            live.push(("import-options", json!({}), None, false));
        }
        if self.error_message.is_some() {
            live.push(("error", json!({}), None, false));
        }
        if self.confirmation.is_some() {
            live.push(("confirmation", json!({}), None, false));
        }
        for popup in &self.pinned_popups {
            match popup {
                PinnedPopup::Dictionary(word, mode, id, _) => live.push((
                    if *mode == DictionaryPopupType::Definition {
                        "definition"
                    } else {
                        "references"
                    },
                    json!({"word":word}),
                    Some(*id),
                    true,
                )),
                PinnedPopup::Similar(segment, _, id, _) => live.push((
                    "similar-segments",
                    json!({"segment":segment}),
                    Some(*id),
                    true,
                )),
            }
        }
        self.control_windows.borrow_mut().reconcile(live);
    }

    pub(crate) fn observe_control_frame(&mut self, ctx: &egui::Context) {
        self.sync_control_windows();
        let mut registry = self.control_windows.borrow_mut();
        registry.rendered_frame += 1;
        let (inner, outer) = ctx.input(|i| (i.viewport().inner_rect, i.viewport().outer_rect));
        if let Some(inner) = inner {
            let outer = outer.unwrap_or(inner);
            registry.windows[0].geometry = Some(WindowGeometry {
                position: outer.min.into(),
                size: inner.size().into(),
                outer_position: outer.min.into(),
                outer_size: outer.size().into(),
                units: "desktop-logical".into(),
            });
        }
    }

    pub(crate) fn control_geometry(
        &mut self,
        ctx: &egui::Context,
        command: &str,
        args: &Value,
    ) -> Result<Value, ControlError> {
        let fail = |code: &str, message: &str| ControlError {
            code: code.into(),
            message: message.into(),
            details: None,
            exit_code: if matches!(code, "invalid_arguments" | "unknown_command") {
                2
            } else {
                3
            },
        };
        let handle = args
            .get("handle")
            .and_then(Value::as_str)
            .ok_or_else(|| fail("invalid_arguments", "A window handle is required"))?;
        let mut registry = self.control_windows.borrow_mut();
        let generation = registry.generation;
        let window = registry.lookup_mut(handle).ok_or_else(|| {
            fail(
                "stale_handle",
                "This window handle is closed or belongs to another session",
            )
        })?;
        let pair = |a: &str, b: &str| -> Result<[f32; 2], ControlError> {
            let number = |key: &str| {
                args.get(key)
                    .and_then(Value::as_f64)
                    .filter(|n| n.is_finite() && n.abs() <= f32::MAX as f64)
                    .map(|n| n as f32)
                    .ok_or_else(|| {
                        fail(
                            "invalid_value",
                            "Geometry coordinates must be finite numbers",
                        )
                    })
            };
            Ok([number(a)?, number(b)?])
        };
        match command {
            "move" => {
                let pos = pair("x", "y")?;
                if window.kind == "main" {
                    if !self.native_position_supported() {
                        return Err(fail(
                            "unsupported_operation",
                            "Native window positioning is unavailable on Wayland",
                        ));
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos.into()));
                } else {
                    window.requested_position = Some(pos);
                    window.explicit_position = true;
                }
            }
            "resize" => {
                if !window.resizable {
                    return Err(fail(
                        "unsupported_operation",
                        "This window is not resizable",
                    ));
                }
                let size = pair("width", "height")?;
                if size.iter().any(|n| *n <= 0.0) {
                    return Err(fail("invalid_value", "Window dimensions must be positive"));
                }
                if window.kind == "main" {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size.into()));
                } else {
                    window.requested_size = Some(size);
                }
            }
            "focus" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                let name = args.get("control").and_then(Value::as_str).unwrap_or("");
                let prefix = format!("g{generation}:");
                let name = name.strip_prefix(&prefix).unwrap_or(name);
                window.focus = Some(name.into());
                let order = if matches!(window.kind.as_str(), "word-menu" | "segment-menu") {
                    egui::Order::Foreground
                } else {
                    egui::Order::Middle
                };
                ctx.move_to_top(egui::LayerId::new(order, window.egui_id()));
            }
            "scroll" => {
                let area = args
                    .get("area")
                    .and_then(Value::as_str)
                    .ok_or_else(|| fail("invalid_arguments", "A scroll area is required"))?;
                let prefix = format!("g{generation}:");
                let area = area.strip_prefix(&prefix).unwrap_or(area);
                if area != "main" && area != "content" && !area.starts_with("segment:") {
                    return Err(fail("unknown_control", "Unknown scroll area"));
                }
                let offset = pair("x", "y")?;
                if offset.iter().any(|n| *n < 0.0) {
                    return Err(fail("invalid_value", "Scroll offsets cannot be negative"));
                }
                window.scroll.insert(area.into(), offset);
            }
            _ => return Err(fail("unknown_command", "Unknown geometry operation")),
        }
        ctx.request_repaint();
        Ok(
            json!({"handle":handle,"geometry":window.geometry,"requested_position":window.requested_position,"requested_size":window.requested_size,"render_observed":false}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_survive_pinning_but_not_close_or_project_replacement() {
        let mut registry = WindowRegistry::default();
        let main = registry.main_handle().to_owned();
        registry.reconcile(vec![("definition", json!({"word":"cat"}), None, true)]);
        let handle = registry.find("definition", None).unwrap().handle.clone();
        registry.reconcile(vec![("definition", json!({"word":"cat"}), Some(5), true)]);
        assert_eq!(registry.find("definition", Some(5)).unwrap().handle, handle);
        registry.reconcile(vec![]);
        assert!(registry.lookup(&handle).is_none());
        registry.reconcile(vec![("definition", json!({"word":"cat"}), None, true)]);
        assert_ne!(registry.find("definition", None).unwrap().handle, handle);
        registry.invalidate_project();
        assert_eq!(registry.main_handle(), main);
        assert_eq!(registry.windows.len(), 1);
        assert_eq!(registry.generation, 2);
    }
    #[test]
    fn token_target_metadata_changes_preserve_the_live_instance() {
        for kind in ["formation-chain", "remove-formation-rule", "word-menu"] {
            let mut registry = WindowRegistry::default();
            registry.reconcile(vec![(
                kind,
                json!({"segment":2,"token":3,"word":"cats"}),
                None,
                true,
            )]);
            let handle = registry.find(kind, None).unwrap().handle.clone();
            registry.reconcile(vec![(
                kind,
                json!({"segment":2,"token":3,"word":"cat"}),
                None,
                true,
            )]);
            assert_eq!(registry.find(kind, None).unwrap().handle, handle);
            assert_eq!(registry.lookup(&handle).unwrap().target["word"], "cat");
            registry.reconcile(vec![(
                kind,
                json!({"segment":2,"token":4,"word":"cat"}),
                None,
                true,
            )]);
            assert!(registry.lookup(&handle).is_none());
        }
    }
    #[test]
    fn backend_capability_controls_native_move_and_exit_status() {
        let mut app = DecryptionApp::default();
        let ctx = egui::Context::default();
        let handle = app.control_windows.borrow().main_handle().to_owned();
        app.control_data.native_position = Some(false);
        let error = app
            .control_geometry(&ctx, "move", &json!({"handle":handle,"x":1,"y":2}))
            .unwrap_err();
        assert_eq!(error.code, "unsupported_operation");
        assert_eq!(error.exit_code, 3);
        app.control_data.native_position = Some(true);
        assert!(
            app.control_geometry(&ctx, "move", &json!({"handle":handle,"x":1,"y":2}))
                .is_ok()
        );
    }
    #[test]
    fn separate_instances_have_distinct_handles() {
        assert_ne!(
            WindowRegistry::default().main_handle(),
            WindowRegistry::default().main_handle()
        );
    }
}
