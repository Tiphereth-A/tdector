//! Application core: state management and eframe integration.
//!
//! ## Submodules
//!
//! - [`actions`] — Action enums and sort modes
//! - [`dialogs`] — Modal dialog rendering
//! - [`file_ops`] — File I/O and project management
//! - [`filtering`] — Segment filtering and sorting
//! - [`panels`] — Filter and content panel rendering
//! - [`popups`] — Dictionary and similar-segments popups
//! - [`similar`] — BM25 similarity search
//! - [`state`] — Application state struct
//! - [`update`] — Main render loop

mod actions;
mod dialogs;
mod file_ops;
mod filtering;
mod panels;
mod popups;
mod similar;
mod state;
mod update;

pub use state::DecryptionApp;
