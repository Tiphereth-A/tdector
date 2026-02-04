use eframe::egui;

use crate::ui::states::DecryptionApp;

impl DecryptionApp {
    pub(crate) fn render_error_dialog(&mut self, ctx: &egui::Context) {
        if let Some(msg) = &self.error_message {
            let mut open = true;
            let mut should_close = false;
            egui::Window::new("Error")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(msg);
                    ui.add_space(10.0);
                    if ui.button("OK").clicked() {
                        should_close = true;
                    }
                });
            if !open || should_close {
                self.error_message = None;
            }
        }
    }

    pub(crate) fn render_confirmation_dialog(&mut self, ctx: &egui::Context) {
        let mut confirmed_action = None;
        let mut close_dialog = false;

        let confirmation_data = self.confirmation.clone();

        if let Some((msg, action)) = confirmation_data {
            let mut open = true;
            egui::Window::new("Confirmation")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(&msg);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            confirmed_action = Some(action);
                            close_dialog = true;
                        }
                        if ui.button("No").clicked() {
                            close_dialog = true;
                        }
                    });
                });
            if !open {
                close_dialog = true;
            }
        }

        if close_dialog {
            self.confirmation = None;
        }

        if let Some(action) = confirmed_action {
            self.confirmation = None;
            self.execute_action(action, ctx);
        }
    }

    pub(crate) fn render_import_dialog(&mut self, ctx: &egui::Context) {
        if self.pending_import.is_some() {
            let mut choice = None;
            let mut open = true;
            egui::Window::new("Import Options")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading("Tokenization Strategy");
                    ui.label("How should the text be split?");
                    ui.label("Select the tokenization strategy based on the language script.");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui
                            .button("Word-based (Spaces)")
                            .on_hover_text("Split text by whitespace (e.g. English)")
                            .clicked()
                        {
                            choice = Some(true);
                        }
                        if ui
                            .button("Character-based")
                            .on_hover_text(
                                "Treat each character as a token (e.g. Chinese, Japanese)",
                            )
                            .clicked()
                        {
                            choice = Some(false);
                        }
                    });
                });

            if !open {
                self.pending_import = None;
            } else if let Some(use_whitespace) = choice
                && let Some((content, name)) = self.pending_import.take()
            {
                let segments = crate::libs::project::segment_content(&content, use_whitespace);
                self.project.segments = segments;
                self.project.project_name = name;
                self.project.font_path = None;
                self.current_path = None;
                self.project_filename = None;
                self.filter_dirty = true;
                self.lookups_dirty = true;
                self.tfidf_dirty = true;
                self.filter_text.clear();
                self.clear_popups();
                self.update_dirty_status(true, ctx);
            }
        }
    }
}
