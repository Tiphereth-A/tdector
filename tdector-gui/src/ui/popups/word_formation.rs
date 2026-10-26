use eframe::egui;

use crate::consts::ui::WORD_FORMATION_SCRIPT_ROWS;
use crate::enums::FormationType;
use crate::ui::popup_utils::create_popup_title;
use crate::ui::states::state::DecryptionApp;

fn rule_label(rule_type: FormationType, description: &str) -> String {
    let prefix = match rule_type {
        FormationType::Derivation => "[D]",
        FormationType::Inflection => "[I]",
        FormationType::Nonmorphological => "[N]",
    };
    format!("{prefix} {description}")
}

impl DecryptionApp {
    pub(super) fn render_word_formation_popup(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.word_formation_popup.take() {
            let mut open = true;
            let mut should_close = false;
            let title = create_popup_title(
                "Set Formation Rule: ",
                &dialog.selected_word,
                self.custom_font_name.is_some(),
            );

            self.popup_window("word-formation", None, title)
                .id(egui::Id::unique("word_formation_popup"))
                .open(&mut open)
                .default_width(400.0)
                .show(ctx, |ui| {
                    let font_family = if self.custom_font_name.is_some() {
                        egui::FontFamily::Name("SentenceFont".into())
                    } else {
                        egui::FontFamily::Proportional
                    };
                    let font_id = egui::FontId {
                        size: egui::TextStyle::Body.resolve(ui.style()).size,
                        family: font_family.clone(),
                    };
                    let old_base_word = dialog.base_word.clone();
                    let old_rule = dialog.selected_rule;

                    ui.label("Base word:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut dialog.base_word)
                                .id(crate::ui::managed::control_id(ui, "base-word"))
                                .font(font_id.clone()),
                        )
                        .changed()
                    {
                        dialog.related_words = self.find_related_words(&dialog.base_word);
                    }
                    if !dialog.base_word.is_empty() && !dialog.related_words.is_empty() {
                        ui.label("Related words:");
                        for word in dialog.related_words.clone() {
                            let label = egui::RichText::new(&word).font(font_id.clone());
                            if ui.selectable_label(false, label).clicked() {
                                dialog.base_word = word;
                                dialog.related_words.clear();
                            }
                        }
                    }

                    ui.separator();
                    ui.label("Formation rules:");
                    let selected_text = dialog
                        .selected_rule
                        .and_then(|index| self.session.project().formation_rules.get(index))
                        .map(|rule| rule_label(rule.rule_type, &rule.description))
                        .unwrap_or_default();
                    if crate::ui::managed::focus_requested("rule-search") {
                        // This is the popup id derived by the pinned ComboBox implementation.
                        egui::Popup::open_id(
                            ui.ctx(),
                            ui.make_persistent_id("formation_rule_combo").with("popup"),
                        );
                    }
                    let rule_response = egui::ComboBox::from_id_salt("formation_rule_combo")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut dialog.rule_search_text)
                                    .id(crate::ui::managed::control_id(ui, "rule-search")),
                            );
                            ui.separator();
                            let search = dialog.rule_search_text.to_lowercase();
                            let mut any_visible = false;
                            for (index, rule) in
                                self.session.project().formation_rules.iter().enumerate()
                            {
                                if !rule.description.to_lowercase().contains(&search) {
                                    continue;
                                }
                                any_visible = true;
                                if ui
                                    .selectable_label(
                                        dialog.selected_rule == Some(index),
                                        rule_label(rule.rule_type, &rule.description),
                                    )
                                    .clicked()
                                {
                                    dialog.selected_rule = Some(index);
                                    dialog.rule_search_text.clear();
                                }
                            }
                            if !any_visible {
                                ui.label(egui::RichText::new("No matching rules").weak());
                            }
                        });

                    crate::ui::managed::focus_response(ui, "rule", &rule_response.response);
                    if dialog.preview.is_empty()
                        || dialog.base_word != old_base_word
                        || dialog.selected_rule != old_rule
                    {
                        crate::ui::drafts::refresh_word_formation(&self.session, &mut dialog);
                    }

