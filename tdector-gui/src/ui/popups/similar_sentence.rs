use eframe::egui;

use crate::consts::ui::{POPUP_SIMILAR_HEIGHT, POPUP_WIDTH};
use crate::enums::{PinnedPopup, PopupRequest};
use crate::ui;
use crate::ui::popup_utils::{create_pinned_title_string, create_popup_title};
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    pub(super) fn render_similar_popup(
        &mut self,
        ctx: &egui::Context,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some((target_idx, scores)) = self.similar_popup.as_ref() {
            let mut open = true;
            let target_label = format!("[{}]", target_idx + 1);
            let title = create_popup_title("Similar to ", &target_label, false);
            self.popup_window("similar-segments", None, title)
                .id(egui::Id::unique("similar_popup"))
                .open(&mut open)
                .default_width(POPUP_WIDTH)
                .default_height(POPUP_SIMILAR_HEIGHT)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("📌 Pin").clicked() {
                            should_pin = true;
                        }
                    });
                    ui.separator();
                    self.render_similar_content(ui, scores, popup_request, None);
                });

            if !open {
                should_close = true;
            }

            if should_pin {
                let pinned_title =
                    create_pinned_title_string("📌 Similar to ", &target_label, false);
                self.pinned_popups.push(PinnedPopup::Similar(
                    *target_idx,
                    scores.clone(),
                    self.next_popup_id,
                    pinned_title,
                ));
                self.next_popup_id += 1;
                should_close = true;
            }
        }

        if should_close {
            self.similar_popup = None;
        }
    }

    pub(super) fn render_similar_content(
        &self,
        ui: &mut egui::Ui,
        similar_indices: &[(usize, f64)],
        popup_request: &mut Option<PopupRequest>,
        _popup_id: Option<u64>,
    ) {
        crate::ui::managed::scroll_area(ui, "content", egui::ScrollArea::vertical())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, score) in similar_indices {
                    if let Some(seg) = self.session.project().segments.get(*idx) {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                let mut label_resp = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!(
                                            "[{}] (Score: {:.2})",
                                            idx + 1,
                                            score
                                        ))
                                        .strong(),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if !seg.comment.is_empty() {
                                    label_resp = label_resp.on_hover_text(&seg.comment);
                                }
                                if label_resp.secondary_clicked() {
                                    let cursor_pos = ui
                                        .ctx()
                                        .input(|i| i.pointer.interact_pos())
                                        .unwrap_or_default();
                                    *popup_request =
                                        Some(PopupRequest::SentenceMenu(*idx, cursor_pos));
                                }
                            });

                            crate::ui::managed::scroll_area(
                                ui,
                                &format!("segment:{idx}"),
                                egui::ScrollArea::horizontal(),
                            )
                            .max_width(ui.available_width())
                            .show(ui, |ui| {
                                if let Some(action) = ui::render_clickable_tokens(
                                    ui,
                                    &seg.tokens,
                                    self.session.project(),
                                    None,
                                    self.custom_font_name.is_some(),
                                ) {
                                    self.handle_ui_action(ui, action, popup_request, *idx);
                                }
                            });
                            ui.add_space(5.0);

                            if !seg.translation.is_empty() {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&seg.translation).italics(),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                    }
                }
            });
    }
}
