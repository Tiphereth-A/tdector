//! WASM-specific file handling using browser APIs.

#[cfg(target_arch = "wasm32")]
pub mod wasm_files {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{HtmlInputElement, window};

    /// Triggers a file picker in the browser for text files.
    /// Returns the file content and name via a callback.
    pub fn pick_and_read_text_file<F>(callback: F)
    where
        F: Fn(Result<(String, String), String>) + 'static + Clone,
    {
        pick_and_read_file("text/plain,.txt", callback);
    }

    /// Triggers a file picker in the browser for JSON files.
    /// Returns the file content and name via a callback.
    pub fn pick_and_read_json_file<F>(callback: F)
    where
        F: Fn(Result<(String, String), String>) + 'static + Clone,
    {
        pick_and_read_file("application/json,.json", callback);
    }

    /// Triggers a file picker in the browser for font files.
    /// Returns the binary font data and name via a callback.
    pub fn pick_and_read_font_file<F>(callback: F)
    where
        F: Fn(Result<(Vec<u8>, String), String>) + 'static + Clone,
    {
        pick_and_read_binary_file(".ttf,.otf,.ttc", callback);
    }

    /// Generic file picker and reader for browser.
    fn pick_and_read_file<F>(accept: &str, callback: F)
    where
        F: Fn(Result<(String, String), String>) + 'static + Clone,
    {
        let document = match window().and_then(|w| w.document()) {
            Some(doc) => doc,
            None => {
                callback(Err("No window/document available".to_string()));
                return;
            }
        };

        // Create a hidden file input element
        let input = match document.create_element("input") {
            Ok(el) => el,
            Err(_) => {
                callback(Err("Failed to create input element".to_string()));
                return;
            }
        };

        let input: HtmlInputElement = match input.dyn_into() {
            Ok(input) => input,
            Err(_) => {
                callback(Err("Failed to cast to HtmlInputElement".to_string()));
                return;
            }
        };

        input.set_type("file");
        input.set_accept(accept);
        input.style().set_property("display", "none").ok();

        // Set up the change event handler
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let input: HtmlInputElement = event.target().unwrap().dyn_into().unwrap();

            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    let file_name = file.name();
                    let file_reader = web_sys::FileReader::new().unwrap();

                    let fr_clone = file_reader.clone();
                    let callback_clone = callback.clone();

                    let onload = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        if let Ok(result) = fr_clone.result() {
                            if let Some(text) = result.as_string() {
                                callback_clone(Ok((text, file_name.clone())));
                            } else {
                                callback_clone(Err("Failed to read file as text".to_string()));
                            }
                        } else {
                            callback_clone(Err("Failed to get file reader result".to_string()));
                        }
                    }) as Box<dyn FnMut(_)>);

                    file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                    onload.forget();

                    file_reader.read_as_text(&file).ok();
                }
            }
        }) as Box<dyn FnMut(_)>);

        input.set_onchange(Some(closure.as_ref().unchecked_ref()));
        closure.forget();

        // Add to document and trigger click
        document.body().unwrap().append_child(&input).ok();
        input.click();

        // Remove the input after a short delay
        let input_clone = input.clone();
        let timeout_closure = Closure::wrap(Box::new(move || {
            input_clone.remove();
        }) as Box<dyn FnMut()>);

        window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout_closure.as_ref().unchecked_ref(),
                1000,
            )
            .ok();
        timeout_closure.forget();
    }

    /// Generic binary file picker and reader for browser.
    fn pick_and_read_binary_file<F>(accept: &str, callback: F)
    where
        F: Fn(Result<(Vec<u8>, String), String>) + 'static + Clone,
    {
        let document = match window().and_then(|w| w.document()) {
            Some(doc) => doc,
            None => {
                callback(Err("No window/document available".to_string()));
                return;
            }
        };

        // Create a hidden file input element
        let input = match document.create_element("input") {
            Ok(el) => el,
            Err(_) => {
                callback(Err("Failed to create input element".to_string()));
                return;
            }
        };

        let input: HtmlInputElement = match input.dyn_into() {
            Ok(input) => input,
            Err(_) => {
                callback(Err("Failed to cast to HtmlInputElement".to_string()));
                return;
            }
        };

        input.set_type("file");
        input.set_accept(accept);
        input.style().set_property("display", "none").ok();

        // Set up the change event handler
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let input: HtmlInputElement = event.target().unwrap().dyn_into().unwrap();

            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    let file_name = file.name();
                    let file_reader = web_sys::FileReader::new().unwrap();

                    let fr_clone = file_reader.clone();
                    let callback_clone = callback.clone();

                    let onload = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        if let Ok(result) = fr_clone.result() {
                            if let Some(array_buffer) = result.dyn_ref::<js_sys::ArrayBuffer>() {
                                let uint8_array = js_sys::Uint8Array::new(array_buffer);
                                let mut bytes = vec![0u8; uint8_array.length() as usize];
                                uint8_array.copy_to(&mut bytes);
                                callback_clone(Ok((bytes, file_name.clone())));
                            } else {
                                callback_clone(Err("Failed to read file as binary".to_string()));
                            }
                        } else {
                            callback_clone(Err("Failed to get file reader result".to_string()));
                        }
                    }) as Box<dyn FnMut(_)>);

                    file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                    onload.forget();

                    file_reader.read_as_array_buffer(&file).ok();
                }
            }
        }) as Box<dyn FnMut(_)>);

        input.set_onchange(Some(closure.as_ref().unchecked_ref()));
        closure.forget();

        // Add to document and trigger click
        document.body().unwrap().append_child(&input).ok();
        input.click();

        // Remove the input after a short delay
        let input_clone = input.clone();
        let timeout_closure = Closure::wrap(Box::new(move || {
            input_clone.remove();
        }) as Box<dyn FnMut()>);

        window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout_closure.as_ref().unchecked_ref(),
                1000,
            )
            .ok();
        timeout_closure.forget();
    }

    /// Downloads a file in the browser.
    pub fn download_file(filename: &str, content: &str, mime_type: &str) {
        let window = match window() {
            Some(w) => w,
            None => return,
        };

        let document = match window.document() {
            Some(d) => d,
            None => return,
        };

        // Create a blob with the content
        let array = js_sys::Array::new();
        array.push(&JsValue::from_str(content));

        let blob_options = web_sys::BlobPropertyBag::new();
        blob_options.set_type(mime_type);

        let blob = match web_sys::Blob::new_with_str_sequence_and_options(&array, &blob_options) {
            Ok(b) => b,
            Err(_) => return,
        };

        // Create download link
        let url = match web_sys::Url::create_object_url_with_blob(&blob) {
            Ok(url) => url,
            Err(_) => return,
        };

        let a = match document.create_element("a") {
            Ok(el) => el,
            Err(_) => return,
        };

        let a: web_sys::HtmlAnchorElement = match a.dyn_into() {
            Ok(a) => a,
            Err(_) => return,
        };

        a.set_href(&url);
        a.set_download(filename);
        a.style().set_property("display", "none").ok();

        document.body().unwrap().append_child(&a).ok();
        a.click();
        a.remove();

        web_sys::Url::revoke_object_url(&url).ok();
    }
}
