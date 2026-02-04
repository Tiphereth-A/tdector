/// Enumeration types for the application
///
/// Includes:
/// - AppAction: High-level menu actions
/// - AppError: Error types
/// - FileType: Supported file types for I/O
/// - FormationType: Word formation rule categories
/// - PopupRequest: Popup window requests
/// - SortMode: Segment sorting options
/// - UiAction: UI element actions
/// - CommentTarget: Comment attachment targets
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

pub use word_ref::CommentTarget;

pub type AppResult<T> = Result<T, AppError>;
