//! Pinned popup rendering.

use std::collections::HashMap;

use eframe::egui;

use crate::ui::{PopupMode, constants};

use super::actions::PinnedPopup;
use super::state::{DecryptionApp, PopupRequest};

impl DecryptionApp {
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
}
