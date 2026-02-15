use eframe::egui;

use crate::ui::states::DecryptionApp;

impl DecryptionApp {
    pub(crate) fn render_custom_tokenization_popup(&mut self, ctx: &egui::Context) {
        let mut should_close = false;
        let mut should_apply = false;
        let mut should_test = false;

        if let Some(dialog) = &mut self.custom_tokenization_popup {
            let mut open = true;
            egui::Window::new("Custom Tokenization Script")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(true)
                .default_width(600.0)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading("Create Custom Tokenization Rule");
                    ui.add_space(8.0);

                    ui.label("Rhai Script (must define fn tokenize(line) -> Array):");
                    ui.add(
                        egui::TextEdit::multiline(&mut dialog.command)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(8.0);

                    ui.separator();
                    ui.label("Test your script:");
                    ui.horizontal(|ui| {
                        ui.label("Test text:");
                        ui.text_edit_singleline(&mut dialog.test_text);
                        if ui.button("Test").clicked() {
                            should_test = true;
                        }
                    });

                    if !dialog.preview.is_empty() {
                        ui.add_space(4.0);
                        ui.label("Tokens:");
                        ui.horizontal_wrapped(|ui| {
                            for token in &dialog.preview {
                                ui.label(format!("[{token}]"));
                            }
                        });
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            should_apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                });

            if !open {
                should_close = true;
            }
        }

        if should_test && let Some(dialog) = &mut self.custom_tokenization_popup {
            let test_rule = crate::libs::eval::TokenizationRule {
                description: "Custom tokenization".to_string(),
                command: dialog.command.clone(),
                cached_ast: crate::libs::eval::default_cached_ast(),
            };

            match test_rule.tokenize(&dialog.test_text) {
                Ok(tokens) => {
                    dialog.preview = tokens;
                }
                Err(e) => {
                    self.error_message = Some(format!("Script error: {e}"));
                    dialog.preview.clear();
                }
            }
        }

        if should_apply {
            if let Some(dialog) = self.custom_tokenization_popup.take() {
                let rule = crate::libs::eval::TokenizationRule {
                    description: "Custom tokenization".to_string(),
                    command: dialog.command,
                    cached_ast: crate::libs::eval::default_cached_ast(),
                };

                // Apply tokenization with the custom rule
                let (content, name) = dialog.import_data;
                let segments = crate::libs::text_analysis::TextProcessor::segment_text_with_rule(
                    &content,
                    Some(&rule),
                )
                .unwrap_or_else(|_| Vec::new());

                self.project.segments = segments;
                self.project.project_name = name;
                self.project.font_path = None;
                self.current_path = None;
                self.project_filename = None;
                self.filter_dirty = true;
                self.lookups_dirty = true;
                self.tfidf_dirty = true;
                self.filter_text.clear();
                self.clear_popups();
                self.update_dirty_status(true, ctx);
            }
            should_close = true;
        }

        if should_close {
            self.custom_tokenization_popup = None;
        }
    }
}
