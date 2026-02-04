use std::collections::{HashMap, HashSet};

use eframe::egui;

use crate::enums::{AppAction, DictionaryPopupType, FormationType, PopupRequest};
use crate::libs::project::load_project_from_json;
use crate::ui;

use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Box<dyn eframe::App> {
        Self::initialize_fonts(&cc.egui_ctx);
        Box::new(Self::default())
    }
}

impl eframe::App for DecryptionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_pending_file_operations(ctx);

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
                    DictionaryPopupType::Definition => self.definition_popup = Some(word),
                    DictionaryPopupType::Reference => self.reference_popup = Some(word),
                },
                PopupRequest::Similar(idx) => {
                    self.compute_similar_segments(idx);
                }
                PopupRequest::WordMenu(word, sentence_idx, word_idx, cursor_pos) => {
                    self.word_menu_popup = Some((word, sentence_idx, word_idx, cursor_pos));
                }
                PopupRequest::SentenceMenu(sentence_idx, cursor_pos) => {
                    self.sentence_menu_popup = Some((sentence_idx, cursor_pos));
                }
                PopupRequest::Filter(text) => {
                    self.filter_text = text;
                    self.current_page = 0;
                    self.filter_dirty = true;
                }
            }
        }

        self.render_popups(ctx, &headword_lookup, &usage_lookup, &mut popup_request);

        self.render_pinned_popups(ctx, &headword_lookup, &usage_lookup, &mut popup_request);

        self.lookup_cache.restore(headword_lookup, usage_lookup);

        if let Some(req) = popup_request {
            match req {
                PopupRequest::Dictionary(word, mode) => match mode {
                    DictionaryPopupType::Definition => self.definition_popup = Some(word),
                    DictionaryPopupType::Reference => self.reference_popup = Some(word),
                },
                PopupRequest::Similar(idx) => {
                    self.compute_similar_segments(idx);
                }
                PopupRequest::WordMenu(word, sentence_idx, word_idx, cursor_pos) => {
                    self.word_menu_popup = Some((word, sentence_idx, word_idx, cursor_pos));
                }
                PopupRequest::SentenceMenu(sentence_idx, cursor_pos) => {
                    self.sentence_menu_popup = Some((sentence_idx, cursor_pos));
                }
                PopupRequest::Filter(text) => {
                    self.filter_text = text;
                    self.current_page = 0;
                    self.filter_dirty = true;
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

    fn calculate_total_pages(&self, total_items: usize) -> usize {
        if total_items > 0 {
            total_items.div_ceil(self.page_size)
        } else {
            0
        }
    }

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
            self.new_formation_rule_popup = Some(super::state::NewFormationRuleDialog {
                description: String::new(),
                rule_type: FormationType::Derivation,
                command: "fn transform(word) { word }".to_string(),
                test_word: String::new(),
                preview: String::new(),
            });
        }
    }

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

    fn process_pending_file_operations(&mut self, ctx: &egui::Context) {
        if let Ok(mut guard) = self.pending_text_file.try_lock()
            && let Some(result) = guard.take()
        {
            match result {
                Ok((content, name)) => {
                    self.pending_import = Some((content, name));
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to load text file: {e}"));
                }
            }
        }

        let project_result = if let Ok(mut guard) = self.pending_project_file.try_lock() {
            guard.take()
        } else {
            None
        };

        if let Some(result) = project_result {
            match result {
                Ok((content, name, full_path)) => {
                    let value: serde_json::Value = match serde_json::from_str(&content) {
                        Ok(parsed) => parsed,
                        Err(e) => {
                            self.error_message = Some(format!("Failed to parse project file: {e}"));
                            return;
                        }
                    };

                    match load_project_from_json(value) {
                        Ok(project) => {
                            self.project = project;
                            self.current_path = None;

                            self.project_filename = full_path.or(Some(name));
                            self.filter_dirty = true;
                            self.lookups_dirty = true;
                            self.tfidf_dirty = true;
                            self.filter_text.clear();
                            self.clear_popups();
                            self.update_dirty_status(false, ctx);
                        }
                        Err(e) => {
                            self.error_message = Some(e);
                        }
                    }
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to load project file: {e}"));
                }
            }
        }

        let save_result = if let Ok(mut guard) = self.pending_save_result.try_lock() {
            guard.take()
        } else {
            None
        };

        if let Some(result) = save_result {
            match result {
                Ok(()) => {
                    self.update_dirty_status(false, ctx);
                }
                Err(e) => {
                    if !e.contains("cancelled") && !e.contains("Cancelled") {
                        self.error_message = Some(format!("Failed to save project: {e}"));
                    }
                }
            }
        }

        let font_result = if let Ok(mut guard) = self.pending_font_file.try_lock() {
            guard.take()
        } else {
            None
        };

        if let Some(result) = font_result {
            match result {
                Ok((data, name)) => {
                    self.load_custom_font_from_bytes(ctx, data, &name);
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to load font file: {e}"));
                }
            }
        }
    }
}
