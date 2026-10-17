//! Semantic bindings between the control protocol and the live GUI state.

use std::path::{Path, PathBuf};

use egui::Context;
use serde_json::{Value, json};
use tdector_app::{Command, PreparedLoad};
use tdector_eval::TokenizationRule;

use super::protocol::{ControlError, Result};
use crate::enums::{AppAction, DictionaryPopupType, PinnedPopup};
use crate::ui::states::state::*;

#[path = "bindings.rs"]
mod bindings;

/// Native I/O state belongs to this GUI adapter, never to the domain session.
pub(crate) struct ControlData {
    pub(crate) source: Option<(PathBuf, Vec<u8>)>,
    pub(crate) pending: Option<PendingOperation>,
    pub(crate) jump_page: usize,
    pub(crate) native_position: Option<bool>,
}

impl Default for ControlData {
    fn default() -> Self {
        Self {
            source: None,
            pending: None,
            jump_page: 1,
            native_position: None,
        }
    }
}

pub(crate) enum PendingOperation {
    Load {
        prepared: PreparedLoad,
        path: PathBuf,
        bytes: Vec<u8>,
    },
    Import {
        content: String,
        name: String,
    },
}

pub(super) fn failure(code: &str, message: impl Into<String>, exit_code: u8) -> ControlError {
    ControlError {
        code: code.into(),
        message: message.into(),
        details: None,
        exit_code,
    }
}

pub(super) fn invalid(message: impl Into<String>) -> ControlError {
    failure("invalid_input", message, 3)
}

pub(super) fn app_error(error: tdector_app::Error) -> ControlError {
    use tdector_app::Error;
    let (code, exit) = match &error {
        Error::InvalidInput(_) => ("invalid_input", 3),
        Error::InvalidIndex { .. } => ("invalid_index", 3),
        Error::Operation(tdector_eval::AppError::InvalidProjectFormat(_)) => ("invalid_project", 3),
        Error::Operation(tdector_eval::AppError::IoError(_)) => ("io_error", 5),
        Error::Operation(_) => ("script_error", 4),
    };
    failure(code, error.to_string(), exit)
}

fn io_error(error: tdector_io::IoError) -> ControlError {
    use tdector_io::IoError;
    let (code, exit) = match &error {
        IoError::OutputExists { .. } => ("output_exists", 6),
        IoError::InputChanged { .. } => ("input_changed", 6),
        IoError::InvalidUtf8(_) | IoError::InvalidPath { .. } => ("invalid_input", 3),
        IoError::LimitExceeded { .. } => ("limit_exceeded", 3),
        IoError::CommittedState { .. } => ("committed_state_error", 1),
        IoError::Io(_) => ("io_error", 5),
    };
    failure(code, error.to_string(), exit)
}

pub(super) fn string<'a>(args: &'a Value, field: &str) -> Result<&'a str> {
    args.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} must be a string")))
}

pub(super) fn index(args: &Value, field: &str) -> Result<usize> {
    args.get(field)
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| invalid(format!("{field} must be a non-negative integer")))
}

fn already_open(handle: &str) -> ControlError {
    let mut error = failure(
        "already_open",
        "A different target already occupies this dialog; close or pin it first",
        3,
    );
    error.details = Some(json!({"handle": handle}));
    error
}

