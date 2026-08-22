use egui::Context;
use rfd::AsyncFileDialog;
use tdector_eval::{AppError, AppResult};

pub mod io;
pub mod project;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

/// File types supported by the application for import/export and loading.
#[derive(Debug, Clone, Copy)]
pub enum FileType {
    /// Plain text files containing segments to be translated.
    Text,
    /// JSON project files.
    Json,
    /// Font files for rendering special scripts and writing systems.
    Font,
    /// Typst markup files for academic publishing and typesetting.
    Typst,
}

impl FileType {
    pub fn filter_name(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Json => "JSON",
            Self::Font => "Font",
            Self::Typst => "Typst",
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Text => &["txt"],
            Self::Json => &["json"],
            Self::Font => &["ttf", "otf", "ttc"],
            Self::Typst => &["typ"],
        }
    }
}

/// Result of selecting a file: bytes, filename, and an optional path.
pub type FileResult = (Vec<u8>, String, Option<String>);

pub struct FileIO;

impl FileIO {
    pub async fn pick_file(filter_name: &str, extensions: &[&str]) -> AppResult<FileResult> {
        let mut dialog = AsyncFileDialog::new();
        if !extensions.is_empty() {
            dialog = dialog.add_filter(filter_name, extensions);
        }

        match dialog.pick_file().await {
            Some(handle) => {
                let bytes = handle.read().await;
                let filename = handle.file_name();

                #[cfg(not(target_arch = "wasm32"))]
                let full_path = handle.path().to_string_lossy().to_string();

                #[cfg(target_arch = "wasm32")]
                let full_path = filename.clone();

                Ok((bytes, filename, Some(full_path)))
            }
            None => Err(AppError::OperationCancelled),
        }
    }

    pub async fn save_file(
        content: &[u8],
        filename: &str,
        filter_name: &str,
        extensions: &[&str],
    ) -> AppResult<()> {
        let mut dialog = AsyncFileDialog::new().set_file_name(filename);
        if !extensions.is_empty() {
            dialog = dialog.add_filter(filter_name, extensions);
        }

        match dialog.save_file().await {
            Some(handle) => handle
                .write(content)
                .await
                .map_err(|error| AppError::IoError(format!("Failed to save file: {error}"))),
            None => Err(AppError::OperationCancelled),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn save_file_to_path(content: &[u8], path: &std::path::Path) -> AppResult<()> {
        std::fs::write(path, content)
            .map_err(|error| AppError::IoError(format!("Failed to write file: {error}")))
    }

    pub fn spawn<F>(future: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        spawn_future(future);
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    pollster::block_on(future);
}

pub fn register_custom_font(ctx: &Context, data: Vec<u8>) {
    use std::sync::Arc;

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "custom_font".to_owned(),
        Arc::new(egui::FontData::from_owned(data)),
    );

    let fallbacks = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut custom_list = vec!["custom_font".to_owned()];
    custom_list.extend(fallbacks);
    fonts
        .families
        .insert(egui::FontFamily::Name("SentenceFont".into()), custom_list);
    ctx.set_fonts(fonts);
}

pub fn initialize_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    let fallbacks = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    fonts
        .families
        .insert(egui::FontFamily::Name("SentenceFont".into()), fallbacks);
    ctx.set_fonts(fonts);
}
