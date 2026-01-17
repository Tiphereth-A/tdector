//! Main render loop and eframe [`App`] implementation.

use std::collections::{HashMap, HashSet};

use eframe::egui;

use crate::ui::{self, PopupMode};

use super::actions::AppAction;
use super::state::{DecryptionApp, PopupRequest};

impl DecryptionApp {
    /// Creates the application instance for eframe.
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

        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(
            ctx,
            &mut do_import,
            &mut do_open,
            &mut do_save,
            &mut do_export,
            &mut do_quit,
        );

        // Render menu bar
        ui::render_menu_bar(
            ctx,
            &mut self.dictionary_mode,
            !self.project.segments.is_empty(),
            || do_import = true,
            || do_open = true,
            || do_save = true,
            || do_export = true,
            || do_quit = true,
            || do_load_font = true,
        );

        // Render filter panel
        if !self.project.segments.is_empty() {
            self.render_filter_panel(ctx);
        }

        // Safety check: Ensure indices are initialized if project is loaded
        if !self.project.segments.is_empty()
            && self.cached_filtered_indices.is_empty()
            && self.filter_text.is_empty()
            && !self.filter_dirty
        {
            self.filter_dirty = true;
        }

        // Update filtered indices if needed
        if self.filter_dirty {
            self.recalculate_filtered_indices();
            self.filter_dirty = false;
        }

        let total_items = self.cached_filtered_indices.len();
        let total_pages = self.calculate_total_pages(total_items);

        if self.current_page >= total_pages && total_pages > 0 {
            self.current_page = total_pages - 1;
        }

        // Process actions
        self.process_actions(
            ctx,
            do_import,
            do_open,
            do_save,
            do_export,
            do_quit,
            do_load_font,
        );

        // Handle window close
        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.trigger_action(AppAction::Quit, ctx);
        }

        // Render pagination
        if let Some(new_page) =
            ui::render_pagination(ctx, self.current_page, total_pages, &mut self.page_size)
        {
            self.current_page = new_page;
        }

        // Render dialogs
        self.render_error_dialog(ctx);
        self.render_confirmation_dialog(ctx);
        self.render_import_dialog(ctx);

        // Update lookup maps if needed
        if self.lookups_dirty {
            self.recalculate_lookup_maps();
            self.lookups_dirty = false;
        }

        // Temporarily take ownership of maps to avoid borrow conflicts
        let headword_lookup = self.cached_headword_lookup.take();
        let usage_lookup = self.cached_usage_lookup.take();

        let mut any_changed = false;
        let mut popup_request = None;

        let current_page_indices = self.get_current_page_indices();

        // Render main content
        self.render_central_panel(
            ctx,
            &current_page_indices,
            &mut any_changed,
            &mut popup_request,
        );

        // Process popup requests
        if let Some(req) = popup_request.take() {
            match req {
                PopupRequest::Dictionary(word, mode) => match mode {
                    PopupMode::Definition => self.definition_popup = Some(word),
                    PopupMode::Reference => self.reference_popup = Some(word),
                },
                PopupRequest::Similar(idx) => {
                    self.compute_similar_segments(idx);
                }
            }
        }

        // Render popup logic
        self.render_popups(ctx, &headword_lookup, &usage_lookup, &mut popup_request);

        // Process pinned popups
        self.render_pinned_popups(ctx, &headword_lookup, &usage_lookup, &mut popup_request);

        // Restore lookup maps
        self.cached_headword_lookup = headword_lookup;
        self.cached_usage_lookup = usage_lookup;

        // Start processing the request
        if let Some(req) = popup_request {
            match req {
                PopupRequest::Dictionary(word, mode) => match mode {
                    PopupMode::Definition => self.definition_popup = Some(word),
                    PopupMode::Reference => self.reference_popup = Some(word),
                },
                PopupRequest::Similar(idx) => {
                    self.compute_similar_segments(idx);
                }
            }
        }

        // Mark dirty if changed
        if any_changed {
            if !self.is_dirty {
                self.is_dirty = true;
                self.update_title(ctx);
            }
            // Content changed, so we must re-filter and re-lookup
            self.filter_dirty = true;
            self.lookups_dirty = true;
            ctx.request_repaint();
        }
    }
}

// =============================================================================
// Helper Methods for Update Loop
// =============================================================================

impl DecryptionApp {
    /// Handles keyboard shortcuts for common actions.
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

    /// Calculates total number of pages based on item count.
    fn calculate_total_pages(&self, total_items: usize) -> usize {
        if total_items > 0 {
            (total_items + self.page_size - 1) / self.page_size
        } else {
            0
        }
    }

    /// Gets the slice of indices for the current page.
    fn get_current_page_indices(&self) -> Vec<usize> {
        let total_items = self.cached_filtered_indices.len();
        let start = (self.current_page * self.page_size).min(total_items);
        let end = (start + self.page_size).min(total_items);
        if start < end {
            self.cached_filtered_indices[start..end].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Processes all pending actions.
    fn process_actions(
        &mut self,
        ctx: &egui::Context,
        do_import: bool,
        do_open: bool,
        do_save: bool,
        do_export: bool,
        do_quit: bool,
        do_load_font: bool,
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
    }

    /// Recalculates lookup maps for headword and usage indexing.
    fn recalculate_lookup_maps(&mut self) {
        if self.project.segments.is_empty() {
            self.cached_headword_lookup = None;
            self.cached_usage_lookup = None;
            return;
        }

        let mut headmap: HashMap<String, Vec<usize>> = HashMap::new();
        let mut usagemap: HashMap<String, Vec<usize>> = HashMap::new();

        for (idx, segment) in self.project.segments.iter().enumerate() {
            if let Some(first) = segment.tokens.first() {
                if let Some(list) = headmap.get_mut(&first.original) {
                    list.push(idx);
                } else {
                    headmap.insert(first.original.clone(), vec![idx]);
                }
            }

            let mut seen = HashSet::new();
            for token in &segment.tokens {
                if seen.insert(&token.original) {
                    if let Some(list) = usagemap.get_mut(&token.original) {
                        list.push(idx);
                    } else {
                        usagemap.insert(token.original.clone(), vec![idx]);
                    }
                }
            }
        }

        self.cached_headword_lookup = Some(headmap);
        self.cached_usage_lookup = Some(usagemap);
    }
}
