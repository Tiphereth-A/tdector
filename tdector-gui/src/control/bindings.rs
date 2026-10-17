//! Typed control discovery and draft operations. The drafts are the UI's own state.

use super::*;
use tdector_app::api::{self, Pagination, Query};
use tdector_core::enums::SortMode;
use tdector_eval::FormationType;

const SORTS: [&str; 12] = [
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
];

fn field(id: String, name: &str, kind: &str, value: Value, commit: &str, choices: Value) -> Value {
    json!({"id":id,"name":name,"type":kind,"value":value,"read_only":commit=="read-only","disabled":false,"commit":commit,"choices":choices})
}

fn action(name: &str, enabled: bool) -> Value {
    json!({"id":name,"enabled":enabled})
}

fn formation_type(kind: FormationType) -> &'static str {
    match kind {
        FormationType::Derivation => "derivation",
        FormationType::Inflection => "inflection",
        FormationType::Nonmorphological => "nonmorphological",
    }
}

impl DecryptionApp {
    pub(super) fn control_inspect(&mut self, ctx: &Context, handle: &str) -> Result<Value> {
        let window = self
            .control_windows
            .borrow()
            .lookup(handle)
            .cloned()
            .ok_or_else(|| {
                failure(
                    "stale_handle",
                    "Window handle is no longer valid in this session",
                    3,
                )
            })?;
        let generation = self.control_windows.borrow().generation;
        let mut controls = Vec::new();
        let mut actions = Vec::new();
        let mut results = Value::Null;
        let mut displayed = Vec::new();
        let scoped = |name: &str| format!("g{generation}:{name}");
        let mut add = |name: &str, kind: &str, value: Value, commit: &str, choices: Value| {
            let id = if window.kind == "main" {
                name.to_owned()
            } else {
                scoped(name)
            };
            controls.push(field(id, name, kind, value, commit, choices));
        };
        match window.kind.as_str() {
            "main" => {
                add(
                    "filter",
                    "text",
                    json!(self.filter_text),
                    "immediate",
                    Value::Null,
                );
                let sort = SORTS[SortMode::all()
                    .iter()
                    .position(|m| *m == self.sort_mode)
                    .unwrap_or(0)];
                add("sort", "enum", json!(sort), "immediate", json!(SORTS));
                add(
                    "page-size",
                    "integer",
                    json!(self.page_size),
                    "immediate",
                    json!([10, 20, 50, 100]),
                );
                add(
                    "page",
                    "integer",
                    json!(self.current_page),
                    "immediate",
                    Value::Null,
                );
                add(
                    "jump",
                    "integer",
                    json!(self.control_data.jump_page),
                    "draft",
                    Value::Null,
                );
                add(
                    "theme",
                    "enum",
                    json!(if ctx.theme() == egui::Theme::Dark {
                        "dark"
                    } else {
                        "light"
                    }),
                    "immediate",
                    json!(["light", "dark"]),
                );
                let indices = self
                    .session
                    .filtered_indices(&self.filter_text, self.sort_mode);
                let pages = indices.len().div_ceil(self.page_size).max(1);
                actions.extend([
                    action(
                        "jump",
                        self.control_data.jump_page >= 1 && self.control_data.jump_page <= pages,
                    ),
                    action("next-page", self.current_page + 1 < pages),
                    action("previous-page", self.current_page > 0),
                    action("first-page", true),
                    action("last-page", true),
                ]);
                for &s in indices
                    .iter()
                    .skip(self.current_page * self.page_size)
                    .take(self.page_size)
                {
                    displayed.push(s);
                    let segment = &self.session.project().segments[s];
                    let id = format!("g{generation}:segment:{s}:translation");
                    controls.push(field(
                        id.clone(),
                        &id,
                        "text",
                        json!(segment.translation),
                        "immediate",
                        Value::Null,
                    ));
                    for (t, token) in segment.tokens.iter().enumerate() {
                        let a = tdector_app::annotate_token(self.session.project(), token);
                        let id = format!("g{generation}:segment:{s}:token:{t}:gloss");
                        let gloss = if a.formation_descriptions.is_empty() {
                            a.base_gloss.clone()
                        } else {
                            format!("{} ({})", a.base_gloss, a.formation_descriptions.join("; "))
                        };
                        controls.push(field(
                            id.clone(),
                            &id,
                            "text",
                            json!(gloss),
                            if a.is_derived {
                                "read-only"
                            } else {
                                "immediate"
                            },
                            Value::Null,
                        ));
                    }
                }
                let records = displayed
                    .iter()
                    .map(|&s| self.control_query(Query::SegmentShow { segment_index: s }))
                    .collect::<Result<Vec<_>>>()?;
                results =
                    json!({"segments":records,"total_filtered":indices.len(),"total_pages":pages});
            }
            "word-comment" => {
                if let Some(d) = &self.update_comment_popup {
                    add("comment", "text", json!(d.comment), "draft", Value::Null);
                    actions.extend([action("save", true), action("cancel", true)]);
                }
            }
            "segment-comment" => {
                if let Some(d) = &self.update_sentence_comment_popup {
                    add("comment", "text", json!(d.comment), "draft", Value::Null);
                    actions.extend([action("save", true), action("cancel", true)]);
                }
            }
            "word-formation" => {
                if let Some(d) = &self.word_formation_popup {
                    add(
                        "base-word",
                        "text",
                        json!(d.base_word),
                        "draft",
                        Value::Null,
                    );
                    add(
                        "rule-search",
                        "text",
                        json!(d.rule_search_text),
                        "draft",
                        Value::Null,
                    );
                    let choices = self
                        .session
                        .project()
                        .formation_rules
                        .iter()
                        .enumerate()
                        .map(|(i, r)| json!({"value":i,"label":r.description}))
                        .collect::<Vec<_>>();
                    add(
                        "rule",
                        "integer",
                        json!(d.selected_rule),
                        "draft",
                        json!(choices),
                    );
                    add(
                        "preview",
                        "text",
                        json!(d.preview),
                        "read-only",
                        Value::Null,
                    );
                    add(
                        "related-words",
                        "array",
                        json!(d.related_words),
                        "read-only",
                        Value::Null,
                    );
                    actions.extend([
                        action(
                            "apply",
                            d.selected_rule.is_some() && !d.base_word.is_empty(),
                        ),
                        action("cancel", true),
                    ]);
                }
            }
            "new-formation-rule" => {
                if let Some(d) = &self.new_formation_rule_popup {
                    add(
                        "description",
                        "text",
                        json!(d.description),
                        "draft",
                        Value::Null,
                    );
                    add(
                        "type",
                        "enum",
                        json!(formation_type(d.rule_type)),
                        "draft",
                        json!(["derivation", "inflection", "nonmorphological"]),
                    );
                    add("script", "text", json!(d.command), "draft", Value::Null);
                    add(
                        "test-word",
                        "text",
                        json!(d.test_word),
                        "draft",
                        Value::Null,
                    );
                    add(
                        "preview",
                        "text",
                        json!(d.preview),
                        "read-only",
                        Value::Null,
                    );
                    actions.extend([
                        action("create", !d.description.is_empty() && !d.command.is_empty()),
                        action("cancel", true),
                    ]);
                }
            }
            "custom-tokenization" => {
                if let Some(d) = &self.custom_tokenization_popup {
                    add("script", "text", json!(d.command), "draft", Value::Null);
                    add(
                        "test-text",
                        "text",
                        json!(d.test_text),
                        "draft",
                        Value::Null,
                    );
                    add(
                        "preview",
                        "array",
                        json!(d.preview),
                        "read-only",
                        Value::Null,
                    );
                    actions.extend([
                        action("test", true),
                        action("apply", true),
                        action("cancel", true),
                    ]);
                }
            }
            "import-options" => {
                add(
                    "name",
                    "text",
                    json!(self.pending_import.as_ref().map(|(_, n)| n)),
                    "read-only",
                    Value::Null,
                );
                actions.extend([
                    action("word-based", true),
                    action("character-based", true),
                    action("custom-script", true),
                    action("cancel", true),
                ]);
            }
            "confirmation" => {
                add(
                    "message",
                    "text",
                    json!(self.confirmation.as_ref().map(|(m, _)| m)),
                    "read-only",
                    Value::Null,
                );
                actions.extend([action("confirm", true), action("cancel", true)]);
            }
            "error" => {
                add(
                    "message",
                    "text",
                    json!(self.error_message),
                    "read-only",
                    Value::Null,
                );
                actions.push(action("ok", true));
            }
            "formation-chain" => {
                if let Some(d) = &self.formatting_chain_popup {
                    results = self.control_query(Query::FormationChain {
                        segment_index: d.sentence_idx,
                        token_index: d.word_idx,
                    })?;
                }
            }
            "remove-formation-rule" => {
                if let Some(d) = &mut self.remove_formation_rule_popup {
                    let can_remove = crate::ui::drafts::refresh_remove_formation(&self.session, d);
                    add(
                        "word",
                        "text",
                        json!(d.formatted_word),
                        "read-only",
                        Value::Null,
                    );
                    add(
                        "base-word",
                        "text",
                        json!(d.base_word),
                        "read-only",
                        Value::Null,
                    );
                    add(
                        "rule-description",
                        "text",
                        json!(d.rule_description),
                        "read-only",
                        Value::Null,
                    );
                    actions.extend([action("remove", can_remove), action("cancel", true)]);
                }
            }
            "definition" | "references" => {
                let word = window.target["word"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                results = self.control_query(Query::Lookup {
                    word,
                    kind: if window.kind == "definition" {
                        api::LookupKind::Headword
                    } else {
                        api::LookupKind::Usage
                    },
                    pagination: Pagination::all(),
                })?;
                actions.push(action("pin", window.pinned.is_none()));
            }
            "similar-segments" => {
                results = self.control_query(Query::SimilarSegments {
                    segment_index: window.target["segment"].as_u64().unwrap_or(0) as usize,
                    limit: crate::consts::domain::DEFAULT_SIMILARITY_RESULTS,
                })?;
                actions.push(action("pin", window.pinned.is_none()));
            }
            "similar-tokens" => {
                results = self.control_query(Query::SimilarTokens {
                    word: window.target["word"].as_str().unwrap_or_default().into(),
                    limit: tdector_core::consts::domain::MAX_SIMILAR_TOKENS_RESULTS,
                })?
            }
            "word-menu" => {
                let s = window.target["segment"].as_u64().unwrap_or(0) as usize;
                let t = window.target["token"].as_u64().unwrap_or(0) as usize;
                let derived = self
                    .session
                    .project()
                    .segments
                    .get(s)
                    .and_then(|s| s.tokens.get(t))
                    .is_some_and(|t| !t.formation_rule_indices.is_empty());
                for a in [
                    "definition",
                    "references",
                    "similar-tokens",
                    "word-formation",
                    "formation-chain",
                    "word-comment",
                ] {
                    actions.push(action(a, true));
                }
                actions.push(action("remove-formation-rule", derived));
            }
            "segment-menu" => actions.extend([
                action("similar-segments", true),
                action("segment-comment", true),
            ]),
            _ => {}
        }
        actions.push(action("close", true));
        let mut scroll_areas = Vec::new();
        match window.kind.as_str() {
            "main" => {
                scroll_areas.push(json!({"id":"main"}));
                for &s in &displayed {
                    scroll_areas.push(json!({"id":format!("g{generation}:segment:{s}")}));
                }
            }
            "definition" | "references" | "similar-segments" | "similar-tokens"
            | "formation-chain" => {
                scroll_areas.push(json!({"id":"content"}));
                if matches!(
                    window.kind.as_str(),
                    "definition" | "references" | "similar-segments"
                ) && let Some(items) = results["items"].as_array()
                {
                    for item in items {
                        if let Some(s) = item["segment_index"].as_u64() {
                            scroll_areas.push(json!({"id":format!("g{generation}:segment:{s}")}));
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(
            json!({"handle":window.handle,"kind":window.kind,"target":window.target,"pinned":window.pinned.is_some(),"resizable":window.resizable,"geometry":window.geometry,"generation":generation,"controls":controls,"actions":actions,"scroll_areas":scroll_areas,"results":results,"displayed_segments":displayed}),
        )
    }

    fn control_query(&mut self, query: Query) -> Result<Value> {
        let result = api::query(&mut self.session, query).map_err(|e| invalid(e.to_string()))?;
        serde_json::to_value(result).map_err(|e| failure("internal_error", e.to_string(), 1))
    }

    pub(super) fn control_set(&mut self, ctx: &Context, args: &Value) -> Result<Value> {
        let handle = string(args, "handle")?;
        let control = string(args, "control")?;
        let inspected = self.control_inspect(ctx, handle)?;
        let descriptor = inspected["controls"]
            .as_array()
            .and_then(|cs| {
                cs.iter()
                    .find(|c| c["id"] == control || c["name"] == control)
            })
            .ok_or_else(|| {
                failure(
                    "unknown_control",
                    "Control is stale or is not advertised by this window",
                    3,
                )
            })?;
        if descriptor["read_only"] == true {
            return Err(failure(
                "read_only_control",
                "This control displays a calculated or derived value",
                3,
            ));
        }
        let name = descriptor["name"].as_str().unwrap_or(control);
        let value = args
            .get("value")
            .ok_or_else(|| invalid("A value is required"))?;
        let text = || {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("This control requires text"))
        };
        let integer = || {
            value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .or_else(|| value.as_str().and_then(|s| s.parse::<usize>().ok()))
                .ok_or_else(|| invalid("This control requires a non-negative integer"))
        };
        let kind = inspected["kind"].as_str().unwrap_or_default();
        match kind {
            "main" => match name {
                "filter" => {
                    self.filter_text = text()?;
                    self.current_page = 0;
                    self.filter_dirty = true;
                }
                "sort" => {
                    let mode = text()?;
                    let i = SORTS
                        .iter()
                        .position(|m| *m == mode)
                        .ok_or_else(|| invalid("Unknown sort mode"))?;
                    self.sort_mode = SortMode::all()[i];
                    self.current_page = 0;
                    self.filter_dirty = true;
                }
                "page-size" => {
                    let size = integer()?;
                    if ![10, 20, 50, 100].contains(&size) {
                        return Err(invalid("Page size must be 10, 20, 50, or 100"));
                    }
                    self.page_size = size;
                    self.current_page = 0;
                }
                "page" => {
                    let page = integer()?;
                    let pages = self
                        .session
                        .filtered_indices(&self.filter_text, self.sort_mode)
                        .len()
                        .div_ceil(self.page_size)
                        .max(1);
                    if page >= pages {
                        return Err(invalid("Page is outside the filtered result set"));
                    }
                    self.current_page = page;
                }
                "jump" => {
                    let page = integer()?;
                    if page == 0 {
                        return Err(invalid("Jump uses one-based page numbers"));
                    }
                    self.control_data.jump_page = page;
                }
                "theme" => match text()?.as_str() {
                    "light" => ctx.set_theme(egui::Theme::Light),
                    "dark" => ctx.set_theme(egui::Theme::Dark),
                    _ => return Err(invalid("Theme must be light or dark")),
                },
                _ => {
                    let parts: Vec<_> = name.split(':').collect();
                    let generation = self.control_windows.borrow().generation;
                    if parts.first().copied() != Some(format!("g{generation}").as_str()) {
                        return Err(failure("unknown_control", "Project generation is stale", 3));
                    }
                    let s = parts
                        .get(2)
                        .and_then(|s| s.parse::<usize>().ok())
                        .ok_or_else(|| invalid("Invalid segment control"))?;
                    if parts.len() == 4 && parts[3] == "translation" {
                        self.control_apply(
                            ctx,
                            Command::SetTranslation {
                                segment: s,
                                translation: text()?,
                            },
                        )?;
                    } else if parts.len() == 6 && parts[5] == "gloss" {
                        let t = parts[4]
                            .parse::<usize>()
                            .map_err(|_| invalid("Invalid token control"))?;
                        let token = self
                            .session
                            .project()
                            .segments
                            .get(s)
                            .and_then(|s| s.tokens.get(t))
                            .ok_or_else(|| invalid("Invalid token"))?;
                        if !token.formation_rule_indices.is_empty() {
                            return Err(failure(
                                "read_only_control",
                                "Derived glosses cannot be edited",
                                3,
                            ));
                        }
                        let word = token
                            .base_word
                            .clone()
                            .unwrap_or_else(|| token.original.clone());
                        self.control_apply(
                            ctx,
                            Command::SetGloss {
                                word,
                                meaning: text()?,
                            },
                        )?;
                    } else {
                        return Err(invalid("Unknown project control"));
                    }
                }
            },
            "word-comment" => {
                self.update_comment_popup
                    .as_mut()
                    .ok_or_else(|| invalid("Closed draft"))?
                    .comment = text()?
            }
            "segment-comment" => {
                self.update_sentence_comment_popup
                    .as_mut()
                    .ok_or_else(|| invalid("Closed draft"))?
                    .comment = text()?
            }
            "word-formation" => {
                let rule_count = self.session.project().formation_rules.len();
                let d = self
                    .word_formation_popup
                    .as_mut()
                    .ok_or_else(|| invalid("Closed draft"))?;
                match name {
                    "base-word" => d.base_word = text()?,
                    "rule-search" => d.rule_search_text = text()?,
                    "rule" => {
                        let i = integer()?;
                        if i >= rule_count {
                            return Err(invalid("Formation rule index does not exist"));
                        }
                        d.selected_rule = Some(i);
                    }
                    _ => return Err(invalid("Unknown draft control")),
                }
            }
            "new-formation-rule" => {
                let d = self
                    .new_formation_rule_popup
                    .as_mut()
                    .ok_or_else(|| invalid("Closed draft"))?;
                match name {
                    "description" => d.description = text()?,
                    "script" => d.command = text()?,
                    "test-word" => d.test_word = text()?,
                    "type" => {
                        d.rule_type = match text()?.as_str() {
                            "derivation" => FormationType::Derivation,
                            "inflection" => FormationType::Inflection,
                            "nonmorphological" => FormationType::Nonmorphological,
                            _ => return Err(invalid("Unknown formation type")),
                        }
                    }
                    _ => return Err(invalid("Unknown draft control")),
                }
            }
            "custom-tokenization" => {
                let d = self
                    .custom_tokenization_popup
                    .as_mut()
                    .ok_or_else(|| invalid("Closed draft"))?;
                match name {
                    "script" => d.command = text()?,
                    "test-text" => d.test_text = text()?,
                    _ => return Err(invalid("Unknown draft control")),
                }
                d.preview.clear();
            }
            _ => {
                return Err(failure(
                    "read_only_control",
                    "This window has no editable controls",
                    3,
                ));
            }
        }
        self.control_refresh_previews();
        let updated = self.control_inspect(ctx, handle)?;
        let current = updated["controls"]
            .as_array()
            .and_then(|cs| cs.iter().find(|c| c["id"] == descriptor["id"]))
            .map(|c| c["value"].clone())
            .unwrap_or(Value::Null);
        Ok(
            json!({"handle":handle,"control":descriptor["id"],"value":current,"changed":current!=descriptor["value"],"dirty":self.session.is_dirty()}),
        )
    }

    pub(super) fn control_refresh_previews(&mut self) {
        if let Some(d) = &mut self.word_formation_popup {
            crate::ui::drafts::refresh_word_formation(&self.session, d);
        }
        if let Some(d) = &mut self.new_formation_rule_popup {
            crate::ui::drafts::refresh_new_formation(d);
        }
    }

    pub(super) fn control_invoke(&mut self, ctx: &Context, args: &Value) -> Result<Value> {
        let handle = string(args, "handle")?;
        let requested = string(args, "action")?;
        let inspected = self.control_inspect(ctx, handle)?;
        let advertised = inspected["actions"]
            .as_array()
            .and_then(|a| a.iter().find(|a| a["id"] == requested))
            .ok_or_else(|| {
                failure(
                    "unknown_action",
                    "Action is not advertised by this window",
                    3,
                )
            })?;
        if advertised["enabled"] != true {
            return Err(failure(
                "action_disabled",
                "The action is currently unavailable",
                3,
            ));
        }
        if matches!(requested, "close" | "cancel" | "ok") {
            return self.control_close(ctx, handle);
        }
        let kind = inspected["kind"].as_str().unwrap_or_default();
        match kind {
            "main" => {
                let pages = self
                    .session
                    .filtered_indices(&self.filter_text, self.sort_mode)
                    .len()
                    .div_ceil(self.page_size)
                    .max(1);
                self.current_page = match requested {
                    "jump" => self.control_data.jump_page - 1,
                    "next-page" => self.current_page + 1,
                    "previous-page" => self.current_page - 1,
                    "first-page" => 0,
                    "last-page" => pages - 1,
                    _ => return Err(invalid("Unknown navigation action")),
                };
            }
            "word-comment" => {
                let d = self
                    .update_comment_popup
                    .clone()
                    .ok_or_else(|| invalid("Closed draft"))?;
                self.control_apply(
                    ctx,
                    Command::SetWordComment {
                        target: d.target,
                        comment: d.comment,
                    },
                )?;
                self.update_comment_popup = None;
            }
            "segment-comment" => {
                let d = self
                    .update_sentence_comment_popup
                    .clone()
                    .ok_or_else(|| invalid("Closed draft"))?;
                self.control_apply(
                    ctx,
                    Command::SetSegmentComment {
                        segment: d.segment_idx,
                        comment: d.comment,
                    },
                )?;
                self.update_sentence_comment_popup = None;
            }
            "word-formation" => {
                let d = self
                    .word_formation_popup
                    .clone()
                    .ok_or_else(|| invalid("Closed draft"))?;
                self.control_apply(
                    ctx,
                    Command::ApplyFormationRule {
                        word: d.selected_word,
                        base_word: d.base_word,
                        rule: d.selected_rule.ok_or_else(|| invalid("No selected rule"))?,
                    },
                )?;
                self.word_formation_popup = None;
            }
            "new-formation-rule" => {
                let d = self
                    .new_formation_rule_popup
                    .clone()
                    .ok_or_else(|| invalid("Closed draft"))?;
                self.control_apply(
                    ctx,
                    Command::CreateFormationRule {
                        description: d.description,
                        rule_type: d.rule_type,
                        command: d.command,
                    },
                )?;
                self.new_formation_rule_popup = None;
            }
            "remove-formation-rule" => {
                let d = self
                    .remove_formation_rule_popup
                    .clone()
                    .ok_or_else(|| invalid("Closed dialog"))?;
                self.control_apply(
                    ctx,
                    Command::RemoveFormationRule {
                        segment: d.sentence_idx,
                        token: d.word_idx,
                    },
                )?;
                self.remove_formation_rule_popup = None;
            }
            "confirmation" => {
                let (_, action) = self
                    .confirmation
                    .take()
                    .ok_or_else(|| invalid("Closed confirmation"))?;
                if action == AppAction::Quit {
                    self.control_data.pending = None;
                    return self.control_quit(ctx, true);
                }
                if let Some(pending) = self.control_data.pending.take() {
                    return self.control_commit_read(ctx, pending);
                }
                self.execute_action(action, ctx);
            }
            "import-options" => {
                let (content, name) = self
                    .pending_import
                    .clone()
                    .ok_or_else(|| invalid("Closed import"))?;
                if requested == "custom-script" {
                    self.pending_import = None;
                    self.custom_tokenization_popup = Some(CustomTokenizationDialog {
                        import_data: (content, name),
                        command: "fn tokenize(line) { line.split(\" \") }".into(),
                        test_text: String::new(),
                        preview: Vec::new(),
                    });
                    self.sync_control_windows();
                    return Ok(
                        json!({"handle":self.control_windows.borrow().find("custom-tokenization",None).map(|w|w.handle.clone()),"kind":"custom-tokenization"}),
                    );
                }
                let rule = if requested == "word-based" {
                    TokenizationRule::default_whitespace()
                } else {
                    TokenizationRule::default_character()
                };
                if !self.import_text(&content, &name, &rule, ctx) {
                    return Err(invalid(self.error_message.clone().unwrap_or_default()));
                }
            }
            "custom-tokenization" => {
                let d = self
                    .custom_tokenization_popup
                    .clone()
                    .ok_or_else(|| invalid("Closed tokenizer"))?;
                let rule = TokenizationRule {
                    description: "Custom tokenization".into(),
                    command: d.command,
                    cached_ast: tdector_eval::default_cached_ast(),
                };
                if requested == "test" {
                    let preview = tdector_app::Session::preview_tokenization(&rule, &d.test_text)
                        .map_err(app_error)?;
                    if let Some(d) = &mut self.custom_tokenization_popup {
                        d.preview = preview.clone();
                    }
                    return Ok(json!({"handle":handle,"preview":preview}));
                }
                if !self.import_text(&d.import_data.0, &d.import_data.1, &rule, ctx) {
                    return Err(failure(
                        "script_error",
                        self.error_message.clone().unwrap_or_default(),
                        4,
                    ));
                }
            }
            "definition" | "references" | "similar-segments" => {
                let id = self.next_popup_id;
                self.next_popup_id += 1;
                match kind {
                    "definition" | "references" => {
                        let word = inspected["target"]["word"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned();
                        self.pinned_popups.push(PinnedPopup::Dictionary(
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
                        ));
                        if kind == "definition" {
                            self.definition_popup = None;
                        } else {
                            self.reference_popup = None;
                        }
                    }
                    _ => {
                        let (s, scores) = self
                            .similar_popup
                            .take()
                            .ok_or_else(|| invalid("Closed similarity window"))?;
                        self.pinned_popups.push(PinnedPopup::Similar(
                            s,
                            scores,
                            id,
                            format!("📌 Similar to [{}]", s + 1),
                        ));
                    }
                }
                self.sync_control_windows();
                return Ok(json!({"handle":handle,"pinned":true}));
            }
            "word-menu" | "segment-menu" => {
                let mut target = inspected["target"].clone();
                target["kind"] = json!(requested);
                let opened = self.control_open(ctx, &target)?;
                self.control_close(ctx, handle)?;
                return Ok(opened);
            }
            _ => return Err(invalid("Unsupported action")),
        }
        self.sync_control_windows();
        Ok(json!({"handle":handle,"invoked":requested,"dirty":self.session.is_dirty()}))
    }
}
