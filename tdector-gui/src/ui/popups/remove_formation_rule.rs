use eframe::egui;

use crate::ui::popup_utils::create_popup_title;
use crate::ui::states::state::DecryptionApp;

impl DecryptionApp {
    pub(super) fn render_remove_formation_rule_popup(&mut self, ctx: &egui::Context) {
        if let Some(dialog) = self.remove_formation_rule_popup.take() {
            let mut open = true;
            let mut should_close = false;
            let title = create_popup_title(
                "Remove Formation Rule: ",
                &dialog.formatted_word,
                self.custom_font_name.is_some(),
            );

            egui::Window::new(title)
                .id(egui::Id::new("remove_formation_rule_popup"))
                .open(&mut open)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Word:");
                        ui.strong(&dialog.formatted_word);
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Base word:");
                        ui.strong(&dialog.base_word);
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Rule to remove:");
                        ui.strong(&dialog.rule_description);
                    });
                    ui.separator();
                    ui.label("This removes the latest applied formation rule for this word.");
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Remove").clicked() {
                            should_close = self.apply_command(
                                tdector_app::Command::RemoveFormationRule {
                                    segment: dialog.sentence_idx,
                                    token: dialog.word_idx,
                                },
                                ctx,
                            );
                        }
                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                });

            if open && !should_close {
                self.remove_formation_rule_popup = Some(dialog);
            }
        }
    }
}
