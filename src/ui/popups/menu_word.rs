use eframe::egui;

use crate::enums::{DictionaryPopupType, PopupRequest};
use crate::ui::states::state::{DecryptionApp, WordFormationDialog};

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

            let (existing_base_word, existing_rule_idx) = self
                .project
                .segments
                .get(sentence_idx)
                .and_then(|seg| seg.tokens.get(word_idx))
                .map(|token| {
                    (
                        token.base_word.clone(),
                        token.formation_rule_indices.last().copied(),
                    )
                })
                .unwrap_or((None, None));

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
                                DictionaryPopupType::Definition,
                            ));
                            should_close = true;
                        }

                        if ui
                            .add(egui::Button::new("Show References").frame(false))
                            .clicked()
                        {
                            *popup_request = Some(PopupRequest::Dictionary(
                                word.clone(),
                                DictionaryPopupType::Reference,
                            ));
                            should_close = true;
                        }

                        if ui
                            .add(egui::Button::new("Show Similar Tokens").frame(false))
                            .clicked()
                        {
                            *popup_request = Some(PopupRequest::SimilarTokens(word.clone()));
                            should_close = true;
                        }

                        if ui
                            .add(egui::Button::new("Set Word Formation Rule").frame(false))
                            .clicked()
                        {
                            self.word_formation_popup = Some(WordFormationDialog {
                                selected_word: word.clone(),
                                base_word: existing_base_word.unwrap_or_default(),
                                preview: String::new(),
                                selected_rule: existing_rule_idx,
                                related_words: Vec::new(),
                                rule_search_text: String::new(),
                            });
                            should_close = true;
                        }

                        if ui
                            .add(egui::Button::new("Show Formatting Chain").frame(false))
                            .clicked()
                        {
                            *popup_request =
                                Some(PopupRequest::FormattingChain(sentence_idx, word_idx));
                            should_close = true;
                        }

                        self.render_update_comment_menu_item(
                            ui,
                            &word,
                            sentence_idx,
                            word_idx,
                            &mut should_close,
                        );
                    });
                });

            if should_close {
                self.word_menu_popup = None;
            }
        }
    }
}
