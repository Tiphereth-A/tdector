//! Popup window rendering (Dictionary reference/definition and Similar segments).

use std::collections::HashMap;

use eframe::egui;

use crate::ui::{self, PopupMode, constants};

use super::actions::PinnedPopup;
use super::popup_utils::{create_pinned_title_string, create_popup_title};
use super::state::{DecryptionApp, PopupRequest};

impl DecryptionApp {
    /// Closes all active and pinned popups.
    pub(super) fn clear_popups(&mut self) {
        self.definition_popup = None;
        self.reference_popup = None;
        self.similar_popup = None;
        self.word_menu_popup = None;
        self.word_formation_popup = None;
        self.pinned_popups.clear();
    }

    /// Renders the currently active unpinned popups.
    pub(super) fn render_popups(
        &mut self,
        ctx: &egui::Context,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        self.render_definition_popup(ctx, headword_lookup, popup_request);
        self.render_reference_popup(ctx, usage_lookup, popup_request);
        self.render_similar_popup(ctx, popup_request);
        self.render_word_menu_popup(ctx, popup_request);
        self.render_word_formation_popup(ctx);
        self.render_new_formation_rule_popup(ctx);
    }

    fn render_definition_popup(
        &mut self,
        ctx: &egui::Context,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some(word) = self.definition_popup.as_ref() {
            let mut open = true;
            let title = create_popup_title("Definition: ", word, self.project.font_path.is_some());
            egui::Window::new(title)
                .id(egui::Id::new("def_popup"))
                .open(&mut open)
                .default_width(constants::POPUP_WIDTH)
                .default_height(constants::POPUP_DEFINITION_HEIGHT)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("📌 Pin").clicked() {
                            should_pin = true;
                        }
                    });
                    ui.separator();

