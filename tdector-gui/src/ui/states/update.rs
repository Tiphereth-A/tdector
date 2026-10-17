use std::cell::Cell;
use std::rc::Rc;

use eframe::egui;

use crate::enums::{AppAction, FormationType};
use crate::ui;

use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
    pub(crate) fn new_controlled(cc: &eframe::CreationContext<'_>) -> Self {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        Self::initialize_fonts(&cc.egui_ctx);
        let mut app = Self::default();
        app.control_data.native_position = cc
            .window_handle()
            .ok()
            .map(|handle| !matches!(handle.as_raw(), RawWindowHandle::Wayland(_)));
        app
    }

    #[allow(clippy::new_ret_no_self)]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Box<dyn eframe::App> {
        Self::new_with_dirty_flag(cc, Rc::new(Cell::new(false)))
    }

    /// Connect an instance-local dirty flag to a platform close/unload callback.
    pub fn new_with_dirty_flag(
        cc: &eframe::CreationContext<'_>,
        unsaved_changes: Rc<Cell<bool>>,
    ) -> Box<dyn eframe::App> {
        Self::initialize_fonts(&cc.egui_ctx);
        unsaved_changes.set(false);
        Box::new(Self {
            unsaved_changes,
            ..Self::default()
        })
    }
}

impl eframe::App for DecryptionApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.process_pending_file_operations(&ctx);
        self.sync_control_windows();
        let _control_scope = self.main_control_scope();

        let mut do_import = false;
        let mut do_open = false;
        let mut do_save = false;
        let mut do_export = false;
        let mut do_quit = false;
        let mut do_load_font = false;
        let mut do_add_word_formation_rule = false;

        self.handle_keyboard_shortcuts(
            &ctx,
            &mut do_import,
            &mut do_open,
            &mut do_save,
            &mut do_export,
            &mut do_quit,
        );

        ui::render_menu_bar(
            ui,
            !self.session.project().segments.is_empty(),
            || do_import = true,
            || do_open = true,
            || do_save = true,
            || do_export = true,
            || do_quit = true,
            || do_load_font = true,
            || do_add_word_formation_rule = true,
        );

        if !self.session.project().segments.is_empty() {
            self.render_filter_panel(ui);
        }

        if !self.session.project().segments.is_empty()
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
            &ctx,
            do_import,
            do_open,
            do_save,
            do_export,
            do_quit,
            do_load_font,
            do_add_word_formation_rule,
        );

        if ctx.input(|i| i.viewport().close_requested()) && self.session.is_dirty() && !self.closing
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.trigger_action(AppAction::Quit, &ctx);
        }

        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        let external_jump = Some(&mut self.control_data.jump_page);
        #[cfg(not(all(feature = "gui-control", not(target_arch = "wasm32"))))]
        let external_jump = None;
        if let Some(new_page) = ui::render_pagination(
            ui,
            self.current_page,
            total_pages,
            &mut self.page_size,
            external_jump,
        ) {
            self.current_page = new_page;
        }

        self.sync_control_windows();
        self.render_error_dialog(&ctx);
        self.render_confirmation_dialog(&ctx);
        self.render_import_dialog(&ctx);
        self.sync_control_windows();
        self.render_custom_tokenization_popup(&ctx);

        if self.filter_dirty {
            self.recalculate_filtered_indices();
            self.filter_dirty = false;
        }

        let mut popup_request = None;

        self.render_central_panel(ui, &mut popup_request);

        if let Some(req) = popup_request.take() {
            self.handle_popup_request(req, &ctx);
        }

        let (headword_lookup, usage_lookup) = self.session.lookup_maps();

        self.sync_control_windows();
        self.render_popups(&ctx, &headword_lookup, &usage_lookup, &mut popup_request);

        self.sync_control_windows();
        self.render_pinned_popups(&ctx, &headword_lookup, &usage_lookup, &mut popup_request);

        if let Some(req) = popup_request {
            self.handle_popup_request(req, &ctx);
        }
        #[cfg(all(feature = "gui-control", not(target_arch = "wasm32")))]
        self.observe_control_frame(&ctx);
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

    #[allow(clippy::too_many_arguments)]
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
            if !self.open_from_mouse(ctx, "new-formation-rule", None, None, None) {
                self.new_formation_rule_popup = Some(super::state::NewFormationRuleDialog {
                    description: String::new(),
                    rule_type: FormationType::Derivation,
                    command: "fn transform(word) { word }".to_string(),
                    test_word: String::new(),
                    preview: String::new(),
                });
            }
        }
    }
}
