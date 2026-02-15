/// File I/O, formatting, and export functionality
///
/// Handles:
/// - `file_io`: Cross-platform file operations with async support
/// - `file_ops`: Font loading and registration
/// - `json_formatter`: Custom JSON serialization formatting
/// - typst: Export to Typst markup for academic publications
pub mod file_io;
mod file_ops;
pub mod json_formatter;
mod typst;

pub use file_io::FileIO;
pub use file_ops::{initialize_fonts, register_custom_font};
pub use typst::generate_typst_content;
