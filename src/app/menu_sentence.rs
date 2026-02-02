//! Sentence context menu popup rendering.

use eframe::egui;

use super::state::{DecryptionApp, PopupRequest, UpdateSentenceCommentDialog};

impl DecryptionApp {
    pub(super) fn render_sentence_menu_popup(
        &mut self,
        ctx: &egui::Context,
        #[cfg_attr(target_arch = "wasm32", allow(unused_variables))] popup_request: &mut Option<
            PopupRequest,
        >,
    ) {
        if let Some((sentence_idx, cursor_pos)) = self.sentence_menu_popup.as_ref().cloned() {
            let mut should_close = false;

            egui::Area::new(egui::Id::new("sentence_context_menu"))
                .order(egui::Order::Foreground)
                .movable(false)
                .fixed_pos(cursor_pos)
                .show(ctx, |ui| {
                    egui::Frame::menu(ui.style()).show(ui, |ui| {
                        ui.set_min_width(200.0);

                        let similarity_button = ui.add_enabled(
                            cfg!(not(target_arch = "wasm32")),
                            egui::Button::new("Query Similar Sentences").frame(false)
                        );

                        #[cfg(not(target_arch = "wasm32"))]
                        if similarity_button.clicked() {
                            {
                                *popup_request = Some(PopupRequest::Similar(sentence_idx));
                                should_close = true;
                            }
                        }

                        #[cfg(target_arch = "wasm32")]
                        if similarity_button.hovered() {
                            similarity_button.on_hover_text("Similarity search is not yet supported in the web version.\nAwaiting SciRS2 v0.3.0 for WASM compatibility.");
                        }

                        if ui
                            .add(egui::Button::new("Update Comments").frame(false))
                            .clicked()
                        {
                            let current_comment = self
                                .project
                                .segments
                                .get(sentence_idx)
                                .map(|segment| segment.comment.clone())
                                .unwrap_or_default();

                            self.update_sentence_comment_popup =
                                Some(UpdateSentenceCommentDialog {
                                    segment_idx: sentence_idx,
                                    comment: current_comment,
                                });
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
