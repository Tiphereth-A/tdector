//! Filter/sort panel and main content area rendering.

use eframe::egui;

use crate::models::Project;
use crate::ui::{self, constants};

use super::actions::SortMode;
use super::state::{DecryptionApp, PopupRequest};

impl DecryptionApp {
    /// Renders the top filter/sort controls panel.
    pub(super) fn render_filter_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("filter_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let text_color = if ui.visuals().dark_mode {
                    crate::ui::colors::FONT_DARK
                } else {
                    crate::ui::colors::FONT_LIGHT
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

    /// Renders the sort mode combo box selector.
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

    /// Renders the central panel with segment content.
    pub(super) fn render_central_panel(
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
        let dictionary_mode = self.dictionary_mode;

        let Project {
            segments,
            vocabulary,
            ..
        } = &mut self.project;

        let mut new_filter = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            if current_page_indices.is_empty() {
                Self::render_empty_state(ui, &filter_text);
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
                                seg_idx + 1,
                                highlight,
                                dictionary_mode,
                                use_custom_font,
                            );

                            match action {
                                ui::UiAction::Changed => *any_changed = true,
                                ui::UiAction::Filter(text) => {
                                    new_filter = Some(text);
                                }
                                ui::UiAction::ShowSimilar(seg_num) => {
                                    *popup_request = Some(PopupRequest::Similar(seg_num - 1));
                                }
                                ui::UiAction::ShowDefinition(word) => {
                                    *popup_request = Some(PopupRequest::Dictionary(
                                        word,
                                        ui::PopupMode::Definition,
                                    ));
                                }
                                ui::UiAction::ShowReference(word) => {
                                    *popup_request = Some(PopupRequest::Dictionary(
                                        word,
                                        ui::PopupMode::Reference,
                                    ));
                                }
                                ui::UiAction::None => {}
                            }

                            ui.add_space(constants::PANEL_SPACING);
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

    /// Renders the empty state message when no segments are displayed.
    fn render_empty_state(ui: &mut egui::Ui, filter_text: &str) {
        ui.centered_and_justified(|ui| {
            if filter_text.is_empty() {
                ui.label("Import a text file to begin.");
            } else {
                ui.label("No segments match filter.");
            }
        });
    }
}
