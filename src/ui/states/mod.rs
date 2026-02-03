//! State management layer: Application state, actions, and event processing.
//!
//! Orchestrates the application's reactive event loop:
//! - **state**: Core `DecryptionApp` state container and lifecycle management
//! - **actions**: User-triggered actions (menu selections, keyboard shortcuts, etc.)
//! - **update**: Main update loop that processes events and updates state

pub mod state;
pub mod update;

pub use state::DecryptionApp;
