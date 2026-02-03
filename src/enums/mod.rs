//! Enumerations used throughout the application.

pub mod app_action;
pub mod app_error;
pub mod file_type;
pub mod formation_type;
pub mod popups;
pub mod sort_mode;
pub mod ui_action;
pub mod word_ref;

pub use app_action::AppAction;
pub use app_error::AppError;
pub use file_type::FileType;
pub use formation_type::FormationType;
pub use popups::{DictionaryPopupType, PinnedPopup, PopupRequest};
pub use sort_mode::{SortDirection, SortField, SortMode};
pub use ui_action::UiAction;

pub use word_ref::WordRef;

/// Result type alias for operations that may produce an [`AppError`].
pub type AppResult<T> = Result<T, AppError>;
