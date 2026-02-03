//! I/O layer: File operations, persistence, and format handling.
//!
//! Handles all file-related operations and data serialization:
//!
//! - **file_io**: Unified async file I/O using rfd for both desktop and WASM
//! - **project**: Project loading and saving (JSON formats with migration)
//! - **json_formatter**: JSON serialization with custom formatting
//! - **typst**: Export functionality for Typst typesetting format
//! - **file_ops**: Integration point for async I/O operations

pub mod file_io;
mod file_ops;
pub mod json_formatter;
mod project;
mod typst;

pub use file_io::FileIO;
pub use file_ops::{initialize_fonts, register_custom_font};
pub use project::{convert_from_saved_project, convert_to_saved_project, segment_content};
pub use typst::generate_typst_content;
