// Tdector: A GUI tool for assisted text decryption and translation
//
// This application supports both native desktop (Windows/macOS/Linux) and web (WASM) platforms.
// Platform-specific code is conditional: WASM uses web_sys/eframe::WebRunner, while native uses eframe::run_native.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod consts;
mod enums;
mod io;
mod libs;
mod ui;

use ui::DecryptionApp;

#[cfg(target_arch = "wasm32")]
thread_local! {
    // Flag to track if the app has unsaved changes for WASM beforeunload handler
    static IS_APP_DIRTY: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

#[cfg(target_arch = "wasm32")]
// Mark the app as dirty (modified) to trigger unsaved changes warning on page unload
pub fn set_app_dirty(dirty: bool) {
    IS_APP_DIRTY.with(|flag| flag.set(dirty));
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Web entry point: Initialize the application in a browser canvas
    use eframe::wasm_bindgen::JsCast as _;

    // Set up console logging for debugging
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    // Spawn async task to set up the eframe web runner
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window exists")
            .document()
            .expect("no document exists");

        // Find the HTML canvas element where the app will be rendered
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("failed to find the canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("canvas element has wrong type");

        // Create and start the eframe web runner
        let runner = eframe::WebRunner::new();
        runner
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(DecryptionApp::new(cc))),
            )
            .await
            .expect("failed to start eframe");

        // Set up handler to warn about unsaved changes
        setup_beforeunload_handler();

        // Remove loading indicator after app initializes
        if let Some(loading_element) = document.get_element_by_id("loading_text") {
            loading_element.remove();
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn setup_beforeunload_handler() {
    // Attach a beforeunload event listener to warn users about unsaved changes when leaving the page
    use eframe::wasm_bindgen::prelude::*;
    use wasm_bindgen::closure::Closure;

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    let closure: Closure<dyn Fn(web_sys::Event)> = Closure::new(move |event: web_sys::Event| {
        // If app has unsaved changes, prompt the user before leaving
        if is_app_dirty() {
            event.prevent_default();

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

    // Prevent the closure from being garbage collected
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn is_app_dirty() -> bool {
    // Check if the app has unsaved changes
    IS_APP_DIRTY.with(|flag| flag.get())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    // Native desktop entry point: Initialize and run the native application
    use crate::consts::ui::{WINDOW_HEIGHT, WINDOW_WIDTH};
    use eframe::egui;

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
