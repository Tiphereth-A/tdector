//! Dictionary popup rendering (Definition and Reference popups).

use std::collections::HashMap;

use eframe::egui;

use crate::ui::{self, PopupMode, constants};

use super::actions::PinnedPopup;
use super::state::{DecryptionApp, PopupRequest};
use crate::ui::popup_utils::{create_pinned_title_string, create_popup_title};

impl DecryptionApp {
    pub(super) fn render_definition_popup(
        &mut self,
        ctx: &egui::Context,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some(word) = self.definition_popup.as_ref() {
            let mut open = true;
            let title = create_popup_title("Definition: ", word, self.project.font_path.is_some());
            egui::Window::new(title)
                .id(egui::Id::new("def_popup"))
                .open(&mut open)
                .default_width(constants::POPUP_WIDTH)
                .default_height(constants::POPUP_DEFINITION_HEIGHT)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("📌 Pin").clicked() {
                            should_pin = true;
                        }
                    });
                    ui.separator();

                    self.render_dictionary_content(
                        ui,
                        word,
                        PopupMode::Definition,
                        headword_lookup,
                        &None,
                        popup_request,
                        None,
                    );
                });

            if !open {
                should_close = true;
            }

            if should_pin {
                let title = create_pinned_title_string(
                    "📌 Definition: ",
                    word,
                    self.project.font_path.is_some(),
                );
                self.pinned_popups.push(PinnedPopup::Dictionary(
                    word.clone(),
                    PopupMode::Definition,
                    self.next_popup_id,
                    title,
                ));
                self.next_popup_id += 1;
                should_close = true;
            }
        }

        if should_close {
            self.definition_popup = None;
        }
    }

    pub(super) fn render_reference_popup(
        &mut self,
        ctx: &egui::Context,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some(word) = self.reference_popup.as_ref() {
            let mut open = true;
            let title = create_popup_title("References: ", word, self.project.font_path.is_some());
            egui::Window::new(title)
                .id(egui::Id::new("ref_popup"))
                .open(&mut open)
                .default_width(constants::POPUP_WIDTH)
                .default_height(constants::POPUP_REFERENCE_HEIGHT)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("📌 Pin").clicked() {
                            should_pin = true;
                        }
                    });
                    ui.separator();

                    self.render_dictionary_content(
                        ui,
                        word,
                        PopupMode::Reference,
                        &None,
                        usage_lookup,
                        popup_request,
                        None,
                    );
                });

            if !open {
                should_close = true;
            }

            if should_pin {
                let title = create_pinned_title_string(
                    "📌 References: ",
                    word,
                    self.project.font_path.is_some(),
                );
                self.pinned_popups.push(PinnedPopup::Dictionary(
                    word.clone(),
                    PopupMode::Reference,
                    self.next_popup_id,
                    title,
                ));
                self.next_popup_id += 1;
                should_close = true;
            }
        }

        if should_close {
            self.reference_popup = None;
        }
    }

    pub(super) fn render_dictionary_content(
        &self,
        ui: &mut egui::Ui,
        word: &str,
        mode: PopupMode,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
        popup_id: Option<u64>,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match mode {
                PopupMode::Reference => {
                    self.render_segment_list(
                        ui,
                        word,
                        usage_lookup,
                        popup_request,
                        popup_id,
                        false,
                    );
                }
                PopupMode::Definition => {
                    self.render_segment_list(
                        ui,
                        word,
                        headword_lookup,
                        popup_request,
                        popup_id,
                        true,
                    );
                }
            });
    }

    /// Generic helper to render a list of segments obtained from a lookup map.
    pub(super) fn render_segment_list(
        &self,
        ui: &mut egui::Ui,
        word: &str,
        lookup_map: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
        popup_id: Option<u64>,
        is_definition: bool,
    ) {
        if let Some(map) = lookup_map {
            if let Some(indices) = map.get(word) {
                for &idx in indices {
                    if let Some(seg) = self.project.segments.get(idx) {
                        ui.horizontal(|ui| {
                            let mut label_resp = ui.add(
                                egui::Label::new(format!("[{}]", idx + 1))
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
                                *popup_request = Some(PopupRequest::SentenceMenu(idx, cursor_pos));
                            }
                            ui.vertical(|ui| {
                                let scroll_id = match popup_id {
                                    Some(id) => egui::Id::new(id).with(idx),
                                    None => egui::Id::new(idx),
                                };
                                egui::ScrollArea::horizontal()
                                    .id_salt(scroll_id)
                                    .max_width(ui.available_width())
                                    .show(ui, |ui| {
                                        let highlight =
                                            if !is_definition { Some(word) } else { None };

                                        if let Some(action) = ui::render_clickable_tokens(
                                            ui,
                                            &seg.tokens,
                                            &self.project.vocabulary,
                                            &self.project.vocabulary_comments,
                                            highlight,
                                            self.project.font_path.is_some(),
                                            &self.project.formation_rules,
                                        ) {
                                            self.handle_ui_action(ui, action, popup_request);
                                        }
                                    });
                                ui.add_space(5.0);
                                let text = if is_definition {
                                    egui::RichText::new(&seg.translation).strong()
                                } else {
                                    egui::RichText::new(&seg.translation).weak()
                                };
                                ui.add(egui::Label::new(text).wrap());
                            });
                        });
                        ui.separator();
                    }
                }
            } else {
                ui.label(if is_definition {
                    "No definitions found."
                } else {
                    "No usages found."
                });
            }
        }
    }

    pub(super) fn handle_ui_action(
        &self,
        ui: &egui::Ui,
        action: ui::UiAction,
        popup_request: &mut Option<PopupRequest>,
    ) {
        match action {
            ui::UiAction::ShowDefinition(word) => {
                *popup_request = Some(PopupRequest::Dictionary(
                    word.to_string(),
                    PopupMode::Definition,
                ));
            }
            ui::UiAction::ShowReference(word) => {
                *popup_request = Some(PopupRequest::Dictionary(
                    word.to_string(),
                    PopupMode::Reference,
                ));
            }
            ui::UiAction::ShowWordMenu(word, word_idx) => {
                let cursor_pos = ui
                    .ctx()
                    .input(|i| i.pointer.interact_pos())
                    .unwrap_or_default();
                *popup_request = Some(PopupRequest::WordMenu(
                    word.to_string(),
                    0,
                    word_idx,
                    cursor_pos,
                ));
            }
            ui::UiAction::Filter(text) => {
                *popup_request = Some(PopupRequest::Filter(text.to_string()));
            }
            _ => {}
        }
    }
}
