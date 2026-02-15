use eframe::egui;

use crate::consts::ui::{POPUP_SIMILAR_HEIGHT, POPUP_WIDTH};
use crate::enums::PopupRequest;
use crate::libs::similarity_token::SimilarToken;
use crate::ui::popup_utils::create_popup_title;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    pub(super) fn render_similar_tokens_popup(
        &mut self,
        ctx: &egui::Context,
        _popup_request: &mut Option<PopupRequest>,
    ) {
        let mut should_close = false;

        if let Some((target_word, similar_tokens)) = self.similar_tokens_popup.as_ref() {
            let mut open = true;
            let title = create_popup_title(
                "Similar tokens: ",
                target_word,
                self.project.font_path.is_some(),
            );
            egui::Window::new(title)
                .id(egui::Id::new("similar_tokens_popup"))
                .open(&mut open)
                .default_width(POPUP_WIDTH)
                .default_height(POPUP_SIMILAR_HEIGHT)
                .show(ctx, |ui| {
                    let count = similar_tokens.len();
                    ui.label(format!("Showing {count} most similar tokens(s)"));
                    ui.separator();
                    self.render_similar_tokens_content(ui, similar_tokens);
                });

            if !open {
                should_close = true;
            }
        }

        if should_close {
            self.similar_tokens_popup = None;
        }
    }

    pub(super) fn render_similar_tokens_content(
        &self,
        ui: &mut egui::Ui,
        similar_tokens: &[SimilarToken],
    ) {
        let custom_font_id = egui::FontId {
            size: egui::TextStyle::Body.resolve(ui.style()).size,
            family: egui::FontFamily::Name("SentenceFont".into()),
        };
        let has_custom_font = self.project.font_path.is_some();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for similar_token in similar_tokens {
                    ui.horizontal(|ui| {
                        // Show the word
                        let word_text = if has_custom_font {
                            egui::RichText::new(&similar_token.word).font(custom_font_id.clone())
                        } else {
                            egui::RichText::new(&similar_token.word)
                        };

                        ui.label(word_text);
                        ui.label(
                            egui::RichText::new(format!(
                                "Levenshtein distance: {:.2}\nLongest common substring: {:.0}",
                                similar_token.distance, similar_token.lcs_length
                            ))
                            .weak(),
                        );
                    });
                }
            });
    }
}
