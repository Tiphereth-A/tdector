use eframe::egui;

use crate::ui::states::DecryptionApp;

impl DecryptionApp {
    pub(crate) fn render_custom_tokenization_popup(&mut self, ctx: &egui::Context) {
        let mut should_close = false;
        let mut should_apply = false;
        let mut should_test = false;

        let window = self
            .custom_tokenization_popup
            .as_ref()
            .map(|_| self.popup_window("custom-tokenization", None, "Custom Tokenization Script"));
        if let Some(dialog) = &mut self.custom_tokenization_popup {
            let mut open = true;
            window
                .expect("open tokenizer")
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
                            .id(crate::ui::managed::control_id(ui, "script"))
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(8.0);

                    ui.separator();
                    ui.label("Test your script:");
                    ui.horizontal(|ui| {
                        ui.label("Test text:");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.test_text)
                                .id(crate::ui::managed::control_id(ui, "test-text")),
                        );
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
            let test_rule = tdector_eval::TokenizationRule {
                description: "Custom tokenization".to_string(),
                command: dialog.command.clone(),
                cached_ast: tdector_eval::default_cached_ast(),
            };

            match tdector_app::Session::preview_tokenization(&test_rule, &dialog.test_text) {
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
                let rule = tdector_eval::TokenizationRule {
                    description: "Custom tokenization".to_string(),
                    command: dialog.command.clone(),
                    cached_ast: tdector_eval::default_cached_ast(),
                };

                let (content, name) = &dialog.import_data;
                should_close = self.import_text(content, name, &rule, ctx);
                if !should_close {
                    self.custom_tokenization_popup = Some(dialog);
                }
            }
        }

        if should_close {
            self.custom_tokenization_popup = None;
        }
    }
}
