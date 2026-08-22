#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    // Native desktop entry point: Initialize and run the native application
    use eframe::egui;
    use tdector_gui::consts::ui::{WINDOW_HEIGHT, WINDOW_WIDTH};
    use tdector_gui::ui::DecryptionApp;

    // Initialize the logging system
    env_logger::init();

    // Configure the main window with size and icon
    let mut viewport =
        egui::ViewportBuilder::default().with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT]);
    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    // Run the native eframe application
    eframe::run_native(
        "Text Decryption Helper",
        options,
        Box::new(|cc| Ok(DecryptionApp::new(cc))),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn load_app_icon() -> Option<eframe::egui::IconData> {
    // Load the application icon from embedded bytes and convert to egui IconData
    const ICON_BYTES: &[u8] = include_bytes!("../assets/favicon.ico");
    let image = image::load_from_memory(ICON_BYTES).ok()?.into_rgba8();
    let (width, height) = image.dimensions();

    Some(eframe::egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