impl DecryptionApp {
    /// Called only by the native host's main-thread logic hook.
    pub fn control_execute(&mut self, ctx: &Context, command: &str, args: Value) -> Result<Value> {
        self.sync_control_windows();
        let result = match command {
            "status" => Ok(json!({
                "project_name": self.session.project().project_name,
                "dirty": self.session.is_dirty(), "revision": self.session.revision(),
                "generation": self.control_windows.borrow().generation,
                "main_handle": self.control_windows.borrow().main_handle(),
                "font": self.custom_font_name,
                "capabilities": {"native_position": self.native_position_supported(), "internal_position": true,
                    "direct_files": true, "render_barrier": true, "renderer_screenshot": true}
            })),
            "windows" => {
                let windows = self.control_windows.borrow().windows.iter().map(|w| {
                    json!({"handle": w.handle, "kind": w.kind, "target": w.target,
                        "pinned": w.pinned.is_some(), "resizable": w.resizable, "geometry": w.geometry})
                }).collect::<Vec<_>>();
                Ok(json!({"windows": windows}))
            }
            "open" => self.control_open(ctx, &args),
            "inspect" => self.control_inspect(ctx, string(&args, "handle")?),
            "get" => {
                let handle = string(&args, "handle")?;
                let control = string(&args, "control")?;
                let inspected = self.control_inspect(ctx, handle)?;
                let field = inspected["controls"]
                    .as_array()
                    .and_then(|cs| {
                        cs.iter()
                            .find(|c| c["id"] == control || c["name"] == control)
                    })
                    .ok_or_else(|| {
                        failure(
                            "unknown_control",
                            "Control is not advertised by this window",
                            3,
                        )
                    })?;
                Ok(json!({"handle": handle, "control": field["id"], "value": field["value"]}))
            }
            "set" => self.control_set(ctx, &args),
            "invoke" => self.control_invoke(ctx, &args),
            "focus" | "scroll" => {
                let inspected = self.control_inspect(ctx, string(&args, "handle")?)?;
                if command == "focus" {
                    if let Some(control) = args.get("control").and_then(Value::as_str) {
                        let field = inspected["controls"]
                            .as_array()
                            .and_then(|cs| {
                                cs.iter()
                                    .find(|c| c["id"] == control || c["name"] == control)
                            })
                            .ok_or_else(|| {
                                failure(
                                    "unknown_control",
                                    "Control is stale or not advertised by this window",
                                    3,
                                )
                            })?;
                        if field["read_only"] == true || field["disabled"] == true {
                            return Err(failure(
                                "action_disabled",
                                "This control cannot receive editing focus",
                                3,
                            ));
                        }
                    }
                } else {
                    let area = string(&args, "area")?;
                    if !inspected["scroll_areas"]
                        .as_array()
                        .is_some_and(|areas| areas.iter().any(|a| a["id"] == area))
                    {
                        return Err(failure(
                            "unknown_control",
                            "Scroll area is stale or not advertised by this window",
                            3,
                        ));
                    }
                }
                self.control_geometry(ctx, command, &args)
            }
            "move" | "resize" => self.control_geometry(ctx, command, &args),
            "close" => self.control_close(ctx, string(&args, "handle")?),
            "load" | "import" => self.control_read(ctx, command, &args),
            "font" => self.control_font(ctx, Path::new(string(&args, "path")?)),
            "save" => self.control_save(ctx, &args),
            "export" => self.control_export(&args),
            "quit" => self.control_quit(ctx, args["discard"].as_bool().unwrap_or(false)),
            "wait" => Ok(json!({"rendered_frame": self.control_windows.borrow().rendered_frame})),
            _ => Err(failure(
                "unknown_command",
                format!("Unknown GUI command: {command}"),
                2,
            )),
        };
        let mut result = result;
        if let Err(error) = &result
            && matches!(
                error.code.as_str(),
                "script_error" | "invalid_project" | "io_error" | "input_changed" | "output_exists"
            )
        {
            self.error_message = Some(error.message.clone());
        }
        self.sync_control_windows();
        if let Err(error) = &mut result
            && self.error_message.as_deref() == Some(error.message.as_str())
            && let Some(window) = self.control_windows.borrow().find("error", None)
        {
            let details = error.details.get_or_insert_with(|| json!({}));
            if let Some(details) = details.as_object_mut() {
                details.insert("dialog_handle".into(), json!(window.handle));
            }
        }
        ctx.request_repaint();
        result
    }

    fn token_coordinates(&self, args: &Value) -> Result<(usize, usize)> {
        let s = index(args, "segment")?;
        let t = index(args, "token")?;
        let token = self
            .session
            .project()
            .segments
            .get(s)
            .and_then(|s| s.tokens.get(t))
            .ok_or_else(|| failure("invalid_index", "Segment/token coordinates do not exist", 3))?;
        if args
            .get("word")
            .and_then(Value::as_str)
            .is_some_and(|word| word != token.original)
        {
            return Err(invalid(
                "The supplied word does not match the token at these coordinates",
            ));
        }
        Ok((s, t))
    }

