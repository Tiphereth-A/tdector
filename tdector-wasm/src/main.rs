fn main() {
    #[cfg(target_arch = "wasm32")]
    start_web_app();
}

#[cfg(target_arch = "wasm32")]
fn start_web_app() {
    use eframe::wasm_bindgen::JsCast as _;
    use tdector_gui::ui::DecryptionApp;
    use wasm_bindgen_futures::spawn_local;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    spawn_local(async move {
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

#[cfg(target_arch = "wasm32")]
fn setup_beforeunload_handler() {
    use eframe::wasm_bindgen::prelude::*;
    use wasm_bindgen::closure::Closure;

    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };

    let closure: Closure<dyn Fn(web_sys::Event)> = Closure::new(move |event: web_sys::Event| {
        if tdector_core::is_app_dirty() {
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

    if let Err(error) =
        window.add_event_listener_with_callback("beforeunload", closure.as_ref().unchecked_ref())
    {
        log::warn!("Failed to set up beforeunload handler: {:?}", error);
    }

    closure.forget();
}
