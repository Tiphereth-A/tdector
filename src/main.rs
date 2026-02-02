//! Text Decryption Helper - An interactive GUI tool for linguistic analysis.
//!
//! This application provides an interactive environment for text decryption, translation,
//! and linguistic annotation. It's particularly useful for working with historical texts,
//! cipher decryption, or learning new writing systems.
//!
//! # Features
//!
//! - Import and tokenize text files (character-based or word-based)
//! - Create and manage vocabulary glossaries with per-token definitions
//! - Translate complete segments with contextual annotations
//! - Custom font support for special scripts and writing systems
//! - TF-IDF similarity search for finding related segments
//! - Export annotated documents to Typst format for professional typesetting

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod io;
mod models;
mod ui;

use app::DecryptionApp;
use eframe::egui;
use ui::constants;

/// Application entry point.
///
/// Initializes the GUI framework and launches the main application window.
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([constants::WINDOW_WIDTH, constants::WINDOW_HEIGHT]),
        ..Default::default()
    };

    eframe::run_native(
        "Text Decryption Helper",
        options,
        Box::new(|cc| Ok(DecryptionApp::new(cc))),
    )
}
