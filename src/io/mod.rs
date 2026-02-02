//! File I/O: dialogs, project persistence, and export.

mod dialogs;
mod json_formatter;
mod project;
mod typst;

#[cfg(target_arch = "wasm32")]
pub mod wasm_file;

#[cfg(not(target_arch = "wasm32"))]
pub use dialogs::{
    pick_font_file, pick_project_file, pick_save_file, pick_text_file, pick_typst_file,
};

#[cfg(target_arch = "wasm32")]
pub use project::{convert_from_saved_project, convert_to_saved_project, segment_content};

#[cfg(not(target_arch = "wasm32"))]
pub use project::{load_project_file, read_text_content, save_project_file, segment_content};

#[cfg(not(target_arch = "wasm32"))]
pub use typst::save_typst_file;

#[cfg(target_arch = "wasm32")]
pub use typst::generate_typst_content;
