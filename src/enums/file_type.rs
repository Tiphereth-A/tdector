/// File types supported by the application for import/export and loading
#[derive(Debug, Clone, Copy)]
pub enum FileType {
    /// Plain text files containing segments to be translated
    Text,

    /// JSON project files (see SavedProjectV2 format)
    Json,

    /// Font files for rendering special scripts and writing systems
    Font,

    /// Typst markup files for academic publishing and typesetting
    Typst,
}

impl FileType {
    /// Get the human-readable name for file dialogs
    pub fn filter_name(&self) -> &'static str {
        match self {
            FileType::Text => "Text",
            FileType::Json => "JSON",
            FileType::Font => "Font",
            FileType::Typst => "Typst",
        }
    }

    /// Get the file extensions associated with this type
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            FileType::Text => &["txt"],
            FileType::Json => &["json"],
            FileType::Font => &["ttf", "otf", "ttc"],
            FileType::Typst => &["typ"],
        }
    }
}
