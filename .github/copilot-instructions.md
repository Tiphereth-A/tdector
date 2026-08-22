# Copilot instructions for tdector

## Big picture architecture
- GUI app built with `eframe`/`egui`: native desktop uses `tdector-gui`, while the WASM entry point lives in [../tdector-wasm/src/main.rs](../tdector-wasm/src/main.rs) and reuses the GUI state (`DecryptionApp`).
- Core domain logic lives in [../tdector-core/src/libs](../tdector-core/src/libs/mod.rs): project model/serialization, caching, filtering/sorting, and persistence support.
- NLP logic lives in [../tdector-text/src](../tdector-text/src/lib.rs): tokenization, text analysis, token similarity, and TF-IDF sentence similarity.
- Rhai script evaluation is isolated in [../tdector-eval/src](../tdector-eval/src/lib.rs), which provides formation and tokenization rules consumed by core and the GUI.
- File dialogs, font loading, and async file operations live in [../tdector-file/src/lib.rs](../tdector-file/src/lib.rs); GUI-specific operation orchestration lives in [../tdector-gui/src/ui/file_ops.rs](../tdector-gui/src/ui/file_ops.rs).
- Data flow: UI reads/writes `Project` state, then persists via project export/import helpers and JSON migration in [../tdector-file/src/project](../tdector-file/src/project/mod.rs).

## Project model + serialization conventions
- Runtime model is `Project`/`Segment`/`Token` in [../tdector-file/src/project/models.rs](../tdector-file/src/project/models.rs).
- Saved format is v2 JSON (`SavedProjectV2`), with vocabulary compressed and referenced by indices; derived words use *negative* indices and an index chain for formation rules.
- Version migration is centralized in `load_project_from_json()` and `migrate_to_latest()` in [../tdector-file/src/project/importer.rs](../tdector-file/src/project/importer.rs). Add new migrations here and update the project format version in [../tdector-file/src/project/importer.rs](../tdector-file/src/project/importer.rs).
- Formation rules are Rhai scripts (`FormationRule`) owned by [../tdector-eval/src](../tdector-eval/src/lib.rs) and applied to base words; derived word reconstruction happens in [../tdector-file/src/project/importer.rs](../tdector-file/src/project/importer.rs) and export mapping in [../tdector-file/src/project/exporter.rs](../tdector-file/src/project/exporter.rs).

## UI state + caching patterns
- `DecryptionApp` is the single app state object (see [../tdector-gui/src/ui/states/state.rs](../tdector-gui/src/ui/states/state.rs)). UI updates set dirty flags (`filter_dirty`, `lookups_dirty`, `tfidf_dirty`) and recalc caches on the next frame.
- Similarity search uses TF-IDF (`CachedTfidf`) from `tdector-text` and is **native-only**; WASM stubs surface a UI error (see `compute_similar_segments()` in [../tdector-gui/src/ui/states/state.rs](../tdector-gui/src/ui/states/state.rs)).

## Workflows (commands)
- Build desktop: `cargo build --release` (see [../README.md](../README.md)).
- Run tests: `cargo test` (sample migration tests live in [../tdector-core/tests/libs/project/migrate_v1_to_v2.rs](../tdector-core/tests/libs/project/migrate_v1_to_v2.rs)).
- Web dev: `cd tdector-wasm && trunk serve`; Web release: `cd tdector-wasm && trunk build --release` (see [../README.md](../README.md)).

## External dependencies and platform splits
- Native-only features use `image` for the app icon; NLP dependencies such as `scirs2-text` and `textdistance` are isolated in `tdector-text`; browser startup and browser APIs are isolated in `tdector-wasm` (see [../tdector-wasm/Cargo.toml](../tdector-wasm/Cargo.toml) and [../tdector-wasm/src/main.rs](../tdector-wasm/src/main.rs)).
- File dialogs and async file operations are implemented in [../tdector-file/src/lib.rs](../tdector-file/src/lib.rs) and driven from UI actions (look for `pending_*` in [../tdector-gui/src/ui/states/state.rs](../tdector-gui/src/ui/states/state.rs)).
