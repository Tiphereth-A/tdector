//! Popup window implementations.
//!
//! This module contains all popup window rendering implementations:
//!
//! - **coordinator**: Central popup rendering coordination
//! - **comments**: Comment editing popups
//! - **dictionary**: Dictionary definition and reference popups
//! - **pinned**: Pinned/persistent popup management
//! - **similar**: Similar segments popup
//! - **`word_formation`**: Word formation rule popups
//! - **`menu_word`**: Word context menu rendering
//! - **`menu_sentence`**: Sentence context menu rendering

pub(crate) mod comments;
pub(crate) mod coordinator;
pub(crate) mod dictionary;
pub(crate) mod menu_sentence;
pub(crate) mod menu_word;
pub(crate) mod pinned;
pub(crate) mod similar;
pub(crate) mod word_formation;
