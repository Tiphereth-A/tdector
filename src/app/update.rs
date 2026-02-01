//! Main application update loop and eframe integration.
//!
//! This module implements the [`eframe::App`] trait for [`DecryptionApp`],
//! providing the main rendering loop that handles:
//!
//! - User input (keyboard shortcuts, menu actions)
//! - UI rendering (panels, dialogs, popups)
//! - State updates and cache invalidation
//! - Window management and title updates
//!
//! The update loop follows a careful ordering to minimize frame-to-frame
//! state inconsistencies and ensure proper cache invalidation.

use std::collections::{HashMap, HashSet};

use eframe::egui;

use crate::ui::{self, PopupMode};

use super::actions::AppAction;
use super::state::{DecryptionApp, PopupRequest};

impl DecryptionApp {
    /// Creates a new application instance and initializes fonts.
    ///
    /// This is called once by eframe during application startup. It sets up
    /// the custom "SentenceFont" family and returns a default application state.
    ///
    /// # Arguments
    ///
    /// * `cc` - The eframe creation context providing access to egui
    pub fn new(cc: &eframe::CreationContext<'_>) -> Box<dyn eframe::App> {
        Self::initialize_fonts(&cc.egui_ctx);
        Box::new(Self::default())
    }
}

impl eframe::App for DecryptionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut do_import = false;
        let mut do_open = false;
        let mut do_save = false;
        let mut do_export = false;
        let mut do_quit = false;
        let mut do_load_font = false;
        let mut do_add_word_formation_rule = false;

        self.handle_keyboard_shortcuts(
            ctx,
            &mut do_import,
            &mut do_open,
            &mut do_save,
            &mut do_export,
            &mut do_quit,
        );

        ui::render_menu_bar(
            ctx,
            !self.project.segments.is_empty(),
            || do_import = true,
            || do_open = true,
            || do_save = true,
            || do_export = true,
            || do_quit = true,
            || do_load_font = true,
            || do_add_word_formation_rule = true,
        );

        if !self.project.segments.is_empty() {
            self.render_filter_panel(ctx);
        }

        if !self.project.segments.is_empty()
            && self.cached_filtered_indices.is_empty()
            && self.filter_text.is_empty()
            && !self.filter_dirty
        {
            self.filter_dirty = true;
        }

        if self.filter_dirty {
            self.recalculate_filtered_indices();
            self.filter_dirty = false;
        }

        let total_items = self.cached_filtered_indices.len();
        let total_pages = self.calculate_total_pages(total_items);

        if self.current_page >= total_pages && total_pages > 0 {
            self.current_page = total_pages - 1;
        }

        self.process_actions(
            ctx,
            do_import,
            do_open,
            do_save,
            do_export,
            do_quit,
            do_load_font,
            do_add_word_formation_rule,
        );

        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.trigger_action(AppAction::Quit, ctx);
        }

        if let Some(new_page) =
            ui::render_pagination(ctx, self.current_page, total_pages, &mut self.page_size)
        {
            self.current_page = new_page;
        }

        self.render_error_dialog(ctx);
        self.render_confirmation_dialog(ctx);
        self.render_import_dialog(ctx);

        if self.lookups_dirty {
            self.recalculate_lookup_maps();
            self.lookups_dirty = false;
        }

        let (headword_lookup, usage_lookup) = self.lookup_cache.take();

        let mut any_changed = false;
        let mut popup_request = None;

        self.render_central_panel(ctx, &mut any_changed, &mut popup_request);

        if let Some(req) = popup_request.take() {
            match req {
                PopupRequest::Dictionary(word, mode) => match mode {
                    PopupMode::Definition => self.definition_popup = Some(word),
                    PopupMode::Reference => self.reference_popup = Some(word),
                },
                PopupRequest::Similar(idx) => {
                    self.compute_similar_segments(idx);
                }
                PopupRequest::WordMenu(word, sentence_idx, word_idx, cursor_pos) => {
                    self.word_menu_popup = Some((word, sentence_idx, word_idx, cursor_pos));
                }
            }
        }

        self.render_popups(ctx, &headword_lookup, &usage_lookup, &mut popup_request);

        self.render_pinned_popups(ctx, &headword_lookup, &usage_lookup, &mut popup_request);

        self.lookup_cache.restore(headword_lookup, usage_lookup);

        if let Some(req) = popup_request {
            match req {
                PopupRequest::Dictionary(word, mode) => match mode {
                    PopupMode::Definition => self.definition_popup = Some(word),
                    PopupMode::Reference => self.reference_popup = Some(word),
                },
                PopupRequest::Similar(idx) => {
                    self.compute_similar_segments(idx);
                }
                PopupRequest::WordMenu(word, sentence_idx, word_idx, cursor_pos) => {
                    self.word_menu_popup = Some((word, sentence_idx, word_idx, cursor_pos));
                }
            }
        }

        if any_changed {
            self.update_dirty_status(true, ctx);
            self.filter_dirty = true;
            self.lookups_dirty = true;
            self.tfidf_dirty = true;
            self.tfidf_cache.invalidate();
            ctx.request_repaint();
        }
    }
}

