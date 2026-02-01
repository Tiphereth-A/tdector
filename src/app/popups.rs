//! Popup window rendering (Dictionary reference/definition and Similar segments).

use std::collections::HashMap;

use eframe::egui;

use crate::ui::{self, PopupMode, constants};

use super::actions::PinnedPopup;
use super::state::{DecryptionApp, PopupRequest};

impl DecryptionApp {
    /// Closes all active and pinned popups.
    pub(super) fn clear_popups(&mut self) {
        self.definition_popup = None;
        self.reference_popup = None;
        self.similar_popup = None;
        self.pinned_popups.clear();
    }

    /// Renders the currently active unpinned popups.
    pub(super) fn render_popups(
        &mut self,
        ctx: &egui::Context,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        self.render_definition_popup(ctx, headword_lookup, popup_request);
        self.render_reference_popup(ctx, usage_lookup, popup_request);
        self.render_similar_popup(ctx, popup_request);
    }

    fn render_definition_popup(
        &mut self,
        ctx: &egui::Context,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some(word) = self.definition_popup.as_ref() {
            let mut open = true;
            egui::Window::new(format!("Definition: {}", word))
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
                let title = format!("📌 Definition: {}", word);
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

    fn render_reference_popup(
        &mut self,
        ctx: &egui::Context,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some(word) = self.reference_popup.as_ref() {
            let mut open = true;
            egui::Window::new(format!("References: {}", word))
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
                let title = format!("📌 References: {}", word);
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

    fn render_similar_popup(
        &mut self,
        ctx: &egui::Context,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;
        let mut should_pin = false;

        if let Some((target_idx, scores)) = self.similar_popup.as_ref() {
            let mut open = true;
            egui::Window::new(format!("Similar to [{}]", target_idx + 1))
                .id(egui::Id::new("similar_popup"))
                .open(&mut open)
                .default_width(constants::POPUP_WIDTH)
                .default_height(constants::POPUP_SIMILAR_HEIGHT)
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
                let title = format!("📌 Similar to [{}]", *target_idx + 1);
                self.pinned_popups.push(PinnedPopup::Similar(
                    *target_idx,
                    scores.clone(),
                    self.next_popup_id,
                    title,
                ));
                self.next_popup_id += 1;
                should_close = true;
            }
        }

        if should_close {
            self.similar_popup = None;
        }
    }

    /// Renders all pinned popup windows.
    pub(super) fn render_pinned_popups(
        &mut self,
        ctx: &egui::Context,
        headword_lookup: &Option<HashMap<String, Vec<usize>>>,
        usage_lookup: &Option<HashMap<String, Vec<usize>>>,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let mut pinned_to_remove = Vec::new();

        for (i, popup) in self.pinned_popups.iter().enumerate() {
            let mut open = true;
            match popup {
                PinnedPopup::Dictionary(word, mode, id, title) => {
                    let height = match mode {
                        PopupMode::Definition => constants::POPUP_DEFINITION_HEIGHT,
                        PopupMode::Reference => constants::POPUP_REFERENCE_HEIGHT,
                    };
                    egui::Window::new(title.as_str())
                        .id(egui::Id::new(id))
                        .open(&mut open)
                        .default_width(constants::POPUP_WIDTH)
                        .default_height(height)
                        .show(ctx, |ui| {
                            self.render_dictionary_content(
                                ui,
                                word,
                                *mode,
                                headword_lookup,
                                usage_lookup,
                                popup_request,
                                Some(*id),
                            );
                        });
                }
                PinnedPopup::Similar(_target_idx, similar_indices, id, title) => {
                    egui::Window::new(title.as_str())
                        .id(egui::Id::new(id))
                        .open(&mut open)
                        .default_width(constants::POPUP_WIDTH)
                        .default_height(constants::POPUP_SIMILAR_HEIGHT)
                        .show(ctx, |ui| {
                            self.render_similar_content(
                                ui,
                                similar_indices,
                                popup_request,
                                Some(*id),
                            );
                        });
                }
            }
            if !open {
                pinned_to_remove.push(i);
            }
        }

        for i in pinned_to_remove.iter().rev() {
            self.pinned_popups.remove(*i);
        }
    }

    fn render_dictionary_content(
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
    fn render_segment_list(
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
                            let label_resp = ui.label(format!("[{}]", idx + 1));
                            if !seg.comment.is_empty() {
                                label_resp.on_hover_text(&seg.comment);
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
                                        ) {
                                            self.handle_ui_action(action, popup_request);
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

    fn render_similar_content(
        &self,
        ui: &mut egui::Ui,
        similar_indices: &[(usize, f64)],
        popup_request: &mut Option<PopupRequest>,
        popup_id: Option<u64>,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, score) in similar_indices {
                    if let Some(seg) = self.project.segments.get(*idx) {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                let label_resp = ui.label(
                                    egui::RichText::new(format!(
                                        "[{}] (Score: {:.2})",
                                        idx + 1,
                                        score
                                    ))
                                    .strong(),
                                );
                                if !seg.comment.is_empty() {
                                    label_resp.on_hover_text(&seg.comment);
                                }
                            });

                            let scroll_id = match popup_id {
                                Some(id) => egui::Id::new(id).with(idx),
                                None => egui::Id::new(idx),
                            };
                            egui::ScrollArea::horizontal()
                                .id_salt(scroll_id)
                                .max_width(ui.available_width())
                                .show(ui, |ui| {
                                    if let Some(action) = ui::render_clickable_tokens(
                                        ui,
                                        &seg.tokens,
                                        &self.project.vocabulary,
                                        &self.project.vocabulary_comments,
                                        None,
                                        self.project.font_path.is_some(),
                                    ) {
                                        self.handle_ui_action(action, popup_request);
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

    fn handle_ui_action(&self, action: ui::UiAction, popup_request: &mut Option<PopupRequest>) {
        match action {
            ui::UiAction::ShowDefinition(word) if self.dictionary_mode => {
                *popup_request = Some(PopupRequest::Dictionary(word, PopupMode::Definition));
            }
            ui::UiAction::ShowReference(word) => {
                *popup_request = Some(PopupRequest::Dictionary(word, PopupMode::Reference));
            }
            _ => {}
        }
    }
}
