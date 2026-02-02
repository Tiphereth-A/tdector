//! File operations for project and font management.
//!
//! This module handles all file I/O operations including:
//! - Loading and saving projects
//! - Importing text files
//! - Exporting to Typst format
//! - Custom font loading and registration

use eframe::egui;

use crate::io;

use super::actions::AppAction;
use super::state::DecryptionApp;

impl DecryptionApp {
    /// Opens a file picker dialog to select a text file for import.
    ///
    /// Reads the text content and prepares it for tokenization by storing it
    /// in the pending import state. The user will then be prompted to select
    /// a tokenization strategy (word-based or character-based).
    pub(super) fn load_text_file(&mut self, _ctx: &egui::Context) {
        let path = match io::pick_text_file() {
            Some(p) => p,
            None => return,
        };

        match io::read_text_content(&path) {
            Ok((content, name)) => {
                self.pending_import = Some((content, name));
            }
            Err(e) => self.error_message = Some(e),
        }
    }

    /// Opens a file picker dialog to select and load a project file.
    ///
    /// Loads the project from JSON, automatically handling format migration
    /// from legacy formats. If the project includes a custom font, it's loaded
    /// and registered with the UI framework.
    ///
    /// On successful load, all caches are invalidated and the UI is reset to
    /// display the newly loaded project.
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
                self.filter_dirty = true;
                self.lookups_dirty = true;
                self.tfidf_dirty = true;
                self.filter_text.clear();
                self.clear_popups();
                self.update_dirty_status(false, ctx);
            }
            Err(e) => self.error_message = Some(e),
        }
    }

    /// Loads a custom font file and registers it as "SentenceFont".
    ///
    /// Reads the font file into memory and registers it with egui's font system
    /// under the name "SentenceFont". This family is configured with fallbacks
    /// to the default proportional fonts for characters not present in the custom font.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context
    /// * `path_str` - Absolute path to the font file (`.ttf`, `.otf`, or `.ttc`)
    pub(super) fn load_custom_font(&self, ctx: &egui::Context, path_str: &str) {
        use std::path::Path;

        let path = Path::new(path_str);
        if let Ok(data) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();

            fonts.font_data.insert(
                "custom_font".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );

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

    /// Saves the current project to disk.
    ///
    /// If the project has been saved before (`current_path` is set), saves to
    /// that location. Otherwise, opens a file picker dialog to select a save location.
    ///
    /// On successful save, clears the dirty flag and updates the window title.
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
                    self.update_dirty_status(false, ctx);
                }
                Err(e) => self.error_message = Some(e),
            }
        }
    }

    /// Opens a file picker dialog to select and load a custom font.
    ///
    /// Allows the user to manually load a font file for the project. The font
    /// is immediately registered with the UI framework and the path is stored
    /// in the project for persistence.
    pub(super) fn load_font_file(&mut self, ctx: &egui::Context) {
        if let Some(path) = io::pick_font_file()
            && let Some(path_str) = path.to_str()
        {
            self.load_custom_font(ctx, path_str);
            self.project.font_path = Some(path_str.to_string());
            self.update_title(ctx);
        }
    }

    /// Initializes the "SentenceFont" family with default fallbacks.
    ///
    /// Called during application startup to register the "SentenceFont" family.
    /// Initially, this family contains the default proportional fonts. When a
    /// custom font is loaded, it's added to the front of this family.
    ///
    /// This approach ensures that:
    /// 1. The "SentenceFont" family always exists (preventing lookup errors)
    /// 2. Missing glyphs fall back to default fonts gracefully
    pub fn initialize_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();

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

    /// Opens a file picker and exports the project to Typst format.
    ///
    /// Converts the project to Typst markup suitable for professional typesetting
    /// of interlinear glossed text. The exported file includes:
    /// - Project title
    /// - Custom font specification (if present)
    /// - All segments with aligned glosses and translations
    pub(super) fn export_typst(&mut self) {
        let path = match io::pick_typst_file() {
            Some(p) => p,
            None => return,
        };

        if let Err(e) = io::save_typst_file(&self.project, &path) {
            self.error_message = Some(e);
        }
    }

    /// Updates the window title to reflect project state.
    ///
    /// The title shows:
    /// - The project name (or "Text Decryption Helper" if unnamed)
    /// - An asterisk (*) suffix if there are unsaved changes
    ///
    /// This provides immediate visual feedback about save state.
    pub(super) fn update_title(&self, ctx: &egui::Context) {
        let dirty_mark = if self.is_dirty { "*" } else { "" };
        let title = if self.project.project_name.is_empty() {
            format!("Text Decryption Helper{dirty_mark}")
        } else {
            format!(
                "Text Decryption Helper - {}{dirty_mark}",
                self.project.project_name
            )
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    /// Reset dirty mark and updates the window title.
    pub(super) fn update_dirty_status(&mut self, new_flag: bool, ctx: &egui::Context) {
        if self.is_dirty != new_flag {
            self.is_dirty = new_flag;
            self.update_title(ctx);
        }
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
                self.is_dirty = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Find top 5 words from vocabulary that start with or contain the given prefix.
    pub(super) fn find_related_words(&self, prefix: &str) -> Vec<String> {
        if prefix.is_empty() {
            return Vec::new();
        }

        let prefix_lower = prefix.to_lowercase();
        let mut matches: Vec<String> = self
            .project
            .vocabulary
            .keys()
            .filter(|word| {
                let word_lower = word.to_lowercase();
                word_lower.starts_with(&prefix_lower) || word_lower.contains(&prefix_lower)
            })
            .take(5)
            .cloned()
            .collect();

        matches.sort_by(|a, b| {
            let a_lower = a.to_lowercase();
            let b_lower = b.to_lowercase();
            let a_starts = a_lower.starts_with(&prefix_lower);
            let b_starts = b_lower.starts_with(&prefix_lower);

            match (a_starts, b_starts) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a_lower.cmp(&b_lower),
            }
        });

        matches
    }
}