impl DecryptionApp {
    /// Processes keyboard shortcuts and sets action flags.
    ///
    /// Checks for standard application shortcuts (Cmd/Ctrl + key combinations)
    /// and sets the corresponding boolean flags to trigger actions.
    ///
    /// # Keyboard Shortcuts
    ///
    /// - `Cmd/Ctrl + I` - Import text file
    /// - `Cmd/Ctrl + O` - Open project
    /// - `Cmd/Ctrl + S` - Save project
    /// - `Cmd/Ctrl + E` - Export to Typst
    /// - `Cmd/Ctrl + Q` - Quit application
    fn handle_keyboard_shortcuts(
        &self,
        ctx: &egui::Context,
        do_import: &mut bool,
        do_open: &mut bool,
        do_save: &mut bool,
        do_export: &mut bool,
        do_quit: &mut bool,
    ) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::I)) {
            *do_import = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            *do_open = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            *do_save = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::E)) {
            *do_export = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Q)) {
            *do_quit = true;
        }
    }

    /// Calculates the total number of pages for pagination.
    ///
    /// Uses ceiling division to ensure all items are included even if
    /// the last page is not full.
    ///
    /// # Arguments
    ///
    /// * `total_items` - Total number of items to paginate
    ///
    /// # Returns
    ///
    /// The number of pages required, or 0 if there are no items.
    fn calculate_total_pages(&self, total_items: usize) -> usize {
        if total_items > 0 {
            total_items.div_ceil(self.page_size)
        } else {
            0
        }
    }

    /// Executes pending actions based on the provided flags.
    ///
    /// Processes all action flags in a specific order to ensure proper
    /// sequencing of operations (e.g., loading before saving).
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context
    /// * `do_import` through `do_load_font` - Boolean flags indicating which actions to execute
    fn process_actions(
        &mut self,
        ctx: &egui::Context,
        do_import: bool,
        do_open: bool,
        do_save: bool,
        do_export: bool,
        do_quit: bool,
        do_load_font: bool,
        do_add_word_formation_rule: bool,
    ) {
        if do_import {
            self.trigger_action(AppAction::Import, ctx);
        }
        if do_open {
            self.trigger_action(AppAction::Open, ctx);
        }
        if do_load_font {
            self.load_font_file(ctx);
        }
        if do_save {
            self.save_project(ctx);
        }
        if do_export {
            self.trigger_action(AppAction::Export, ctx);
        }
        if do_quit {
            self.trigger_action(AppAction::Quit, ctx);
        }
        if do_add_word_formation_rule {
            // Open dialog to create new word formation rule
            self.new_formation_rule_popup = Some(super::state::NewFormationRuleDialog {
                description: String::new(),
                rule_type: crate::models::FormationType::Derivation,
                command: "fn transform(word) { word }".to_string(),
                test_word: String::new(),
                preview: String::new(),
            });
        }
    }

    /// Rebuilds the headword and usage lookup indices.
    ///
    /// Creates two inverted indices:
    /// - **Headword lookup**: Maps each vocabulary word to segment indices where it appears
    /// - **Usage lookup**: Maps each token to segment indices where it's actually used
    ///
    /// These indices enable efficient dictionary popup rendering showing where
    /// words appear and how they're used in context.
    fn recalculate_lookup_maps(&mut self) {
        if self.project.segments.is_empty() {
            self.lookup_cache.invalidate();
            return;
        }

        let mut headmap: HashMap<String, Vec<usize>> = HashMap::new();
        let mut usagemap: HashMap<String, Vec<usize>> = HashMap::new();

        for (idx, segment) in self.project.segments.iter().enumerate() {
            if let Some(first) = segment.tokens.first() {
                headmap.entry(first.original.clone()).or_default().push(idx);
            }

            let mut seen = HashSet::new();
            for token in &segment.tokens {
                if seen.insert(&token.original) {
                    usagemap
                        .entry(token.original.clone())
                        .or_default()
                        .push(idx);
                }
            }
        }

        self.lookup_cache.restore(Some(headmap), Some(usagemap));
    }
}
