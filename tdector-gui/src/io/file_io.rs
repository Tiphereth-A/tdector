use crate::enums::{AppError, AppResult};
use rfd::AsyncFileDialog;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

/// File I/O result tuple: (file bytes, filename, full path)
pub type FileResult = (Vec<u8>, String, Option<String>);

/// Cross-platform file I/O utilities supporting both native and WASM targets
pub struct FileIO;

impl FileIO {
    /// Open a file picker dialog and read the selected file asynchronously.
    /// Returns file bytes, filename, and full path (if available on this platform).
    pub async fn pick_file(filter_name: &str, extensions: &[&str]) -> AppResult<FileResult> {
        let mut dialog = AsyncFileDialog::new();
        if !extensions.is_empty() {
            dialog = dialog.add_filter(filter_name, extensions);
        }

        match dialog.pick_file().await {
            Some(handle) => {
                let bytes = handle.read().await;
                let filename = handle.file_name();

                // Native: use full file system path; WASM: use filename only
                #[cfg(not(target_arch = "wasm32"))]
                let full_path = handle.path().to_string_lossy().to_string();

                #[cfg(target_arch = "wasm32")]
                let full_path = filename.clone();

                Ok((bytes, filename, Some(full_path)))
            }
            None => Err(AppError::OperationCancelled),
        }
    }

    /// Open a save file dialog and write content to the selected file asynchronously
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
                .map_err(|e| AppError::IoError(format!("Failed to save file: {e}"))),
            None => Err(AppError::OperationCancelled),
        }
    }

    /// Synchronously write content to a specific file path (native only)
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn save_file_to_path(content: &[u8], path: &std::path::Path) -> AppResult<()> {
        std::fs::write(path, content)
            .map_err(|e| AppError::IoError(format!("Failed to write file: {e}")))
    }

    /// Platform-specific async spawning: WASM uses spawn_local, native uses pollster::block_on
    #[cfg(target_arch = "wasm32")]
    pub fn spawn<F>(future: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        spawn_local(future);
    }

    /// Platform-specific async spawning: WASM uses `spawn_local`, native uses `pollster::block_on`
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn<F>(future: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        pollster::block_on(future);
    }
}