    pub(crate) fn control_open(&mut self, ctx: &Context, args: &Value) -> Result<Value> {
        let kind = string(args, "kind")?;
        let pinned = args["pinned"].as_bool().unwrap_or(false);
        if pinned && !matches!(kind, "definition" | "references" | "similar-segments") {
            return Err(failure(
                "unsupported_operation",
                "This window cannot be pinned",
                3,
            ));
        }
        // Validate an explicit token before singleton reuse; word-only callers keep
        // their existing first-occurrence fallback when creating a new draft.
        let formation_coordinates = if kind == "word-formation"
            && (!args["segment"].is_null() || !args["token"].is_null())
        {
            Some(self.token_coordinates(args)?)
        } else {
            None
        };
        let target = match kind {
            "definition" | "references" | "similar-tokens" | "word-formation" => {
                let word = string(args, "word")?;
                if !self
                    .session
                    .project()
                    .segments
                    .iter()
                    .any(|s| s.tokens.iter().any(|t| t.original == word))
                    && !self.session.project().vocabulary.contains_key(word)
                {
                    return Err(invalid("Word is not present in the current project"));
                }
                json!({"word": word})
            }
            "similar-segments" | "segment-menu" | "segment-comment" => {
                let segment = index(args, "segment")?;
                if segment >= self.session.project().segments.len() {
                    return Err(failure("invalid_index", "Segment does not exist", 3));
                }
                json!({"segment": segment})
            }
            "word-menu" | "formation-chain" | "remove-formation-rule" | "word-comment" => {
                let (s, t) = self.token_coordinates(args)?;
                let word = &self.session.project().segments[s].tokens[t].original;
                if kind == "word-comment" {
                    let (comment_target, _) =
                        self.session.token_comment(s, t).map_err(app_error)?;
                    super::lifecycle::word_comment_target(word, &comment_target)
                } else {
                    json!({"segment": s, "token": t, "word": word})
                }
            }
            "new-formation-rule" => json!({}),
            _ => {
                return Err(invalid(
                    "Unsupported popup kind; operation dialogs are opened by their corresponding command",
                ));
            }
        };
        if pinned {
            let existing = self
                .control_windows
                .borrow()
                .windows
                .iter()
                .find(|w| w.kind == kind && w.target == target && w.pinned.is_none())
                .map(|w| w.handle.clone());
            if let Some(handle) = existing {
                self.control_invoke(ctx, &json!({"handle":handle,"action":"pin"}))?;
                return Ok(json!({"handle":handle,"kind":kind,"reused":true}));
            }
        }
        {
            let windows = self.control_windows.borrow();
            if let Some(existing) = windows.windows.iter().find(|w| {
                w.kind == kind && w.target == target && (w.pinned.is_some() == pinned || !pinned)
            }) {
                return Ok(json!({"handle": existing.handle, "kind": kind, "reused": true}));
            }
            if !pinned
                && let Some(existing) = windows
                    .windows
                    .iter()
                    .find(|w| w.kind == kind && w.pinned.is_none())
            {
                return Err(already_open(&existing.handle));
            }
        }
        let word = target["word"].as_str().unwrap_or_default().to_owned();
        let s = target["segment"].as_u64().unwrap_or_default() as usize;
        if pinned {
            let id = self.next_popup_id;
            self.next_popup_id += 1;
            let popup = match kind {
                "similar-segments" => PinnedPopup::Similar(
                    s,
                    self.session
                        .similar_segments(s, crate::consts::domain::DEFAULT_SIMILARITY_RESULTS)
                        .map_err(app_error)?,
                    id,
                    format!("📌 Similar to [{}]", s + 1),
                ),
                _ => PinnedPopup::Dictionary(
                    word.clone(),
                    if kind == "definition" {
                        DictionaryPopupType::Definition
                    } else {
                        DictionaryPopupType::Reference
                    },
                    id,
                    crate::ui::popup_utils::create_pinned_title_string(
                        if kind == "definition" {
                            "📌 Definition: "
                        } else {
                            "📌 References: "
                        },
                        &word,
                        self.custom_font_name.is_some(),
                    ),
                ),
            };
            self.pinned_popups.push(popup);
        } else {
            match kind {
                "definition" => self.definition_popup = Some(word),
                "references" => self.reference_popup = Some(word),
                "similar-segments" => self.compute_similar_segments(s),
                "similar-tokens" => {
                    self.similar_tokens_popup =
                        Some((word.clone(), self.session.similar_tokens(&word)))
                }
                "word-menu" => {
                    let (s, t) = self.token_coordinates(args)?;
                    self.word_menu_popup = Some((word, s, t, egui::pos2(20.0, 20.0)));
                }
                "segment-menu" => self.sentence_menu_popup = Some((s, egui::pos2(20.0, 20.0))),
                "formation-chain" => {
                    let (s, t) = self.token_coordinates(args)?;
                    self.formatting_chain_popup = Some(FormattingChainDialog {
                        sentence_idx: s,
                        word_idx: t,
                    });
                }
                "remove-formation-rule" => {
                    let (s, t) = self.token_coordinates(args)?;
                    let token = &self.session.project().segments[s].tokens[t];
                    let rule = token
                        .formation_rule_indices
                        .last()
                        .and_then(|i| self.session.project().formation_rules.get(*i))
                        .ok_or_else(|| {
                            failure(
                                "action_disabled",
                                "Token has no formation rule to remove",
                                3,
                            )
                        })?;
                    self.remove_formation_rule_popup = Some(RemoveFormationRuleDialog {
                        sentence_idx: s,
                        word_idx: t,
                        formatted_word: word.clone(),
                        base_word: token.base_word.clone().unwrap_or(word),
                        rule_description: rule.description.clone(),
                    });
                }
                "word-comment" => {
                    let (s, t) = self.token_coordinates(args)?;
                    let (target, comment) = self.session.token_comment(s, t).map_err(app_error)?;
                    self.update_comment_popup = Some(UpdateCommentDialog {
                        word,
                        comment,
                        target,
                    });
                }
                "segment-comment" => {
                    self.update_sentence_comment_popup = Some(UpdateSentenceCommentDialog {
                        segment_idx: s,
                        comment: self.session.project().segments[s].comment.clone(),
                    })
                }
                "word-formation" => {
                    let token = if let Some((s, t)) = formation_coordinates {
                        Some(&self.session.project().segments[s].tokens[t])
                    } else {
                        self.session
                            .project()
                            .segments
                            .iter()
                            .flat_map(|s| &s.tokens)
                            .find(|t| t.original == word)
                    };
                    self.word_formation_popup = Some(WordFormationDialog {
                        selected_word: word,
                        base_word: token.and_then(|t| t.base_word.clone()).unwrap_or_default(),
                        selected_rule: token.and_then(|t| t.formation_rule_indices.last().copied()),
                        preview: String::new(),
                        related_words: Vec::new(),
                        rule_search_text: String::new(),
                    });
                    self.control_refresh_previews();
                }
                "new-formation-rule" => {
                    self.new_formation_rule_popup = Some(NewFormationRuleDialog {
                        description: String::new(),
                        rule_type: tdector_eval::FormationType::Derivation,
                        command: "fn transform(word) { word }".into(),
                        test_word: String::new(),
                        preview: String::new(),
                    })
                }
                _ => unreachable!(),
            }
        }
        self.sync_control_windows();
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        let windows = self.control_windows.borrow();
        let window = windows
            .windows
            .iter()
            .rev()
            .find(|w| w.kind == kind && w.target == target && w.pinned.is_some() == pinned)
            .ok_or_else(|| failure("internal_error", "Opened window was not registered", 1))?;
        Ok(json!({"handle":window.handle,"kind":kind,"reused":false,"geometry":window.geometry}))
    }

