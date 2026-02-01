//! Application core: state management and eframe integration.
//!
//! ## Submodules
//!
//! - [`actions`] — Action enums and sort modes
//! - [`commands`] — Command pattern implementation
//! - [`dialogs`] — Modal dialog rendering
//! - [`file_ops`] — File I/O and project management
//! - [`filtering`] — Segment filtering and sorting
//! - [`lookup_cache`] — Lookup cache management
//! - [`panels`] — Filter and content panel rendering
//! - [`popups`] — Dictionary and similar-segments popups
//! - [`popup_utils`] — Popup utility functions
//! - [`similar`] — TF-IDF similarity search
//! - [`state`] — Application state struct
//! - [`tfidf_cache`] — TF-IDF cache with incremental updates
//! - [`update`] — Main render loop

mod actions;
mod commands;
mod dialogs;
mod file_ops;
mod filtering;
mod lookup_cache;
mod panels;
mod popup_utils;
mod popups;
mod similar;
mod state;
mod tfidf_cache;
mod update;

pub use state::DecryptionApp;
