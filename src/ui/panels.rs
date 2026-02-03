//! UI panel rendering for filter controls and content display.
//!
//! This module renders:
//! - Top panel with filter input and sort controls
//! - Central panel with paginated segment list
//! - Empty state when no content is available

use eframe::egui;

use crate::consts::{
    colors::{FONT_DARK, FONT_LIGHT},
    ui::PANEL_SPACING,
};
use crate::enums::{DictionaryPopupType, PopupRequest, SortMode, UiAction};
use crate::libs::Project;
use crate::ui;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    /// Renders the top filter and sort control panel.
    ///
    /// Displays a horizontal panel containing:
    /// - A filter text input that searches across segments and tokens
    /// - A clear button (X) when filter text is present
    /// - A sort mode selector dropdown
    ///
    /// Changes to filter or sort settings reset to page 0 and trigger cache invalidation.
    pub(crate) fn render_filter_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("filter_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let text_color = if ui.visuals().dark_mode {
                    FONT_DARK
                } else {
                    FONT_LIGHT
                };

                ui.label(egui::RichText::new("Filter:").color(text_color));

                let font_id = if self.project.font_path.is_some() {
                    egui::FontId {
                        size: egui::TextStyle::Body.resolve(ui.style()).size,
                        family: egui::FontFamily::Name("SentenceFont".into()),
                    }
                } else {
                    egui::TextStyle::Body.resolve(ui.style())
                };

                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.filter_text)
                            .font(font_id)
                            .text_color(text_color),
                    )
                    .changed()
                {
                    self.current_page = 0;
                    self.filter_dirty = true;
                }
                if !self.filter_text.is_empty() && ui.button("X").clicked() {
                    self.filter_text.clear();
                    self.current_page = 0;
                    self.filter_dirty = true;
                }

                ui.separator();
                ui.label(egui::RichText::new("Sort by:").color(text_color));
                self.render_sort_selector(ui);
            });
        });
    }

    /// Renders the sort mode dropdown selector.
    ///
    /// Displays a combo box with all available sort modes. Selecting a new
    /// mode triggers re-filtering and resets pagination to the first page.
    fn render_sort_selector(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("sort_selector")
            .selected_text(self.sort_mode.display_text())
            .show_ui(ui, |ui| {
                for mode in SortMode::all() {
                    self.render_sort_option(ui, mode);
                }
            });
    }

    /// Renders a single sort option in the combo box.
    fn render_sort_option(&mut self, ui: &mut egui::Ui, mode: SortMode) {
        if ui
            .selectable_value(&mut self.sort_mode, mode, mode.display_text())
            .clicked()
        {
            self.current_page = 0;
            self.filter_dirty = true;
        }
    }

    /// Renders the central content panel with the current page of segments.
    ///
    /// Computes the slice of segments for the current page and renders each one.
    /// Handles user interactions with segments and tokens, updating state and
    /// setting popup requests as needed.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context
    /// * `any_changed` - Set to `true` if any segment content was modified
    /// * `popup_request` - Set to trigger popup display
    pub(crate) fn render_central_panel(
        &mut self,
        ctx: &egui::Context,
        any_changed: &mut bool,
        popup_request: &mut Option<PopupRequest>,
    ) {
        let total_items = self.cached_filtered_indices.len();
        let start = (self.current_page * self.page_size).min(total_items);
        let end = (start + self.page_size).min(total_items);
        let current_page_indices: &[usize] = if start < end {
            &self.cached_filtered_indices[start..end]
        } else {
            &[]
        };

        let use_custom_font = self.project.font_path.is_some();
        let filter_text = self.filter_text.as_str();

        let Project {
            segments,
            vocabulary,
            vocabulary_comments,
            formation_rules,
            ..
        } = &mut self.project;

        let mut new_filter = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            if current_page_indices.is_empty() {
                Self::render_empty_state(ui, filter_text);
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for &seg_idx in current_page_indices {
                        if let Some(segment) = segments.get_mut(seg_idx) {
                            let highlight = if filter_text.is_empty() {
                                None
                            } else {
                                Some(filter_text)
                            };
                            let action = ui::render_segment(
                                ui,
                                segment,
                                vocabulary,
                                vocabulary_comments,
                                seg_idx + 1,
                                highlight,
                                use_custom_font,
                                formation_rules,
                            );

                            match action {
                                UiAction::Changed => *any_changed = true,
                                UiAction::Filter(text) => {
                                    new_filter = Some(text.to_string());
                                }
                                UiAction::ShowWordMenu(word, word_idx) => {
                                    let cursor_pos =
                                        ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default();
                                    *popup_request = Some(PopupRequest::WordMenu(
                                        word.to_string(),
                                        seg_idx,
                                        word_idx,
                                        cursor_pos,
                                    ));
                                }
                                UiAction::ShowSentenceMenu(segment_idx) => {
                                    let cursor_pos =
                                        ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default();
                                    *popup_request =
                                        Some(PopupRequest::SentenceMenu(segment_idx, cursor_pos));
                                }
                                UiAction::ShowSimilar(seg_num) => {
                                    *popup_request = Some(PopupRequest::Similar(seg_num - 1));
                                }
                                UiAction::ShowDefinition(word) => {
                                    *popup_request = Some(PopupRequest::Dictionary(
                                        word.to_string(),
                                        DictionaryPopupType::Definition,
                                    ));
                                }
                                UiAction::ShowReference(word) => {
                                    *popup_request = Some(PopupRequest::Dictionary(
                                        word.to_string(),
                                        DictionaryPopupType::Reference,
                                    ));
                                }
                                UiAction::None => {}
                            }

                            ui.add_space(PANEL_SPACING);
                        }
                    }
                });
            }
        });

        if let Some(text) = new_filter {
            self.filter_text = text;
            self.current_page = 0;
            self.filter_dirty = true;
        }
    }

    /// Renders a centered message when no content is available.
    ///
    /// Shows different messages depending on whether the list is empty due to
    /// filtering ("No segments match filter") or because no project is loaded
    /// ("Import a text file or open a project to begin").
    fn render_empty_state(ui: &mut egui::Ui, filter_text: &str) {
        ui.centered_and_justified(|ui| {
            if filter_text.is_empty() {
                ui.label("Import a text file or open a project to begin.");
            } else {
                ui.label("No segments match filter.");
            }
        });
    }
}