    fn control_close(&mut self, ctx: &Context, handle: &str) -> Result<Value> {
        let window = self
            .control_windows
            .borrow()
            .lookup(handle)
            .cloned()
            .ok_or_else(|| {
                failure(
                    "stale_handle",
                    "Window handle is closed, invalid, or belongs to another session",
                    3,
                )
            })?;
        if window.kind == "main" {
            return self.control_quit(ctx, false);
        }
        if let Some(id) = window.pinned {
            self.pinned_popups.retain(|p| match p {
                PinnedPopup::Dictionary(_, _, p, _) | PinnedPopup::Similar(_, _, p, _) => *p != id,
            });
        } else {
            match window.kind.as_str() {
                "definition" => self.definition_popup = None,
                "references" => self.reference_popup = None,
                "similar-segments" => self.similar_popup = None,
                "similar-tokens" => self.similar_tokens_popup = None,
                "word-menu" => self.word_menu_popup = None,
                "segment-menu" => self.sentence_menu_popup = None,
                "word-formation" => self.word_formation_popup = None,
                "formation-chain" => self.formatting_chain_popup = None,
                "remove-formation-rule" => self.remove_formation_rule_popup = None,
                "new-formation-rule" => self.new_formation_rule_popup = None,
                "word-comment" => self.update_comment_popup = None,
                "segment-comment" => self.update_sentence_comment_popup = None,
                "custom-tokenization" => self.custom_tokenization_popup = None,
                "import-options" => self.pending_import = None,
                "error" => self.error_message = None,
                "confirmation" => {
                    self.confirmation = None;
                    self.control_data.pending = None;
                }
                _ => return Err(invalid("Unsupported window kind")),
            }
        }
        self.sync_control_windows();
        Ok(json!({"handle":handle,"closed":true,"main":false}))
    }

