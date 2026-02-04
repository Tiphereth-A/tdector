// Library module declarations for the Tdector text decryption and translation tool
//
// Organization:
// - consts: Constants like UI dimensions, colors, and domain rules
// - enums: Enumeration types for app states and actions
// - io: File I/O, JSON formatting, and export functionality
// - libs: Core business logic (project management, text analysis, caching, filtering, sorting)
// - ui: GUI components and state management using egui

pub mod consts;
pub mod enums;
pub mod io;
pub mod libs;
pub mod ui;

#[cfg(target_arch = "wasm32")]
thread_local! {
    // Flag to track if the app has unsaved changes for WASM beforeunload handler
    static IS_APP_DIRTY: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

#[cfg(target_arch = "wasm32")]
/// Mark the app as dirty (modified) to trigger unsaved changes warning on page unload.
pub fn set_app_dirty(dirty: bool) {
    IS_APP_DIRTY.with(|flag| flag.set(dirty));
}

#[cfg(target_arch = "wasm32")]
pub fn is_app_dirty() -> bool {
    IS_APP_DIRTY.with(|flag| flag.get())
}
