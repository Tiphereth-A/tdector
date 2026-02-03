//! File operations for project and font management.
//!
//! This module handles all file I/O operations including:
//!
//! - Loading and saving projects
//! - Importing text files for tokenization
//! - Exporting translated documents to Typst format
//! - Loading and registering custom fonts
//!
//! Uses unified async rfd API to eliminate duplication between desktop and WASM

use eframe::egui;

use crate::consts::domain::DEFAULT_RELATED_WORDS_COUNT;
use crate::enums::{AppAction, AppError, FileType};
use crate::io;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    /// Opens a file picker dialog to select a text file for import.
    ///
    /// Reads the text content asynchronously and prepares it for tokenization by storing it
    /// in the pending import state. The user will then be prompted to select
    /// a tokenization strategy (word-based or character-based).
    pub(crate) fn load_text_file(&mut self, _ctx: &egui::Context) {
        let pending = self.pending_text_file.clone();
        io::FileIO::spawn(async move {
            let file_type = FileType::Text;
            let result =
                io::FileIO::pick_file(file_type.filter_name(), file_type.extensions()).await;
            let decoded = result
                .and_then(|(bytes, filename, _path)| {
                    String::from_utf8(bytes)
                        .map(|content| (content, filename))
                        .map_err(|e| AppError::IoError(format!("Failed to decode file: {e}")))
                })
                .map_err(|e| e.to_string());
            let mut guard = pending.lock().unwrap();
            *guard = Some(decoded);
        });
    }

    /// Opens a file picker dialog to select and load a project file.
    ///
    /// Loads the project from JSON asynchronously, automatically handling format migration
    /// from legacy formats. If the project includes a custom font, it's loaded
    /// and registered with the UI framework.
    ///
    /// On successful load, all caches are invalidated and the UI is reset to
    /// display the newly loaded project.
    pub(crate) fn load_project(&mut self, _ctx: &egui::Context) {
        let pending = self.pending_project_file.clone();
        io::FileIO::spawn(async move {
            let file_type = FileType::Json;
            let result =
                io::FileIO::pick_file(file_type.filter_name(), file_type.extensions()).await;
            let decoded = result
                .and_then(|(bytes, filename, full_path)| {
                    String::from_utf8(bytes)
                        .map(|content| (content, filename, full_path))
                        .map_err(|e| AppError::IoError(format!("Failed to decode file: {e}")))
                })
                .map_err(|e| e.to_string());
            let mut guard = pending.lock().unwrap();
            *guard = Some(decoded);
        });
    }

    /// Saves the current project to disk or downloads in browser.
    ///
    /// Converts the project to JSON format and uses async file dialog to save.
    /// On desktop, if the project was loaded from a file, saves directly to that file.
    /// Otherwise, shows a save dialog. On successful save, clears the dirty flag and updates the window title.
    pub(crate) fn save_project(&mut self, _ctx: &egui::Context) {
        match io::convert_to_saved_project(&self.project) {
            Ok(saved_project) => {
                let formatter = io::json_formatter::Formatter::new();
                let mut buf = Vec::new();
                let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
                match serde::Serialize::serialize(&saved_project, &mut serializer) {
                    Ok(()) => {
                        let json_content =
                            String::from_utf8(buf).unwrap_or_else(|_| String::from("{}"));
                        let json_bytes = json_content.into_bytes();

                        // Desktop: If we have a stored filename, save directly to that file
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(ref filename) = self.project_filename {
                            use std::path::PathBuf;
                            let path = PathBuf::from(filename);
                            let pending = self.pending_save_result.clone();
                            io::FileIO::spawn(async move {
                                let result = io::FileIO::save_file_to_path(&json_bytes, &path)
                                    .await
                                    .map_err(|e| e.to_string());
                                let mut guard = pending.lock().unwrap();
                                *guard = Some(result);
                            });
                            return;
                        }

                        // Use stored filename if available, otherwise generate from project name
                        let filename = if let Some(ref stored_filename) = self.project_filename {
                            stored_filename.clone()
                        } else if self.project.project_name.is_empty() {
                            "project.json".to_string()
                        } else {
                            format!("{}.json", self.project.project_name)
                        };
                        let pending = self.pending_save_result.clone();
                        io::FileIO::spawn(async move {
                            let result =
                                io::FileIO::save_file(&json_bytes, &filename, "JSON", &["json"])
                                    .await
                                    .map_err(|e| e.to_string());
                            let mut guard = pending.lock().unwrap();
                            *guard = Some(result);
                        });
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Failed to serialize project: {e}"));
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to convert project: {e}"));
            }
        }
    }

    /// Opens a file picker dialog to select and load a custom font.
    ///
    /// Allows the user to manually load a font file for the project. The font
    /// is immediately registered with the UI framework asynchronously.
    pub(crate) fn load_font_file(&mut self, _ctx: &egui::Context) {
        let pending = self.pending_font_file.clone();
        io::FileIO::spawn(async move {
            let file_type = FileType::Font;
            let result =
                io::FileIO::pick_file(file_type.filter_name(), file_type.extensions()).await;
            let converted = result
                .map(|(bytes, filename, _path)| (bytes, filename))
                .map_err(|e| e.to_string());
            let mut guard = pending.lock().unwrap();
            *guard = Some(converted);
        });
    }

    /// Loads a custom font from binary data.
    pub(crate) fn load_custom_font_from_bytes(
        &mut self,
        ctx: &egui::Context,
        data: Vec<u8>,
        font_name: &str,
    ) {
        io::register_custom_font(ctx, data);

        // Store font name in project (not the full data, just a reference)
        self.project.font_path = Some(font_name.to_string());
        self.update_title(ctx);
    }

    /// Initializes the "`SentenceFont`" family with default fallbacks.
    ///
    /// Called during application startup to register the "`SentenceFont`" family.
    /// Initially, this family contains the default proportional fonts. When a
    /// custom font is loaded, it's added to the front of this family.
    ///
    /// This approach ensures that:
    /// 1. The "`SentenceFont`" family always exists (preventing lookup errors)
    /// 2. Missing glyphs fall back to default fonts gracefully
    pub fn initialize_fonts(ctx: &egui::Context) {
        io::initialize_fonts(ctx);
    }

    /// Opens a file picker and exports the project to Typst format.
    ///
    /// Converts the project to Typst markup suitable for professional typesetting
    /// of interlinear glossed text asynchronously. The exported file includes:
    /// - Project title
    /// - Custom font specification (if present)
    /// - All segments with aligned glosses and translations
    pub(crate) fn export_typst(&mut self) {
        let content = io::generate_typst_content(&self.project);
        let filename = format!(
            "{}.typ",
            if self.project.project_name.is_empty() {
                "export".to_string()
            } else {
                self.project.project_name.clone()
            }
        );
        let content_bytes = content.into_bytes();
        io::FileIO::spawn(async move {
            let file_type = FileType::Typst;
            let _result = io::FileIO::save_file(
                &content_bytes,
                &filename,
                file_type.filter_name(),
                file_type.extensions(),
            )
            .await;
        });
    }

    /// Updates the window title to reflect project state.
    ///
    /// The title shows:
    /// - The project name (or "Text Decryption Helper" if unnamed)
    /// - An asterisk (*) suffix if there are unsaved changes
    ///
    /// This provides immediate visual feedback about save state.
    pub(crate) fn update_title(&self, ctx: &egui::Context) {
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
    pub(crate) fn update_dirty_status(&mut self, new_flag: bool, ctx: &egui::Context) {
        if self.is_dirty != new_flag {
            self.is_dirty = new_flag;
            self.update_title(ctx);

            // Update WASM dirty flag for beforeunload handler
            #[cfg(target_arch = "wasm32")]
            crate::set_app_dirty(new_flag);
        }
    }

    /// Triggers an action, prompting for confirmation if there are unsaved changes.
    pub(crate) fn trigger_action(&mut self, action: AppAction, ctx: &egui::Context) {
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
    pub(crate) fn execute_action(&mut self, action: AppAction, ctx: &egui::Context) {
        match action {
            AppAction::Import => self.load_text_file(ctx),
            AppAction::Open => self.load_project(ctx),
            AppAction::Export => self.export_typst(),
            AppAction::Quit => {
                self.update_dirty_status(false, ctx);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Find top 5 words from vocabulary that start with or contain the given prefix.
    pub(crate) fn find_related_words(&self, prefix: &str) -> Vec<String> {
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
            .take(DEFAULT_RELATED_WORDS_COUNT)
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

/// Common font loading logic shared between platforms
pub fn register_custom_font(ctx: &egui::Context, data: Vec<u8>) {
    use std::sync::Arc;

    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "custom_font".to_owned(),
        Arc::new(egui::FontData::from_owned(data)),
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

/// Initialize the `SentenceFont` family with default fallbacks
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
