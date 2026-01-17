//! File operations: load, save, export, and font management.

use eframe::egui;

use crate::io;

use super::actions::AppAction;
use super::state::DecryptionApp;

impl DecryptionApp {
    /// Opens a file dialog and prepares text content for import.
    pub(super) fn load_text_file(&mut self, _ctx: &egui::Context) {
        let path = match io::pick_text_file() {
            Some(p) => p,
            None => return,
        };

        match io::read_text_content(&path) {
            Ok((content, name, font_path)) => {
                self.pending_import = Some((content, name, font_path));
            }
            Err(e) => self.error_message = Some(e),
        }
    }

    /// Opens a project file and loads it into the application.
    pub(super) fn load_project(&mut self, ctx: &egui::Context) {
        let path = match io::pick_project_file() {
            Some(p) => p,
            None => return,
        };

        match io::load_project_file(&path) {
            Ok(project) => {
                if let Some(font_path) = &project.font_path {
                    self.load_custom_font(ctx, font_path);
                }
                self.project = project;
                self.current_path = Some(path);
                self.is_dirty = false;
                self.filter_dirty = true;
                self.lookups_dirty = true;
                self.filter_text.clear();
                self.clear_popups();
                self.update_title(ctx);
            }
            Err(e) => self.error_message = Some(e),
        }
    }

    /// Loads a custom font file and registers it as "SentenceFont".
    pub(super) fn load_custom_font(&self, ctx: &egui::Context, path_str: &str) {
        use std::path::Path;

        let path = Path::new(path_str);
        if let Ok(data) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();

            fonts.font_data.insert(
                "custom_font".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );

            // Define "SentenceFont" family with fallbacks
            let fallbacks = fonts
                .families
                .get(&egui::FontFamily::Proportional)
                .cloned()
                .unwrap_or_default();

            let mut custom_list = vec!["custom_font".to_owned()];
            custom_list.extend(fallbacks);

            fonts
                .families
                .insert(egui::FontFamily::Name("SentenceFont".into()), custom_list);

            ctx.set_fonts(fonts);
        }
    }

    /// Saves the project to the current path or prompts for a new location.
    pub(super) fn save_project(&mut self, ctx: &egui::Context) {
        let path = if let Some(p) = &self.current_path {
            Some(p.clone())
        } else {
            io::pick_save_file()
        };

        if let Some(path) = path {
            match io::save_project_file(&self.project, &path) {
                Ok(()) => {
                    self.current_path = Some(path);
                    self.is_dirty = false;
                    self.update_title(ctx);
                }
                Err(e) => self.error_message = Some(e),
            }
        }
    }

    /// Opens a file dialog to select and load a font file.
    pub(super) fn load_font_file(&mut self, ctx: &egui::Context) {
        if let Some(path) = io::pick_font_file() {
            if let Some(path_str) = path.to_str() {
                self.load_custom_font(ctx, path_str);
                self.project.font_path = Some(path_str.to_string());
                self.update_title(ctx);
            }
        }
    }

    /// Registers the "SentenceFont" family with default fallbacks.
    pub fn initialize_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();

        // Initialize "SentenceFont" with default proportional fonts
        let fallbacks = fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();

        fonts
            .families
            .insert(egui::FontFamily::Name("SentenceFont".into()), fallbacks);

        ctx.set_fonts(fonts);
    }

    /// Exports the project to Typst markup format.
    pub(super) fn export_typst(&mut self) {
        let path = match io::pick_typst_file() {
            Some(p) => p,
            None => return,
        };

        if let Err(e) = io::save_typst_file(&self.project, &path) {
            self.error_message = Some(e);
        }
    }

    /// Updates the window title to reflect project name and dirty state.
    pub(super) fn update_title(&self, ctx: &egui::Context) {
        let dirty_mark = if self.is_dirty { "*" } else { "" };
        let title = if self.project.project_name.is_empty() {
            format!("Text Decryption Helper{}", dirty_mark)
        } else {
            format!(
                "Text Decryption Helper - {}{}",
                self.project.project_name, dirty_mark
            )
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    /// Triggers an action, prompting for confirmation if there are unsaved changes.
    pub(super) fn trigger_action(&mut self, action: AppAction, ctx: &egui::Context) {
        if self.is_dirty {
            let msg = match action {
                AppAction::Quit => "You have unsaved changes. Are you sure you want to quit?",
                _ => "You have unsaved changes. Continue anyway?",
            };
            self.confirmation = Some((msg.to_string(), action));
            return;
        }

        self.execute_action(action, ctx);
    }

    /// Executes an action without confirmation checks.
    pub(super) fn execute_action(&mut self, action: AppAction, ctx: &egui::Context) {
        match action {
            AppAction::Import => self.load_text_file(ctx),
            AppAction::Open => self.load_project(ctx),
            AppAction::Export => self.export_typst(),
            AppAction::Quit => {
                self.is_dirty = false; // Prevent further checks
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}
