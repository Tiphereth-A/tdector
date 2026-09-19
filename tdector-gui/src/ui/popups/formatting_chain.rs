use eframe::egui;

use crate::ui::popup_utils::create_popup_title;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    pub(super) fn render_formatting_chain_popup(&mut self, ctx: &egui::Context) {
        let mut should_close = false;

        if let Some(dialog) = self.formatting_chain_popup.as_ref() {
            let mut open = true;
            let token_info = self
                .session
                .project()
                .segments
                .get(dialog.sentence_idx)
                .and_then(|segment| segment.tokens.get(dialog.word_idx))
                .map(|token| {
                    (
                        token.original.clone(),
                        token.base_word.as_ref().unwrap_or(&token.original).clone(),
                    )
                });
            let chain = self
                .session
                .formation_chain(dialog.sentence_idx, dialog.word_idx);
            let title_text = token_info
                .as_ref()
                .map(|(word, _)| word.as_str())
                .unwrap_or("Formatting Chain");
            let title = create_popup_title(
                "Formatting Chain: ",
                title_text,
                self.custom_font_name.is_some(),
            );

            egui::Window::new(title)
                .id(egui::Id::new("formatting_chain_popup"))
                .open(&mut open)
                .default_width(420.0)
                .default_height(260.0)
                .show(ctx, |ui| {
                    let font_family = if self.custom_font_name.is_some() {
                        egui::FontFamily::Name("SentenceFont".into())
                    } else {
                        egui::FontFamily::Proportional
                    };
                    if let Some((word, base_word)) = &token_info {
                        ui.horizontal(|ui| {
                            ui.label("Word: ");
                            ui.label(egui::RichText::new(word).family(font_family.clone()));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Base: ");
                            ui.label(egui::RichText::new(base_word).family(font_family.clone()));
                        });
                        ui.separator();
                    }
                    match &chain {
                        Ok(steps) if steps.is_empty() => {
                            ui.label("No formation rules applied to this word.");
                        }
                        Ok(steps) => {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    for (index, step) in steps.iter().enumerate() {
                                        ui.group(|ui| {
                                            let type_prefix = match step.rule_type {
                                                crate::enums::FormationType::Derivation => "[D]",
                                                crate::enums::FormationType::Inflection => "[I]",
                                                crate::enums::FormationType::Nonmorphological => {
                                                    "[N]"
                                                }
                                            };
                                            ui.label(format!(
                                                "Step {}: {} {}",
                                                index + 1,
                                                type_prefix,
                                                step.description,
                                            ));
                                            ui.horizontal(|ui| {
                                                ui.label("Result: ");
                                                ui.label(
                                                    egui::RichText::new(&step.result)
                                                        .family(font_family.clone()),
                                                );
                                            });
                                        });
                                    }
                                });
                        }
                        Err(error) => {
                            ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
                        }
                    }
                });
            if !open {
                should_close = true;
            }
        }
        if should_close {
            self.formatting_chain_popup = None;
        }
    }
}
