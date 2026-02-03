//! Domain configuration constants.
//!
//! Defines domain-level thresholds and constraints (business rules),
//! distinct from UI layout constants.

/// Project serialization version number.
pub const PROJECT_VERSION: u32 = 1;

/// Default number of similar segments to retrieve in similarity searches.
pub const DEFAULT_SIMILARITY_RESULTS: usize = 5;

/// Default number of related words to suggest.
pub const DEFAULT_RELATED_WORDS_COUNT: usize = 5;

/// Maximum depth for script evaluation to prevent stack overflow.
pub const MAX_SCRIPT_DEPTH: usize = 5000;

/// Maximum operations for script evaluation to prevent infinite loops.
pub const MAX_SCRIPT_OPERATIONS: u64 = 100000;

/// Timeout duration in milliseconds for file operations in WASM.
#[cfg(target_arch = "wasm32")]
pub const FILE_OPERATION_TIMEOUT_MS: u32 = 1000;
