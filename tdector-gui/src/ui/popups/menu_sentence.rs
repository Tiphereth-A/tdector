use eframe::egui;

use crate::enums::PopupRequest;
use crate::ui::states::state::{DecryptionApp, UpdateSentenceCommentDialog};

impl DecryptionApp {
    pub(super) fn render_sentence_menu_popup(
        &mut self,
        ctx: &egui::Context,
        popup_request: &mut Option<PopupRequest>,
    ) {
        if let Some((sentence_idx, cursor_pos)) = self.sentence_menu_popup.as_ref().cloned() {
            let mut should_close = false;

            self.popup_area("segment-menu", cursor_pos).show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(200.0);

                    let similarity_button =
                        ui.add(egui::Button::new("Query Similar Sentences").frame(false));

                    if similarity_button.clicked() {
                        *popup_request = Some(PopupRequest::Similar(sentence_idx));
                        should_close = true;
                    }

                    if ui
                        .add(egui::Button::new("Update Comments").frame(false))
                        .clicked()
                    {
                        let current_comment = self
                            .session
                            .project()
                            .segments
                            .get(sentence_idx)
                            .map(|segment| segment.comment.clone())
                            .unwrap_or_default();

                        if !self.open_from_mouse(
                            ui.ctx(),
                            "segment-comment",
                            None,
                            Some(sentence_idx),
                            None,
                        ) {
                            self.update_sentence_comment_popup =
                                Some(UpdateSentenceCommentDialog {
                                    segment_idx: sentence_idx,
                                    comment: current_comment,
                                });
                        }
                        should_close = true;
                    }
                });
            });

            if should_close {
                self.sentence_menu_popup = None;
            }
        }
    }
}
