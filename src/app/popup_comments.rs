//! Comment popup rendering.

use eframe::egui;

use super::state::DecryptionApp;
use crate::ui::popup_utils::create_popup_title;

impl DecryptionApp {
    pub(super) fn render_update_comment_menu_item(
        &mut self,
        ui: &mut egui::Ui,
        word: &str,
        should_close: &mut bool,
    ) {
        if ui
            .add(egui::Button::new("Update Comment").frame(false))
            .clicked()
        {
            // Get current comment if it exists
            let current_comment = self
                .project
                .vocabulary_comments
                .get(word)
                .cloned()
                .unwrap_or_default();

            self.update_comment_popup = Some(super::state::UpdateCommentDialog {
                word: word.to_string(),
                comment: current_comment,
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
                            // Update or insert the comment
                            if dialog.comment.is_empty() {
                                self.project.vocabulary_comments.remove(&dialog.word);
                            } else {
                                self.project
                                    .vocabulary_comments
                                    .insert(dialog.word.clone(), dialog.comment.clone());
                            }
                            self.update_dirty_status(true, ctx);
                            should_close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            // Just close without saving
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
