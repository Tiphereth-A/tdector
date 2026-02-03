//! Popup window rendering coordination module.
//!
//! This module coordinates the rendering of various popup types.
//! The actual rendering implementations are in sibling modules within `ui::popups`:
//! - `dictionary`: Dictionary definition and reference popups
//! - `similar`: Similar segments popups
//! - `pinned`: Pinned popup management
//! - `word_formation`: Word formation and menu popups
//! - `comments`: Comment editing popups
//! - `menu_word`: Word context menu
//! - `menu_sentence`: Sentence context menu

use std::collections::HashMap;

use eframe::egui;

use crate::enums::PopupRequest;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    /// Closes all active and pinned popups.
    pub(crate) fn clear_popups(&mut self) {
        self.definition_popup = None;
        self.reference_popup = None;
        self.similar_popup = None;
        self.word_menu_popup = None;
        self.sentence_menu_popup = None;
        self.word_formation_popup = None;
        self.update_comment_popup = None;
        self.update_sentence_comment_popup = None;
        self.pinned_popups.clear();
    }

    /// Renders the currently active unpinned popups.
    pub(crate) fn render_popups(
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
        self.render_sentence_menu_popup(ctx, popup_request);
        self.render_word_formation_popup(ctx);
        self.render_new_formation_rule_popup(ctx);
        self.render_update_comment_popup(ctx);
        self.render_update_sentence_comment_popup(ctx);
    }
}
