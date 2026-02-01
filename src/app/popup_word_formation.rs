//! Word formation popup rendering.

use eframe::egui;

use super::state::DecryptionApp;
use crate::ui::popup_utils::create_popup_title;

impl DecryptionApp {
    pub(super) fn render_word_formation_popup(&mut self, ctx: &egui::Context) {
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
                    let custom_font_id = egui::FontId {
                        size: egui::TextStyle::Body.resolve(ui.style()).size,
                        family: egui::FontFamily::Name("SentenceFont".into()),
                    };
                    let has_custom_font = self.project.font_path.is_some();

                    ui.label("Base word:");
                    let old_base_word = dialog.base_word.clone();
                    if has_custom_font {
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.base_word)
                                .font(custom_font_id.clone()),
                        );
                    } else {
                        ui.text_edit_singleline(&mut dialog.base_word);
                    }

                    // Update related words and preview if base word changed
                    if dialog.base_word != old_base_word {
                        dialog.related_words = self.find_related_words(&dialog.base_word);
                        // Update preview if a rule is already selected
                        if let Some(rule_idx) = dialog.selected_rule {
                            if let Some(rule) = self.project.formation_rules.get(rule_idx) {
                                if !dialog.base_word.is_empty() {
                                    dialog.preview = rule
                                        .apply(&dialog.base_word)
                                        .unwrap_or_else(|_| dialog.base_word.clone());
                                } else {
                                    dialog.preview.clear();
                                }
                            }
                        }
                    }

                    // Show top 5 related words
                    if !dialog.base_word.is_empty() && !dialog.related_words.is_empty() {
                        ui.label("Related words:");
                        let related_words_clone = dialog.related_words.clone();
                        for word in related_words_clone {
                            let word_label = if has_custom_font {
                                egui::RichText::new(&word).font(custom_font_id.clone())
                            } else {
                                egui::RichText::new(&word)
                            };
                            if ui.selectable_label(false, word_label).clicked() {
                                dialog.base_word = word;
                                dialog.related_words.clear(); // Hide suggestions after selection
                                // Update preview if a rule is already selected
                                if let Some(rule_idx) = dialog.selected_rule {
                                    if let Some(rule) = self.project.formation_rules.get(rule_idx) {
                                        dialog.preview = rule
                                            .apply(&dialog.base_word)
                                            .unwrap_or_else(|_| dialog.base_word.clone());
                                    }
                                }
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
                            let preview_text = if has_custom_font {
                                egui::RichText::new(&dialog.preview)
                                    .font(custom_font_id.clone())
                                    .strong()
                            } else {
                                egui::RichText::new(&dialog.preview).strong()
                            };
                            ui.label(preview_text);
                        });

                        // Check if preview matches the selected word
                        let matches = dialog.preview == dialog.selected_word;
                        // Check if base word exists in vocabulary
                        let base_word_exists =
                            self.project.vocabulary.contains_key(&dialog.base_word);

                        if !base_word_exists {
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(egui::Color32::RED, "Base word ");
                                let word_text = if has_custom_font {
                                    egui::RichText::new(&format!("'{}'", dialog.base_word))
                                        .font(custom_font_id.clone())
                                        .color(egui::Color32::RED)
                                } else {
                                    egui::RichText::new(&format!("'{}'", dialog.base_word))
                                        .color(egui::Color32::RED)
                                };
                                ui.label(word_text);
                                ui.colored_label(egui::Color32::RED, " not found in vocabulary");
                            });
                        }
                        if !matches {
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(egui::Color32::RED, "Preview ");
                                let preview_styled = if has_custom_font {
                                    egui::RichText::new(&format!("'{}'", dialog.preview))
                                        .font(custom_font_id.clone())
                                        .color(egui::Color32::RED)
                                } else {
                                    egui::RichText::new(&format!("'{}'", dialog.preview))
                                        .color(egui::Color32::RED)
                                };
                                ui.label(preview_styled);
                                ui.colored_label(
                                    egui::Color32::RED,
                                    " does not match selected word ",
                                );
                                let word_styled = if has_custom_font {
                                    egui::RichText::new(&format!("'{}'", dialog.selected_word))
                                        .font(custom_font_id.clone())
                                        .color(egui::Color32::RED)
                                } else {
                                    egui::RichText::new(&format!("'{}'", dialog.selected_word))
                                        .color(egui::Color32::RED)
                                };
                                ui.label(word_styled);
                            });
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
                            // Apply the rule to all matching tokens across sentences
                            if let Some(rule_idx) = dialog.selected_rule {
                                let base_word_for_lookup = dialog.base_word.clone();
                                if self.project.vocabulary.contains_key(&base_word_for_lookup) {
                                    // Remove the original derived word from vocabulary
                                    // since it's now represented as a formation of the base word
                                    let original_word = dialog.selected_word.clone();
                                    self.project.vocabulary.remove(&original_word);
                                    self.project.vocabulary_comments.remove(&original_word);

                                    for segment in &mut self.project.segments {
                                        for token in &mut segment.tokens {
                                            if token.original == original_word {
                                                // Update token with formation rule info
                                                token.base_word =
                                                    Some(base_word_for_lookup.clone());
                                                token.formation_rule_idx = Some(rule_idx);
                                                token.original = dialog.preview.clone();
                                            }
                                        }
                                    }

                                    // Mark as dirty and refresh caches
                                    self.update_dirty_status(true, ctx);
                                    self.filter_dirty = true;
                                    self.lookups_dirty = true;
                                    self.tfidf_dirty = true;
                                    self.tfidf_cache.invalidate();
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

    pub(super) fn render_new_formation_rule_popup(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.new_formation_rule_popup.take() {
            let mut open = true;
            let mut should_close = false;

            egui::Window::new("Create New Word Formation Rule")
                .id(egui::Id::new("new_formation_rule_popup"))
                .open(&mut open)
                .default_width(500.0)
                .show(ctx, |ui| {
                    let custom_font_id = egui::FontId {
                        size: egui::TextStyle::Body.resolve(ui.style()).size,
                        family: egui::FontFamily::Name("SentenceFont".into()),
                    };
                    let has_custom_font = self.project.font_path.is_some();

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
                    if has_custom_font {
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.test_word)
                                .font(custom_font_id.clone()),
                        );
                    } else {
                        ui.text_edit_singleline(&mut dialog.test_word);
                    }

                    // Test the command
                    if !dialog.test_word.is_empty() && !dialog.command.is_empty() {
                        let engine = crate::models::get_engine();

                        let result = engine.eval::<String>(&format!(
                            "{}\nlet result = transform(\"{}\");\nresult",
                            dialog.command, dialog.test_word
                        ));
                        dialog.preview = result.unwrap_or_else(|e| format!("Error: {}", e));
                    }

                    if !dialog.test_word.is_empty() && !dialog.preview.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Preview:");
                            let preview_text = if has_custom_font {
                                egui::RichText::new(&dialog.preview)
                                    .font(custom_font_id.clone())
                                    .strong()
                            } else {
                                egui::RichText::new(&dialog.preview).strong()
                            };
                            ui.label(preview_text);
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
}
