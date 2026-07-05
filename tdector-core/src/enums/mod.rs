pub mod app_error;
pub mod formation_type;
pub mod sort_mode;
pub mod word_ref;

pub use app_error::AppError;
pub use formation_type::FormationType;
pub use sort_mode::{SortDirection, SortField, SortMode};
pub use word_ref::CommentTarget;

pub type AppResult<T> = Result<T, AppError>;
