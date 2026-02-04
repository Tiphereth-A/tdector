# Copilot instructions for tdector

## Big picture architecture
- GUI app built with `eframe`/`egui`: native desktop and WASM share the same UI state (`DecryptionApp`) and rendering flow. Entry points are in [../src/main.rs](../src/main.rs).
- Core domain logic lives in [../src/libs](../src/libs/mod.rs): project model/serialization, text analysis, caching, filtering/sorting, similarity, and formation rules.
- File I/O and export live in [../src/io](../src/io/mod.rs) (JSON formatting + Typst export).
- Data flow: UI reads/writes `Project` state, then persists via project export/import helpers and JSON migration in [../src/libs/project](../src/libs/project/mod.rs).

## Project model + serialization conventions
- Runtime model is `Project`/`Segment`/`Token` in [../src/libs/project/models.rs](../src/libs/project/models.rs).
- Saved format is v2 JSON (`SavedProjectV2`), with vocabulary compressed and referenced by indices; derived words use *negative* indices and an index chain for formation rules.
- The saved vocabulary field name is historically misspelled as `orignal` (see `SavedVocabularyV2` in [../src/libs/project/models.rs](../src/libs/project/models.rs)); preserve this in any JSON changes.
- Version migration is centralized in `load_project_from_json()` and `migrate_to_latest()` in [../src/libs/project/importer.rs](../src/libs/project/importer.rs). Add new migrations here and update `PROJECT_VERSION` in [../src/consts/domain.rs](../src/consts/domain.rs).
- Formation rules are Rhai scripts (`FormationRule`) applied to base words; derived word reconstruction happens in [../src/libs/project/importer.rs](../src/libs/project/importer.rs) and export mapping in [../src/libs/project/exporter.rs](../src/libs/project/exporter.rs).

## UI state + caching patterns
- `DecryptionApp` is the single app state object (see [../src/ui/states/state.rs](../src/ui/states/state.rs)). UI updates set dirty flags (`filter_dirty`, `lookups_dirty`, `tfidf_dirty`) and recalc caches on the next frame.
- Similarity search uses TF-IDF (`CachedTfidf`) and is **native-only**; WASM stubs surface a UI error (see `compute_similar_segments()` in [../src/ui/states/state.rs](../src/ui/states/state.rs)).

## Workflows (commands)
- Build desktop: `cargo build --release` (see [../README.md](../README.md)).
- Run tests: `cargo test` (sample migration tests live in [../tests/libs/project/migrate_v1_to_v2.rs](../tests/libs/project/migrate_v1_to_v2.rs)).
- Web dev: `trunk serve`; Web release: `trunk build --release` (see [../README.md](../README.md)).

## External dependencies and platform splits
- Native-only features use `scirs2-text` (TF-IDF similarity) and `image` for app icon; WASM builds use `wasm-bindgen`/`web-sys` (see [../Cargo.toml](../Cargo.toml) and [../src/main.rs](../src/main.rs)).
- File dialogs and async file operations are implemented in `io` and driven from UI actions (look for `pending_*` in [../src/ui/states/state.rs](../src/ui/states/state.rs)).
