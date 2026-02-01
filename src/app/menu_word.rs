//! Word context menu popup rendering.

use eframe::egui;

use crate::ui::PopupMode;

use super::state::{DecryptionApp, PopupRequest, WordFormationDialog};

impl DecryptionApp {
    pub(super) fn render_word_menu_popup(
        &mut self,
        ctx: &egui::Context,
        popup_request: &mut Option<PopupRequest>,
    ) {
        if let Some((word, sentence_idx, word_idx, cursor_pos)) =
            self.word_menu_popup.as_ref().cloned()
        {
            let mut should_close = false;

            // Check if the word already has a formation rule applied
            let has_formation_rule = self
                .project
                .segments
                .get(sentence_idx)
                .and_then(|seg| seg.tokens.get(word_idx))
                .map(|token| token.formation_rule_idx.is_some())
                .unwrap_or(false);

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
                            .add_enabled(
                                !has_formation_rule,
                                egui::Button::new("Set Word Formation Rule").frame(false),
                            )
                            .clicked()
                        {
                            // Open word formation rule dialog
                            self.word_formation_popup = Some(WordFormationDialog {
                                selected_word: word.clone(),
                                base_word: String::new(),
                                preview: String::new(),
                                selected_rule: None,
                                related_words: Vec::new(),
                                rule_search_text: String::new(),
                            });
                            should_close = true;
                        }

                        self.render_update_comment_menu_item(ui, &word, &mut should_close);
                    });
                });

            if should_close {
                self.word_menu_popup = None;
            }
        }
    }
}
