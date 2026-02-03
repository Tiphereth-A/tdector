//! Comment popup rendering.

use eframe::egui;

use crate::enums::CommentTarget;
use crate::ui::popup_utils::create_popup_title;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    pub(super) fn render_update_comment_menu_item(
        &mut self,
        ui: &mut egui::Ui,
        word: &str,
        sentence_idx: usize,
        word_idx: usize,
        should_close: &mut bool,
    ) {
        if ui
            .add(egui::Button::new("Update Comment").frame(false))
            .clicked()
        {
            let (target, current_comment) = self
                .project
                .segments
                .get(sentence_idx)
                .and_then(|seg| seg.tokens.get(word_idx))
                .map(|token| {
                    let base_word = token
                        .base_word
                        .clone()
                        .unwrap_or_else(|| token.original.clone());

                    if token.formation_rule_indices.is_empty() {
                        let comment = self
                            .project
                            .vocabulary_comments
                            .get(&base_word)
                            .cloned()
                            .unwrap_or_default();
                        (CommentTarget::BaseWord(base_word), comment)
                    } else {
                        let formatted_word = token.original.clone();
                        let comment = self
                            .project
                            .formatted_word_comments
                            .get(&formatted_word)
                            .cloned()
                            .unwrap_or_default();
                        (CommentTarget::FormattedWord(formatted_word), comment)
                    }
                })
                .unwrap_or_else(|| {
                    (
                        CommentTarget::BaseWord(word.to_string()),
                        self.project
                            .vocabulary_comments
                            .get(word)
                            .cloned()
                            .unwrap_or_default(),
                    )
                });

            self.update_comment_popup = Some(crate::ui::states::state::UpdateCommentDialog {
                word: word.to_string(),
                comment: current_comment,
                target,
            });
            *should_close = true;
        }
    }

    pub(super) fn render_update_comment_popup(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.update_comment_popup.take() {
            let mut open = true;
            let title = create_popup_title(
                "Update Comment: ",
                &dialog.word,
                self.project.font_path.is_some(),
            );

            let mut should_close = false;
            egui::Window::new(title)
                .id(egui::Id::new("update_comment_popup"))
                .open(&mut open)
                .default_width(350.0)
                .show(ctx, |ui| {
                    ui.label("Comment:");
                    ui.text_edit_multiline(&mut dialog.comment);

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            match &dialog.target {
                                CommentTarget::BaseWord(base_word) => {
                                    if dialog.comment.is_empty() {
                                        self.project.vocabulary_comments.remove(base_word);
                                    } else {
                                        self.project
                                            .vocabulary_comments
                                            .insert(base_word.clone(), dialog.comment.clone());
                                    }
                                }
                                CommentTarget::FormattedWord(formatted_word) => {
                                    if dialog.comment.is_empty() {
                                        self.project.formatted_word_comments.remove(formatted_word);
                                    } else {
                                        self.project
                                            .formatted_word_comments
                                            .insert(formatted_word.clone(), dialog.comment.clone());
                                    }
                                }
                            }
                            self.update_dirty_status(true, ctx);
                            should_close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                });

            if open && !should_close {
                self.update_comment_popup = Some(dialog);
            }
        }
    }

    pub(super) fn render_update_sentence_comment_popup(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.update_sentence_comment_popup.take() {
            let mut open = true;

            let mut should_close = false;
            egui::Window::new("Update Sentence Comment")
                .id(egui::Id::new("update_sentence_comment_popup"))
                .open(&mut open)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.label("Comment:");
                    ui.text_edit_multiline(&mut dialog.comment);

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            if let Some(segment) = self.project.segments.get_mut(dialog.segment_idx)
                            {
                                segment.comment = dialog.comment.clone();
                                self.update_dirty_status(true, ctx);
                            }
                            should_close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                });

            if open && !should_close {
                self.update_sentence_comment_popup = Some(dialog);
            }
        }
    }
}
