use eframe::egui;

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
            let (target, current_comment) = match self.session.token_comment(sentence_idx, word_idx)
            {
                Ok(comment) => comment,
                Err(error) => {
                    self.error_message = Some(error.to_string());
                    *should_close = true;
                    return;
                }
            };
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
                self.custom_font_name.is_some(),
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
                            should_close = self.apply_command(
                                tdector_app::Command::SetWordComment {
                                    target: dialog.target.clone(),
                                    comment: dialog.comment.clone(),
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
                            should_close = self.apply_command(
                                tdector_app::Command::SetSegmentComment {
                                    segment: dialog.segment_idx,
                                    comment: dialog.comment.clone(),
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
                self.update_sentence_comment_popup = Some(dialog);
            }
        }
    }
}
