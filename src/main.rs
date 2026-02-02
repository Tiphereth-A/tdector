//! Text Decryption Helper - An interactive GUI tool for linguistic analysis.
//!
//! This application provides an interactive environment for text decryption, translation,
//! and linguistic annotation. It's particularly useful for working with historical texts,
//! cipher decryption, or learning new writing systems.
//!
//! # Features
//!
//! - Import and tokenize text files (character-based or word-based)
//! - Create and manage vocabulary glossaries with per-token definitions
//! - Translate complete segments with contextual annotations
//! - Custom font support for special scripts and writing systems
//! - TF-IDF similarity search for finding related segments
//! - Export annotated documents to Typst format for professional typesetting

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod io;
mod models;
mod ui;

use app::DecryptionApp;

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window exists")
            .document()
            .expect("no document exists");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("failed to find the canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("canvas element has wrong type");
        let runner = eframe::WebRunner::new();
        runner
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(DecryptionApp::new(cc))),
            )
            .await
            .expect("failed to start eframe");

        // Remove the loading spinner once the app has started
        if let Some(loading_element) = document.get_element_by_id("loading_text") {
            loading_element.remove();
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use eframe::egui;
    use ui::constants;

    env_logger::init();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([constants::WINDOW_WIDTH, constants::WINDOW_HEIGHT]);
    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Text Decryption Helper",
        options,
        Box::new(|cc| Ok(DecryptionApp::new(cc))),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn load_app_icon() -> Option<eframe::egui::IconData> {
    const ICON_BYTES: &[u8] = include_bytes!("../assets/favicon.ico");
    let image = image::load_from_memory(ICON_BYTES).ok()?.into_rgba8();
    let (width, height) = image.dimensions();

    Some(eframe::egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
