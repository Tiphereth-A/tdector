use eframe::egui;

use crate::ui::popup_utils::create_popup_title;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    pub(super) fn render_formatting_chain_popup(&mut self, ctx: &egui::Context) {
        let mut should_close = false;

        if let Some(dialog) = self.formatting_chain_popup.as_ref() {
            let mut open = true;

            let token_info = self
                .project
                .segments
                .get(dialog.sentence_idx)
                .and_then(|seg| seg.tokens.get(dialog.word_idx))
                .map(|token| {
                    let base_word = token
                        .base_word
                        .clone()
                        .unwrap_or_else(|| token.original.clone());
                    (
                        token.original.clone(),
                        base_word,
                        token.formation_rule_indices.clone(),
                    )
                });

            let title_text = token_info
                .as_ref()
                .map(|(word, _, _)| word.as_str())
                .unwrap_or("Formatting Chain");
            let title = create_popup_title(
                "Formatting Chain: ",
                title_text,
                self.project.font_path.is_some(),
            );

            egui::Window::new(title)
                .id(egui::Id::new("formatting_chain_popup"))
                .open(&mut open)
                .default_width(420.0)
                .default_height(260.0)
                .show(ctx, |ui| {
                    if let Some((word, base_word, rule_indices)) = token_info {
                        let font_family = if self.project.font_path.is_some() {
                            egui::FontFamily::Name("SentenceFont".into())
                        } else {
                            egui::FontFamily::Proportional
                        };

                        ui.horizontal(|ui| {
                            ui.label("Word: ");
                            ui.label(
                                egui::RichText::new(&word)
                                    .family(font_family.clone()),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Base: ");
                            ui.label(
                                egui::RichText::new(&base_word)
                                    .family(font_family.clone()),
                            );
                        });
                        ui.separator();

                        if rule_indices.is_empty() {
                            ui.label("No formation rules applied to this word.");
                        } else {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut current = base_word;
                                    for (step_idx, rule_idx) in rule_indices.iter().enumerate() {
                                        ui.group(|ui| {
                                            if let Some(rule) =
                                                self.project.formation_rules.get(*rule_idx)
                                            {
                                                let type_prefix = match rule.rule_type {
                                                    crate::enums::FormationType::Derivation => {
                                                        "[D]"
                                                    }
                                                    crate::enums::FormationType::Inflection => {
                                                        "[I]"
                                                    }
                                                    crate::enums::FormationType::Nonmorphological => {
                                                        "[N]"
                                                    }
                                                };

                                                ui.label(format!(
                                                    "Step {}: {} {}",
                                                    step_idx + 1,
                                                    type_prefix,
                                                    rule.description
                                                ));

                                                match rule.apply(&current) {
                                                    Ok(next) => {
                                                        ui.horizontal(|ui| {
                                                            ui.label("Result: ");
                                                            ui.label(
                                                                egui::RichText::new(&next)
                                                                    .family(font_family.clone()),
                                                            );
                                                        });
                                                        current = next;
                                                    }
                                                    Err(err) => {
                                                        ui.colored_label(
                                                            egui::Color32::LIGHT_RED,
                                                            format!("Error: {err}"),
                                                        );
                                                    }
                                                }
                                            } else {
                                                ui.colored_label(
                                                    egui::Color32::LIGHT_RED,
                                                    format!(
                                                        "Step {}: Missing formation rule index {}",
                                                        step_idx + 1,
                                                        rule_idx
                                                    ),
                                                );
                                            }
                                        });
                                    }
                                });
                        }
                    } else {
                        ui.label("Word not found for this formatting chain.");
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