                    if !dialog.preview.is_empty() {
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Preview:");
                            ui.label(
                                egui::RichText::new(&dialog.preview)
                                    .family(font_family)
                                    .strong(),
                            );
                        });
                        if dialog.preview == dialog.selected_word {
                            ui.colored_label(egui::Color32::GREEN, "Preview matches selected word");
                        } else {
                            ui.colored_label(
                                egui::Color32::RED,
                                "Preview does not match selected word",
                            );
                        }
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                dialog.selected_rule.is_some() && !dialog.base_word.is_empty(),
                                egui::Button::new("Apply Rule"),
                            )
                            .clicked()
                            && let Some(rule) = dialog.selected_rule
                        {
                            should_close = self.apply_command(
                                tdector_app::Command::ApplyFormationRule {
                                    word: dialog.selected_word.clone(),
                                    base_word: dialog.base_word.clone(),
                                    rule,
                                },
                                ctx,
                            );
                        }
                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                });

            if open && !should_close {
                self.word_formation_popup = Some(dialog);
            }
        }
    }

    pub(super) fn render_new_formation_rule_popup(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.new_formation_rule_popup.take() {
            let mut open = true;
            let mut should_close = false;
            self.popup_window("new-formation-rule", None, "Create New Word Formation Rule")
                .id(egui::Id::unique("new_formation_rule_popup"))
                .open(&mut open)
                .default_width(500.0)
                .show(ctx, |ui| {
                    let font_family = if self.custom_font_name.is_some() {
                        egui::FontFamily::Name("SentenceFont".into())
                    } else {
                        egui::FontFamily::Proportional
                    };
                    let font_id = egui::FontId {
                        size: egui::TextStyle::Body.resolve(ui.style()).size,
                        family: font_family,
                    };
                    let old_command = dialog.command.clone();
                    let old_test_word = dialog.test_word.clone();
                    ui.label("Description:");
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.description)
                            .id(crate::ui::managed::control_id(ui, "description")),
                    );
                    ui.separator();
                    ui.label("Rule Type:");
                    for (rule_type, label) in [
                        (FormationType::Derivation, "Derivation"),
                        (FormationType::Inflection, "Inflection"),
                        (FormationType::Nonmorphological, "Nonmorphological"),
                    ] {
                        let response = ui.selectable_value(&mut dialog.rule_type, rule_type, label);
                        if dialog.rule_type == rule_type {
                            crate::ui::managed::focus_response(ui, "type", &response);
                        }
                    }
                    ui.separator();
                    ui.label("Rhai Script Command (fn transform(word: String) -> String):");
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.command)
                            .id(crate::ui::managed::control_id(ui, "script"))
                            .code_editor()
                            .desired_rows(WORD_FORMATION_SCRIPT_ROWS)
                            .desired_width(f32::INFINITY),
                    );
                    ui.separator();
                    ui.label("Test Word:");
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.test_word)
                            .id(crate::ui::managed::control_id(ui, "test-word"))
                            .font(font_id.clone()),
                    );
                    if dialog.preview.is_empty()
                        || dialog.command != old_command
                        || dialog.test_word != old_test_word
                    {
                        crate::ui::drafts::refresh_new_formation(&mut dialog);
                    }

                    if !dialog.preview.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Preview:");
                            ui.label(egui::RichText::new(&dialog.preview).font(font_id).strong());
                        });
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !dialog.description.is_empty() && !dialog.command.is_empty(),
                                egui::Button::new("Create Rule"),
                            )
                            .clicked()
                        {
                            should_close = self.apply_command(
                                tdector_app::Command::CreateFormationRule {
                                    description: dialog.description.clone(),
                                    rule_type: dialog.rule_type,
                                    command: dialog.command.clone(),
                                },
                                ctx,
                            );
                        }
                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                });
            if open && !should_close {
                self.new_formation_rule_popup = Some(dialog);
            }
        }
    }
}
