use eframe::egui;

use crate::ui::popup_utils::create_popup_title;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    pub(super) fn render_remove_formation_rule_popup(&mut self, ctx: &egui::Context) {
        if let Some(dialog) = self.remove_formation_rule_popup.take() {
            let mut open = true;
            let mut should_close = false;
            let title = create_popup_title(
                "Remove Formation Rule: ",
                &dialog.formatted_word,
                self.project.font_path.is_some(),
            );

            egui::Window::new(title)
                .id(egui::Id::new("remove_formation_rule_popup"))
                .open(&mut open)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Word:");
                        ui.strong(&dialog.formatted_word);
                    });

                    ui.horizontal_wrapped(|ui| {
                        ui.label("Base word:");
                        ui.strong(&dialog.base_word);
                    });

                    ui.horizontal_wrapped(|ui| {
                        ui.label("Rule to remove:");
                        ui.strong(&dialog.rule_description);
                    });

                    ui.separator();
                    ui.label("This removes the latest applied formation rule for this word.");

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Remove").clicked() {
                            let token_data = self
                                .project
                                .segments
                                .get(dialog.sentence_idx)
                                .and_then(|segment| segment.tokens.get(dialog.word_idx))
                                .map(|token| {
                                    (
                                        token.original.clone(),
                                        token
                                            .base_word
                                            .clone()
                                            .unwrap_or_else(|| token.original.clone()),
                                        token.formation_rule_indices.clone(),
                                    )
                                });

                            if let Some((old_word, base_word, original_chain)) = token_data
                                && !original_chain.is_empty()
                            {
                                let mut changed = false;

                                for segment in &mut self.project.segments {
                                    for token in &mut segment.tokens {
                                        let token_base_word = token
                                            .base_word
                                            .clone()
                                            .unwrap_or_else(|| token.original.clone());

                                        if token.original == old_word
                                            && token.formation_rule_indices == original_chain
                                            && token_base_word == base_word
                                        {
                                            token.original = old_word.clone();
                                            token.formation_rule_indices.clear();
                                            token.base_word = None;
                                            changed = true;
                                        }
                                    }
                                }

                                if changed {
                                    if let Some(comment) =
                                        self.project.formatted_word_comments.remove(&old_word)
                                    {
                                        self.project
                                            .vocabulary_comments
                                            .entry(old_word)
                                            .or_insert(comment);
                                    }

                                    self.update_dirty_status(true, ctx);
                                    self.filter_dirty = true;
                                    self.lookups_dirty = true;
                                    self.tfidf_dirty = true;
                                    self.tfidf_cache.invalidate();
                                }
                            }

                            should_close = true;
                        }

                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                });

            if open && !should_close {
                self.remove_formation_rule_popup = Some(dialog);
            }
        }
    }
}
