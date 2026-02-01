//! Application core: state management and eframe integration.
//!
//! ## Submodules
//!
//! - [`actions`] — Action enums and sort modes
//! - [`commands`] — Command pattern implementation
//! - [`dialogs`] — Modal dialog rendering
//! - [`dictionary_popups`] — Dictionary definition and reference popups
//! - [`file_ops`] — File I/O and project management
//! - [`filtering`] — Segment filtering and sorting
//! - [`lookup_cache`] — Lookup cache management
//! - [`panels`] — Filter and content panel rendering
//! - [`pinned_popups`] — Pinned popup management
//! - [`popups`] — Popup rendering coordination
//! - [`popup_utils`] — Popup utility functions
//! - [`similar`] — TF-IDF similarity search
//! - [`similar_popups`] — Similar segments popups
//! - [`state`] — Application state struct
//! - [`tfidf_cache`] — TF-IDF cache with incremental updates
//! - [`update`] — Main render loop
//! - [`word_formation_popups`] — Word formation and menu popups

mod actions;
mod cache_lookup;
mod cache_tfidf;
mod commands;
mod dialogs;
mod file_ops;
mod filtering;
mod panels;
mod popup_dictionary;
mod popup_pinned;
mod popup_similar;
mod popup_utils;
mod popup_word_formation;
mod popups;
mod similar;
mod state;
mod update;

pub use state::DecryptionApp;
