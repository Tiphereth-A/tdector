pub const PROJECT_VERSION: u64 = 2;

#[cfg(not(target_arch = "wasm32"))]
pub const DEFAULT_SIMILARITY_RESULTS: usize = 5;

pub const DEFAULT_RELATED_WORDS_COUNT: usize = 5;

pub const MAX_SCRIPT_DEPTH: usize = 500000;

pub const MAX_SCRIPT_OPERATIONS: u64 = 10000000;