    fn control_quit(&mut self, ctx: &Context, discard: bool) -> Result<Value> {
        if self.session.is_dirty() && !discard {
            if let Some(w) = self
                .control_windows
                .borrow()
                .windows
                .iter()
                .find(|w| w.kind == "confirmation")
            {
                return Err(already_open(&w.handle));
            }
            self.trigger_action(AppAction::Quit, ctx);
            self.sync_control_windows();
            return Ok(
                json!({"closed":false,"handle":self.control_windows.borrow().find("confirmation",None).map(|w| w.handle.clone())}),
            );
        }
        self.closing = true;
        self.unsaved_changes.set(false);
        Ok(json!({"closed":true,"main":true}))
    }

    fn control_read(&mut self, ctx: &Context, command: &str, args: &Value) -> Result<Value> {
        let path = PathBuf::from(string(args, "path")?);
        let bytes = tdector_io::read_file(&path).map_err(io_error)?;
        let content = tdector_io::decode_utf8(&bytes).map_err(io_error)?;
        let pending =
            if command == "load" {
                PendingOperation::Load {
                    prepared: self.session.prepare_load_json(content).map_err(app_error)?,
                    path,
                    bytes,
                }
            } else {
                if let Some(w) =
                    self.control_windows.borrow().windows.iter().find(|w| {
                        matches!(w.kind.as_str(), "import-options" | "custom-tokenization")
                    })
                {
                    return Err(already_open(&w.handle));
                }
                PendingOperation::Import {
                    content: content.to_owned(),
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                }
            };
        if self.session.is_dirty() {
            if let Some(w) = self
                .control_windows
                .borrow()
                .windows
                .iter()
                .find(|w| w.kind == "confirmation")
            {
                return Err(already_open(&w.handle));
            }
            let action = if command == "load" {
                AppAction::Open
            } else {
                AppAction::Import
            };
            self.control_data.pending = Some(pending);
            self.confirmation = Some((
                "You have unsaved changes. Continue with the requested file?".into(),
                action,
            ));
            self.sync_control_windows();
            return Ok(
                json!({"applied":false,"handle":self.control_windows.borrow().find("confirmation",None).map(|w| w.handle.clone())}),
            );
        }
        self.control_commit_read(ctx, pending)
    }

