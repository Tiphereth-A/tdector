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
thread_local! {
    static IS_APP_DIRTY: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// Sets the dirty flag in WASM mode. Called from the app's update loop.
#[cfg(target_arch = "wasm32")]
pub fn set_app_dirty(dirty: bool) {
    IS_APP_DIRTY.with(|flag| flag.set(dirty));
}

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

        setup_beforeunload_handler();

        if let Some(loading_element) = document.get_element_by_id("loading_text") {
            loading_element.remove();
        }
    });
}

/// Sets up a `beforeunload` event listener to warn users about unsaved changes.
///
/// This prevents accidental data loss by prompting the user if they try to
/// close or navigate away from the page while the project has unsaved changes.
/// The check is performed via a periodic callback that accesses the app state.
#[cfg(target_arch = "wasm32")]
fn setup_beforeunload_handler() {
    use eframe::wasm_bindgen::prelude::*;
    use wasm_bindgen::closure::Closure;

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    // Create a closure that will be called when beforeunload fires
    let closure: Closure<dyn Fn(web_sys::Event)> = Closure::new(move |event: web_sys::Event| {
        // Check if the app has unsaved changes
        if is_app_dirty() {
            // Prevent default behavior and show the browser's "unsaved changes" dialog
            event.prevent_default();

            // For modern browsers, try to set returnValue using js_sys reflection
            use js_sys::Reflect;
            use wasm_bindgen::JsValue;

            let _ = Reflect::set(
                &event,
                &JsValue::from_str("returnValue"),
                &JsValue::from_str("You have unsaved changes. Are you sure you want to leave?"),
            );
        }
    });

    if let Err(e) =
        window.add_event_listener_with_callback("beforeunload", closure.as_ref().unchecked_ref())
    {
        log::warn!("Failed to set up beforeunload handler: {:?}", e);
    }

    // Leak the closure to keep it alive for the lifetime of the page
    closure.forget();
}

/// Checks if the current app has unsaved changes.
///
/// This function checks the thread-local is_dirty flag which is updated
/// by the app's update loop.
/// Returns `true` if the app should be considered dirty.
#[cfg(target_arch = "wasm32")]
fn is_app_dirty() -> bool {
    IS_APP_DIRTY.with(|flag| flag.get())
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
