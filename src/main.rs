//! Text Decryption Helper - An interactive GUI application for linguistic analysis and text annotation.
//!
//! A comprehensive tool for text decryption, translation, and linguistic annotation.
//! Particularly useful for historical texts, cipher analysis, and learning new writing systems.
//!
//! ## Core Features
//!
//! - **Import & Tokenization**: Support for character-based and word-based text segmentation
//! - **Vocabulary Management**: Create and maintain comprehensive glossaries with per-token definitions
//! - **Segment Translation**: Translate segments with contextual annotations and metadata
//! - **Custom Fonts**: Support for special scripts and non-Latin writing systems
//! - **Similarity Search**: TF-IDF-based engine to find semantically related segments
//! - **Export**: Generate professionally typeset documents in Typst format

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! ## Module Organization
//!
//! The codebase is organized into distinct architectural layers:

/// Constants: Application-wide domain rules and UI theming
mod consts;

/// Domain layer: Core business logic and algorithms
mod libs;

/// Enumerations: Core enum types used throughout the application
mod enums;

/// I/O layer: File operations, persistence, and format handling
mod io;

/// Presentation layer: UI components, rendering, and user interaction
mod ui;

use ui::DecryptionApp;

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

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

        if let Some(loading_element) = document.get_element_by_id("loading_text") {
            loading_element.remove();
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use eframe::egui;

    use crate::consts::ui::{WINDOW_HEIGHT, WINDOW_WIDTH};

    env_logger::init();

    let mut viewport =
        egui::ViewportBuilder::default().with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT]);
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
