/// Project loading, conversion, and saving functionality
///
/// Handles all serialization concerns including format versioning and migration.
/// Project files use a v2 format with space-optimized vocabulary storage:
/// - Base vocabulary words are stored once and referenced by index
/// - Derived words store the base word index and a chain of formation rule indices
/// - Word references use positive integers for base words, negative for derived words
pub mod exporter;
pub mod importer;
pub mod models;
pub mod update_v1;

pub use exporter::convert_to_saved_project;
pub use importer::load_project_from_json;
pub use models::{Project, Segment, Token};