                    self.render_dictionary_content(
                        ui,
                        word,
                        PopupMode::Definition,
                        headword_lookup,
                        &None,
                        popup_request,
                        None,
                    );
                });

            if !open {
                should_close = true;
            }

            if should_pin {
                let title = create_pinned_title_string(
                    "📌 Definition: ",
                    word,
                    self.project.font_path.is_some(),
                );
                self.pinned_popups.push(PinnedPopup::Dictionary(
                    word.clone(),
                    PopupMode::Definition,
                    self.next_popup_id,
                    title,
                ));
                self.next_popup_id += 1;
                should_close = true;
            }
        }

        if should_close {
            self.definition_popup = None;
        }
    }

    fn render_reference_popup(
        &mut self,
        ctx: &egui::Context,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some(word) = self.reference_popup.as_ref() {
            let mut open = true;
            let title = create_popup_title("References: ", word, self.project.font_path.is_some());
            egui::Window::new(title)
                .id(egui::Id::new("ref_popup"))
                .open(&mut open)
                .default_width(constants::POPUP_WIDTH)
                .default_height(constants::POPUP_REFERENCE_HEIGHT)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("📌 Pin").clicked() {
                            should_pin = true;
                        }
                    });
                    ui.separator();

                    self.render_dictionary_content(
                        ui,
                        word,
                        PopupMode::Reference,
                        &None,
                        usage_lookup,
                        popup_request,
                        None,
                    );
                });

            if !open {
                should_close = true;
            }

            if should_pin {
                let title = create_pinned_title_string(
                    "📌 References: ",
                    word,
                    self.project.font_path.is_some(),
                );
                self.pinned_popups.push(PinnedPopup::Dictionary(
                    word.clone(),
                    PopupMode::Reference,
                    self.next_popup_id,
                    title,
                ));
                self.next_popup_id += 1;
                should_close = true;
            }
        }

        if should_close {
            self.reference_popup = None;
        }
    }

    fn render_similar_popup(
        &mut self,
        ctx: &egui::Context,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some((target_idx, scores)) = self.similar_popup.as_ref() {
            let mut open = true;
            let target_label = format!("[{}]", target_idx + 1);
            let title = create_popup_title("Similar to ", &target_label, false);
            egui::Window::new(title)
                .id(egui::Id::new("similar_popup"))
                .open(&mut open)
                .default_width(constants::POPUP_WIDTH)
                .default_height(constants::POPUP_SIMILAR_HEIGHT)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("📌 Pin").clicked() {
                            should_pin = true;
                        }
                    });
                    ui.separator();
                    self.render_similar_content(ui, scores, popup_request, None);
                });

            if !open {
                should_close = true;
            }

            if should_pin {
                let pinned_title =
                    create_pinned_title_string("📌 Similar to ", &target_label, false);
                self.pinned_popups.push(PinnedPopup::Similar(
                    *target_idx,
                    scores.clone(),
                    self.next_popup_id,
                    pinned_title,
                ));
                self.next_popup_id += 1;
                should_close = true;
            }
        }

        if should_close {
            self.similar_popup = None;
        }
    }

    /// Renders all pinned popup windows.
    pub(super) fn render_pinned_popups(
        &mut self,
        ctx: &egui::Context,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut pinned_to_remove = Vec::new();

        for (i, popup) in self.pinned_popups.iter().enumerate() {
            let mut open = true;
            match popup {
                PinnedPopup::Dictionary(word, mode, id, title) => {
                    let height = match mode {
                        PopupMode::Definition => constants::POPUP_DEFINITION_HEIGHT,
                        PopupMode::Reference => constants::POPUP_REFERENCE_HEIGHT,
                    };
                    egui::Window::new(title.as_str())
                        .id(egui::Id::new(id))
                        .open(&mut open)
                        .default_width(constants::POPUP_WIDTH)
                        .default_height(height)
                        .show(ctx, |ui| {
                            self.render_dictionary_content(
                                ui,
                                word,
                                *mode,
                                headword_lookup,
                                usage_lookup,
                                popup_request,
                                Some(*id),
                            );
                        });
                }
                PinnedPopup::Similar(_target_idx, similar_indices, id, title) => {
                    egui::Window::new(title.as_str())
                        .id(egui::Id::new(id))
                        .open(&mut open)
                        .default_width(constants::POPUP_WIDTH)
                        .default_height(constants::POPUP_SIMILAR_HEIGHT)
                        .show(ctx, |ui| {
                            self.render_similar_content(
                                ui,
                                similar_indices,
                                popup_request,
                                Some(*id),
                            );
                        });
                }
            }
            if !open {
                pinned_to_remove.push(i);
            }
        }

        for i in pinned_to_remove.iter().rev() {
            self.pinned_popups.remove(*i);
        }
    }

    fn render_dictionary_content(
        &self,
        ui: &mut egui::Ui,
        word: &str,
        mode: PopupMode,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
        popup_id: Option<u64>,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match mode {
                PopupMode::Reference => {
                    self.render_segment_list(
                        ui,
                        word,
                        usage_lookup,
                        popup_request,
                        popup_id,
                        false,
                    );
                }
                PopupMode::Definition => {
                    self.render_segment_list(
                        ui,
                        word,
                        headword_lookup,
                        popup_request,
                        popup_id,
                        true,
                    );
                }
            });
    }

    /// Generic helper to render a list of segments obtained from a lookup map.
    fn render_segment_list(
        &self,
        ui: &mut egui::Ui,
        word: &str,
        lookup_map: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
        popup_id: Option<u64>,
        is_definition: bool,
    ) {
        if let Some(map) = lookup_map {
            if let Some(indices) = map.get(word) {
                for &idx in indices {
                    if let Some(seg) = self.project.segments.get(idx) {
                        ui.horizontal(|ui| {
                            let label_resp = ui.label(format!("[{}]", idx + 1));
                            if !seg.comment.is_empty() {
                                label_resp.on_hover_text(&seg.comment);
                            }
                            ui.vertical(|ui| {
                                let scroll_id = match popup_id {
                                    Some(id) => egui::Id::new(id).with(idx),
                                    None => egui::Id::new(idx),
                                };
                                egui::ScrollArea::horizontal()
                                    .id_salt(scroll_id)
                                    .max_width(ui.available_width())
                                    .show(ui, |ui| {
                                        let highlight =
                                            if !is_definition { Some(word) } else { None };

                                        if let Some(action) = ui::render_clickable_tokens(
                                            ui,
                                            &seg.tokens,
                                            &self.project.vocabulary,
                                            &self.project.vocabulary_comments,
                                            highlight,
                                            self.project.font_path.is_some(),
                                        ) {
                                            self.handle_ui_action(ui, action, popup_request);
                                        }
                                    });
                                ui.add_space(5.0);
                                let text = if is_definition {
                                    egui::RichText::new(&seg.translation).strong()
                                } else {
                                    egui::RichText::new(&seg.translation).weak()
                                };
                                ui.add(egui::Label::new(text).wrap());
                            });
                        });
                        ui.separator();
                    }
                }
            } else {
                ui.label(if is_definition {
                    "No definitions found."
                } else {
                    "No usages found."
                });
            }
        }
    }

    fn render_similar_content(
        &self,
        ui: &mut egui::Ui,
        similar_indices: &[(usize, f64)],
        popup_request: &mut Option<PopupRequest>,
        popup_id: Option<u64>,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, score) in similar_indices {
                    if let Some(seg) = self.project.segments.get(*idx) {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                let label_resp = ui.label(
                                    egui::RichText::new(format!(
                                        "[{}] (Score: {:.2})",
                                        idx + 1,
                                        score
                                    ))
                                    .strong(),
                                );
                                if !seg.comment.is_empty() {
                                    label_resp.on_hover_text(&seg.comment);
                                }
                            });

                            let scroll_id = match popup_id {
                                Some(id) => egui::Id::new(id).with(idx),
                                None => egui::Id::new(idx),
                            };
                            egui::ScrollArea::horizontal()
                                .id_salt(scroll_id)
                                .max_width(ui.available_width())
                                .show(ui, |ui| {
                                    if let Some(action) = ui::render_clickable_tokens(
                                        ui,
                                        &seg.tokens,
                                        &self.project.vocabulary,
                                        &self.project.vocabulary_comments,
                                        None,
                                        self.project.font_path.is_some(),
                                    ) {
                                        self.handle_ui_action(ui, action, popup_request);
                                    }
                                });
                            ui.add_space(5.0);

                            if !seg.translation.is_empty() {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&seg.translation).italics(),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                    }
                }
            });
    }

    fn render_word_menu_popup(
        &mut self,
        ctx: &egui::Context,
        popup_request: &mut Option<PopupRequest>,
    ) {
        if let Some((word, sentence_idx, word_idx, cursor_pos)) =
            self.word_menu_popup.as_ref().cloned()
        {
            let mut should_close = false;

            egui::Area::new(egui::Id::new("word_context_menu"))
                .order(egui::Order::Foreground)
                .movable(false)
                .fixed_pos(cursor_pos)
                .show(ctx, |ui| {
                    egui::Frame::menu(ui.style()).show(ui, |ui| {
                        ui.set_min_width(180.0);

                        if ui
                            .add(egui::Button::new("Show Definition").frame(false))
                            .clicked()
                        {
                            *popup_request = Some(PopupRequest::Dictionary(
                                word.clone(),
                                PopupMode::Definition,
                            ));
                            should_close = true;
                        }

                        if ui
                            .add(egui::Button::new("Show References").frame(false))
                            .clicked()
                        {
                            *popup_request =
                                Some(PopupRequest::Dictionary(word.clone(), PopupMode::Reference));
                            should_close = true;
                        }

                        if ui
                            .add(egui::Button::new("Set Word Formation Rule").frame(false))
                            .clicked()
                        {
                            // Open word formation rule dialog
                            self.word_formation_popup = Some(super::state::WordFormationDialog {
                                sentence_idx,
                                word_idx,
                                selected_word: word.clone(),
                                base_word: String::new(),
                                preview: String::new(),
                                selected_rule: None,
                                related_words: Vec::new(),
                                rule_search_text: String::new(),
                            });
                            should_close = true;
                        }
                    });
                });

            if should_close {
                self.word_menu_popup = None;
            }
        }
    }

    fn render_word_formation_popup(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.word_formation_popup.take() {
            let mut open = true;
            let title = create_popup_title(
                "Set Formation Rule: ",
                &dialog.selected_word,
                self.project.font_path.is_some(),
            );

            let mut should_keep = false;
            egui::Window::new(title)
                .id(egui::Id::new("word_formation_popup"))
                .open(&mut open)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.label("Base word:");
                    let old_base_word = dialog.base_word.clone();
                    ui.text_edit_singleline(&mut dialog.base_word);

                    // Update related words if base word changed
                    if dialog.base_word != old_base_word {
                        dialog.related_words = self.find_related_words(&dialog.base_word);
                    }

                    // Show top 5 related words
                    if !dialog.base_word.is_empty() && !dialog.related_words.is_empty() {
                        ui.label("Related words:");
                        let related_words_clone = dialog.related_words.clone();
                        for word in related_words_clone {
                            if ui.selectable_label(false, &word).clicked() {
                                dialog.base_word = word;
                                dialog.related_words.clear(); // Hide suggestions after selection
                            }
                        }
                    }

                    ui.separator();
                    ui.label("Formation rules:");

                    // Get the currently selected rule description
                    let selected_text = dialog
                        .selected_rule
                        .and_then(|idx| self.project.formation_rules.get(idx))
                        .map(|rule| {
                            let type_prefix = match rule.rule_type {
                                crate::models::FormationType::Derivation => "[D]",
                                crate::models::FormationType::Inflection => "[I]",
                                crate::models::FormationType::Nonmorphological => "[N]",
                            };
                            format!("{} {}", type_prefix, rule.description)
                        })
                        .unwrap_or_default();

                    let combo_id = egui::Id::new("formation_rule_combo");
                    egui::ComboBox::from_id_salt(combo_id)
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            // Show search text edit at the top with focus request
                            let text_edit_id = ui.id().with("search");
                            let text_edit_response = ui.add(
                                egui::TextEdit::singleline(&mut dialog.rule_search_text)
                                    .id(text_edit_id),
                            );

                            // Auto-focus the text edit when combo opens
                            if !text_edit_response.has_focus() {
                                ui.memory_mut(|mem| mem.request_focus(text_edit_id));
                            }

                            ui.separator();

                            let search_lower = dialog.rule_search_text.to_lowercase();
                            let mut any_visible = false;

                            for (rule_idx, rule) in self.project.formation_rules.iter().enumerate()
                            {
                                // Filter rules based on search text
                                if search_lower.is_empty()
                                    || rule.description.to_lowercase().contains(&search_lower)
                                {
                                    any_visible = true;
                                    let is_selected = dialog.selected_rule == Some(rule_idx);

                                    let type_prefix = match rule.rule_type {
                                        crate::models::FormationType::Derivation => "[D]",
                                        crate::models::FormationType::Inflection => "[I]",
                                        crate::models::FormationType::Nonmorphological => "[N]",
                                    };
                                    let display_text =
                                        format!("{} {}", type_prefix, rule.description);

                                    if ui.selectable_label(is_selected, display_text).clicked() {
                                        dialog.selected_rule = Some(rule_idx);
                                        // Update preview
                                        if !dialog.base_word.is_empty() {
                                            dialog.preview = rule
                                                .apply(&dialog.base_word)
                                                .unwrap_or_else(|_| dialog.base_word.clone());
                                        }
                                        // Clear search after selection
                                        dialog.rule_search_text.clear();
                                    }
                                }
                            }

                            if !any_visible && !dialog.rule_search_text.is_empty() {
                                ui.label(egui::RichText::new("No matching rules").weak());
                            }
                        });

                    ui.separator();

                    if !dialog.base_word.is_empty() && !dialog.preview.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Preview:");
                            ui.label(egui::RichText::new(&dialog.preview).strong());
                        });

                        // Check if preview matches the selected word
                        let matches = dialog.preview == dialog.selected_word;
                        // Check if base word exists in vocabulary
                        let base_word_exists =
                            self.project.vocabulary.contains_key(&dialog.base_word);

                        if !base_word_exists {
                            ui.colored_label(
                                egui::Color32::RED,
                                format!("Base word '{}' not found in vocabulary", dialog.base_word),
                            );
                        }
                        if !matches {
                            ui.colored_label(
                                egui::Color32::RED,
                                format!(
                                    "Preview '{}' does not match selected word '{}'",
                                    dialog.preview, dialog.selected_word
                                ),
                            );
                        } else {
                            ui.colored_label(egui::Color32::GREEN, "Preview matches selected word");
                        }

                        ui.separator();

                        if ui
                            .add_enabled(
                                matches && base_word_exists,
                                egui::Button::new("Apply Rule"),
                            )
                            .clicked()
                        {
                            // Apply the rule to the sentence
                            if let Some(rule_idx) = dialog.selected_rule
                                && dialog.sentence_idx < self.project.segments.len()
                                && dialog.word_idx
                                    < self.project.segments[dialog.sentence_idx].tokens.len()
                            {
                                let token = &mut self.project.segments[dialog.sentence_idx].tokens
                                    [dialog.word_idx];
                                // Find the vocabulary index of the base word
                                let base_word_for_lookup = dialog.base_word.clone();
                                if self.project.vocabulary.contains_key(&base_word_for_lookup) {
                                    // Remove the original derived word from vocabulary
                                    // since it's now represented as a formation of the base word
                                    let original_word = dialog.selected_word.clone();
                                    self.project.vocabulary.remove(&original_word);
                                    self.project.vocabulary_comments.remove(&original_word);

                                    // Update token with formation rule info
                                    token.base_word = Some(base_word_for_lookup);
                                    token.formation_rule_idx = Some(rule_idx);
                                    token.original = dialog.preview.clone();
                                    // Mark as dirty
                                    self.update_dirty_status(true, ctx);
                                }
                            }
                            should_keep = false;
                        } else {
                            should_keep = true;
                        }
                    } else {
                        should_keep = true;
                    }
                });

            if open && should_keep {
                self.word_formation_popup = Some(dialog);
            }
        }
    }

    fn render_new_formation_rule_popup(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.new_formation_rule_popup.take() {
            let mut open = true;
            let mut should_close = false;

            egui::Window::new("Create New Word Formation Rule")
                .id(egui::Id::new("new_formation_rule_popup"))
                .open(&mut open)
                .default_width(500.0)
                .show(ctx, |ui| {
                    ui.label("Description:");
                    ui.text_edit_singleline(&mut dialog.description);

                    ui.separator();
                    ui.label("Rule Type:");
                    let current_type = dialog.rule_type;
                    if ui
                        .selectable_label(
                            current_type == crate::models::FormationType::Derivation,
                            "Derivation",
                        )
                        .clicked()
                    {
                        dialog.rule_type = crate::models::FormationType::Derivation;
                    }
                    if ui
                        .selectable_label(
                            current_type == crate::models::FormationType::Inflection,
                            "Inflection",
                        )
                        .clicked()
                    {
                        dialog.rule_type = crate::models::FormationType::Inflection;
                    }
                    if ui
                        .selectable_label(
                            current_type == crate::models::FormationType::Nonmorphological,
                            "Nonmorphological",
                        )
                        .clicked()
                    {
                        dialog.rule_type = crate::models::FormationType::Nonmorphological;
                    }

                    ui.separator();
                    ui.label("Rhai Script Command (fn transform(word: String) -> String):");
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.command)
                            .code_editor()
                            .desired_rows(10)
                            .desired_width(f32::INFINITY),
                    );

                    ui.separator();
                    ui.label("Test Word:");
                    ui.text_edit_singleline(&mut dialog.test_word);

                    // Test the command
                    if !dialog.test_word.is_empty() && !dialog.command.is_empty() {
                        let mut engine = rhai::Engine::new();
                        // Security constraints
                        engine.set_max_expr_depths(10, 10);
                        engine.set_max_operations(1000);

                        // Disable all I/O operations
                        engine.disable_symbol("eval");
                        engine.disable_symbol("load");
                        engine.disable_symbol("save");
                        engine.disable_symbol("read");
                        engine.disable_symbol("write");
                        engine.disable_symbol("append");
                        engine.disable_symbol("delete");
                        engine.disable_symbol("copy");

                        // Disable network operations
                        engine.disable_symbol("http");
                        engine.disable_symbol("request");
                        engine.disable_symbol("fetch");
                        engine.disable_symbol("socket");
                        engine.disable_symbol("tcp");
                        engine.disable_symbol("udp");

                        // Disable system operations
                        engine.disable_symbol("system");
                        engine.disable_symbol("exec");
                        engine.disable_symbol("spawn");
                        engine.disable_symbol("command");

                        let result = engine.eval::<String>(&format!(
                            "{}\nlet result = transform(\"{}\");\nresult",
                            dialog.command, dialog.test_word
                        ));
                        dialog.preview = result.unwrap_or_else(|_| "Error in script".to_string());
                    }

                    if !dialog.test_word.is_empty() && !dialog.preview.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Preview:");
                            ui.label(egui::RichText::new(&dialog.preview).strong());
                        });
                    }

                    ui.separator();

                    if ui
                        .add_enabled(
                            !dialog.description.is_empty() && !dialog.command.is_empty(),
                            egui::Button::new("Create Rule"),
                        )
                        .clicked()
                    {
                        // Add the new rule to the project
                        self.project
                            .formation_rules
                            .push(crate::models::FormationRule {
                                description: dialog.description.clone(),
                                rule_type: dialog.rule_type,
                                command: dialog.command.clone(),
                            });
                        self.update_dirty_status(true, ctx);
                        should_close = true;
                    }
                });

            if open && !should_close {
                self.new_formation_rule_popup = Some(dialog);
            }
        }
    }

    fn handle_ui_action(
        &self,
        ui: &egui::Ui,
        action: ui::UiAction,
        popup_request: &mut Option<PopupRequest>,
    ) {
        match action {
            ui::UiAction::ShowDefinition(word) => {
                *popup_request = Some(PopupRequest::Dictionary(
                    word.to_string(),
                    PopupMode::Definition,
                ));
            }
            ui::UiAction::ShowReference(word) => {
                *popup_request = Some(PopupRequest::Dictionary(
                    word.to_string(),
                    PopupMode::Reference,
                ));
            }
            ui::UiAction::ShowWordMenu(word, word_idx) => {
                // Get actual cursor position for word menu in popup windows
                let cursor_pos = ui
                    .ctx()
                    .input(|i| i.pointer.interact_pos())
                    .unwrap_or_default();
                *popup_request = Some(PopupRequest::WordMenu(
                    word.to_string(),
                    0,
                    word_idx,
                    cursor_pos,
                ));
            }
            ui::UiAction::Filter(text) => {
                *popup_request = Some(PopupRequest::Filter(text.to_string()));
            }
            _ => {}
        }
    }
}
