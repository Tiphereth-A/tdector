//! Popup window rendering coordination module.
//!
//! This module coordinates the rendering of various popup types.
//! The actual rendering implementations are in separate modules:
//! - dictionary_popups: Dictionary definition and reference popups
//! - similar_popups: Similar segments popups
//! - pinned_popups: Pinned popup management
//! - word_formation_popups: Word formation and menu popups

use std::collections::HashMap;

use eframe::egui;

use super::state::{DecryptionApp, PopupRequest};

impl DecryptionApp {
    /// Closes all active and pinned popups.
    pub(super) fn clear_popups(&mut self) {
        self.definition_popup = None;
        self.reference_popup = None;
        self.similar_popup = None;
        self.word_menu_popup = None;
        self.word_formation_popup = None;
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
        self.render_word_menu_popup(ctx, popup_request);
        self.render_word_formation_popup(ctx);
        self.render_new_formation_rule_popup(ctx);
    }
}
