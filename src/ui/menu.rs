//! Top menu bar with File operations and settings.

use eframe::egui;

/// Renders the menu bar with File menu and Dictionary Mode toggle.
pub fn render_menu_bar(
    ctx: &egui::Context,
    dictionary_mode: &mut bool,
    project_loaded: bool,
    on_import: impl FnOnce(),
    on_open: impl FnOnce(),
    on_save: impl FnOnce(),
    on_export: impl FnOnce(),
    on_quit: impl FnOnce(),
    on_load_font: impl FnOnce(),
) {
    let cmd = if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    };

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.add(egui::Button::new("Import Text...").shortcut_text(format!("{}+I", cmd))).clicked() {
                    on_import();
                    ui.close();
                }
                if ui.add(egui::Button::new("Open Project...").shortcut_text(format!("{}+O", cmd))).clicked() {
                    on_open();
                    ui.close();
                }
                if ui.add_enabled(project_loaded, egui::Button::new("Load Sentence Font...")).clicked() {
                    on_load_font();
                    ui.close();
                }
                if ui.add(egui::Button::new("Save Project").shortcut_text(format!("{}+S", cmd))).clicked() {
                    on_save();
                    ui.close();
                }
                ui.separator();
                if ui.add(egui::Button::new("Export...").shortcut_text(format!("{}+E", cmd))).clicked() {
                    on_export();
                    ui.close();
                }
                if ui.add(egui::Button::new("Quit").shortcut_text(format!("{}+Q", cmd))).clicked() {
                    on_quit();
                    ui.close();
                }
            });
            ui.menu_button("Theme", |ui| {
                if ui.button("Light").clicked() {
                    ctx.set_visuals(egui::Visuals::light());
                    ui.close();
                }
                if ui.button("Dark").clicked() {
                    ctx.set_visuals(egui::Visuals::dark());
                    ui.close();
                }
            });
            ui.checkbox(dictionary_mode, "Dictionary Mode")
                .on_hover_text("Enable Dictionary Mode to use the loaded dictionary for decryption suggestions.");
        });
    });
}