    fn control_commit_read(&mut self, ctx: &Context, pending: PendingOperation) -> Result<Value> {
        match pending {
            PendingOperation::Load {
                prepared,
                path,
                bytes,
            } => {
                self.session.commit_load(prepared).map_err(app_error)?;
                self.reset_project_view(ctx);
                self.project_filename = Some(path.to_string_lossy().into_owned());
                self.control_data.source = Some((path, bytes));
                self.sync_control_windows();
                Ok(json!({"loaded":true,"generation":self.control_windows.borrow().generation}))
            }
            PendingOperation::Import { content, name } => {
                self.pending_import = Some((content, name));
                self.sync_control_windows();
                Ok(
                    json!({"handle":self.control_windows.borrow().find("import-options",None).map(|w| w.handle.clone()),"kind":"import-options"}),
                )
            }
        }
    }

    /// Called by the ordinary confirmation buttons too; no OS picker replaces a pending path operation.
    pub(crate) fn execute_control_pending(&mut self, action: AppAction, ctx: &Context) -> bool {
        let matches = matches!(
            (&self.control_data.pending, action),
            (Some(PendingOperation::Load { .. }), AppAction::Open)
                | (Some(PendingOperation::Import { .. }), AppAction::Import)
        );
        if matches && let Some(pending) = self.control_data.pending.take() {
            if let Err(error) = self.control_commit_read(ctx, pending) {
                self.error_message = Some(error.message);
            }
            true
        } else {
            false
        }
    }

    fn control_font(&mut self, ctx: &Context, path: &Path) -> Result<Value> {
        let bytes = tdector_io::read_file(path).map_err(io_error)?;
        ttf_parser::Face::parse(&bytes, 0).map_err(|e| invalid(format!("Invalid font: {e:?}")))?;
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        self.load_custom_font_from_bytes(ctx, bytes, &name);
        Ok(json!({"font":name,"loaded":true}))
    }

    pub(crate) fn control_save(&mut self, ctx: &Context, args: &Value) -> Result<Value> {
        let output = args
            .get("output")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let bound = self.control_data.source.as_ref();
        let path = output
            .clone()
            .or_else(|| bound.map(|(p, _)| p.clone()))
            .ok_or_else(|| invalid("Saving this project requires --output FILE"))?;
        let snapshot = self.session.save_snapshot().map_err(app_error)?;
        let expected = match bound {
            Some((source, bytes))
                if source == &path || tdector_io::same_file(source, &path).map_err(io_error)? =>
            {
                Some((source.as_path(), bytes.as_slice()))
            }
            _ => None,
        };
        let overwrite = output.is_none() || args["overwrite"].as_bool().unwrap_or(false);
        tdector_io::atomic_write(&path, &snapshot.bytes, overwrite, expected).map_err(io_error)?;
        let token = format!("{:?}", snapshot.token);
        if !self.session.acknowledge_saved(snapshot.token) {
            return Err(failure(
                "internal_error",
                "Saved snapshot no longer matches the live session",
                1,
            ));
        }
        self.project_filename = Some(path.to_string_lossy().into_owned());
        self.control_data.source = Some((path.clone(), snapshot.bytes));
        self.update_title(ctx);
        Ok(json!({"saved":true,"output":path,"save_token":token,"dirty":self.session.is_dirty()}))
    }

    fn control_export(&mut self, args: &Value) -> Result<Value> {
        if args
            .get("format")
            .and_then(Value::as_str)
            .is_some_and(|f| f != "typst")
        {
            return Err(invalid("Only Typst export is supported"));
        }
        let path = Path::new(string(args, "output")?);
        if let Some((source, _)) = &self.control_data.source
            && (source == path || tdector_io::same_file(source, path).map_err(io_error)?)
        {
            return Err(invalid("Export destination refers to the loaded project"));
        }
        tdector_io::atomic_write(
            path,
            self.session.export_typst().as_bytes(),
            args["overwrite"].as_bool().unwrap_or(false),
            None,
        )
        .map_err(io_error)?;
        Ok(json!({"exported":true,"format":"typst","output":path}))
    }

    fn control_apply(&mut self, ctx: &Context, command: Command) -> Result<()> {
        self.execute_gui_command(command, ctx)
            .map(|_| ())
            .map_err(app_error)
    }
}

pub(crate) fn is_wayland() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
