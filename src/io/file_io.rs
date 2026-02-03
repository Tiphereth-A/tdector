//! Unified async file I/O using rfd for both desktop and WASM.
//!
//! This module provides platform-agnostic async file operations using rfd's `AsyncFileDialog`.
//! It eliminates code duplication by using the same async API for both platforms:
//! - File picking (async dialogs work on both desktop and browser)
//! - File reading (async on both platforms)
//! - File writing (async on both platforms)

use crate::enums::{AppError, AppResult};
use rfd::AsyncFileDialog;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

/// Result of a file read operation: (bytes, filename, optional full path on desktop)
pub type FileResult = (Vec<u8>, String, Option<String>);

/// Unified async file operations struct
pub struct FileIO;

impl FileIO {
    /// Pick and read a file with specified filters
    /// Returns raw bytes - caller can decode as UTF-8 if needed
    /// On desktop, also returns the full path to enable in-place saving
    pub async fn pick_file(filter_name: &str, extensions: &[&str]) -> AppResult<FileResult> {
        let mut dialog = AsyncFileDialog::new();
        if !extensions.is_empty() {
            dialog = dialog.add_filter(filter_name, extensions);
        }

        match dialog.pick_file().await {
            Some(handle) => {
                let bytes = handle.read().await;
                let filename = handle.file_name();
                
                // On desktop, get the full path for in-place saving
                #[cfg(not(target_arch = "wasm32"))]
                let full_path = handle.path().to_string_lossy().to_string();
                
                #[cfg(target_arch = "wasm32")]
                let full_path = filename.clone();
                
                Ok((bytes, filename, Some(full_path)))
            }
            None => Err(AppError::OperationCancelled),
        }
    }

    /// Save a file with specified filters
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

    /// Desktop-only: Save directly to a path without showing a dialog
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn save_file_to_path(content: &[u8], path: &std::path::Path) -> AppResult<()> {
        std::fs::write(path, content)
            .map_err(|e| AppError::IoError(format!("Failed to write file: {e}")))
    }

    /// Platform-specific async spawner for launching async tasks
    #[cfg(target_arch = "wasm32")]
    pub fn spawn<F>(future: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        spawn_local(future);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn<F>(future: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        pollster::block_on(future);
    }
}
