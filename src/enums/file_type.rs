/// Supported file types for I/O operations
#[derive(Debug, Clone, Copy)]
pub enum FileType {
    /// Text files (.txt)
    Text,
    /// JSON project files (.json)
    Json,
    /// Font files (.ttf, .otf, .ttc)
    Font,
    /// Typst markup files (.typ)
    Typst,
}

impl FileType {
    /// Get the user-friendly filter name for this file type
    pub fn filter_name(&self) -> &'static str {
        match self {
            FileType::Text => "Text",
            FileType::Json => "JSON",
            FileType::Font => "Font",
            FileType::Typst => "Typst",
        }
    }

    /// Get the file extensions for this file type
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            FileType::Text => &["txt"],
            FileType::Json => &["json"],
            FileType::Font => &["ttf", "otf", "ttc"],
            FileType::Typst => &["typ"],
        }
    }
}
