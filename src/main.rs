//! # Text Decryption Helper
//!
//! An interactive GUI tool for assisted text decryption and translation.
//!
//! ## Features
//!
//! - Import text files and segment them into tokens
//! - Add glosses (meanings) to individual tokens
//! - Translate complete text segments
//! - Export projects to Typst format for typesetting

mod app;
mod io;
mod models;
mod ui;

use app::DecryptionApp;
use eframe::egui;
use ui::constants;

/// Launches the application with default window settings (1024×768).
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([constants::WINDOW_WIDTH, constants::WINDOW_HEIGHT]),
        ..Default::default()
    };

    eframe::run_native(
        "Text Decryption Helper",
        options,
        Box::new(|cc| DecryptionApp::new(cc)),
    )
}
