//! Application menu bar with File menu and settings.
//!
//! Provides the top menu bar containing:
//! - File operations (Import, Open, Save, Export, Quit)
//! - Theme selection (Light/Dark mode)
//!
//! Uses platform-appropriate keyboard shortcuts (Cmd on macOS, Ctrl elsewhere).

use eframe::egui;

/// Renders the application menu bar.
///
/// Displays a menu bar with File operations and theme selection.
/// Uses callback functions to decouple menu actions from application state.
///
/// # Arguments
///
/// * `ctx` - The egui context
/// * `project_loaded` - Whether a project is currently loaded (enables/disables certain menu items)
/// * `on_*` - Callback functions invoked when menu items are clicked
///
/// # Keyboard Shortcuts
///
/// - Cmd/Ctrl+I: Import text file
/// - Cmd/Ctrl+O: Open project
/// - Cmd/Ctrl+S: Save project
/// - Cmd/Ctrl+E: Export to Typst
/// - Cmd/Ctrl+Q: Quit application
pub fn render_menu_bar(
    ctx: &egui::Context,
    project_loaded: bool,
    on_import: impl FnOnce(),
    on_open: impl FnOnce(),
    on_save: impl FnOnce(),
    on_export: impl FnOnce(),
    on_quit: impl FnOnce(),
    on_load_font: impl FnOnce(),
    on_add_word_formation_rule: impl FnOnce(),
) {
    let cmd = if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    };

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui
                    .add(egui::Button::new("Import Text...").shortcut_text(format!("{cmd}+I")))
                    .clicked()
                {
                    on_import();
                    ui.close();
                }
                if ui
                    .add(egui::Button::new("Open Project...").shortcut_text(format!("{cmd}+O")))
                    .clicked()
                {
                    on_open();
                    ui.close();
                }
                if ui
                    .add_enabled(project_loaded, egui::Button::new("Load Sentence Font..."))
                    .clicked()
                {
                    on_load_font();
                    ui.close();
                }
                if ui
                    .add(egui::Button::new("Save Project").shortcut_text(format!("{cmd}+S")))
                    .clicked()
                {
                    on_save();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("Export...").shortcut_text(format!("{cmd}+E")))
                    .clicked()
                {
                    on_export();
                    ui.close();
                }
                if ui
                    .add(egui::Button::new("Quit").shortcut_text(format!("{cmd}+Q")))
                    .clicked()
                {
                    on_quit();
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(
                        project_loaded,
                        egui::Button::new("Add New Word Formation Rule"),
                    )
                    .clicked()
                {
                    on_add_word_formation_rule();
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
        });
    });
}
