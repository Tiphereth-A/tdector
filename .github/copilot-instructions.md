# Copilot Instructions for tdector

A GUI application for assisted text decryption, translation, and linguistic analysis. Built with Rust using egui (desktop) and WASM (web).

## Project Overview

**tdector** is a cross-platform linguistic annotation tool that helps users:
- Import and tokenize text (word or character-based)
- Build project-wide vocabularies with persistent glosses
- Translate and annotate segments
- Search similar segments using TF-IDF similarity
- Export to Typst for academic typesetting
- Apply custom word formation rules (derivation, inflection)

Targets: Desktop (egui) and Web (WASM via Trunk)

## Architecture & Key Components

### Layered Architecture (5 modules)

**`consts/`** - Application-wide constants
- `domain.rs`: Business rule thresholds (PROJECT_VERSION, limits)
- `colors.rs`, `ui.rs`: Visual theming and UI dimensions
- Centralized configuration to avoid magic numbers

**`libs/`** - Domain logic layer (business rules, pure functions)
- `models.rs`: `Project`, `Segment`, `Token` (runtime), plus `SavedProject` (serialized)
- `cache.rs`: `LookupCache`, `CachedTfidf` for expensive computations
- `text_analysis.rs`: Tokenization, normalization, linguistic processing
- `similarity.rs`: TF-IDF similarity engine (disabled in WASM; requires SciRS2 v0.3.0)
- `filtering.rs`, `sorting.rs`: Segment filtering and sort operations
- `formation.rs`: Word formation rules (Rhai scripting for transformations)

**`enums/`** - Core enum types
- `AppAction`: High-level user intentions (Import, Open, Export, Quit)
- `AppError`: Error type with `AppResult<T>` alias
- `PopupRequest`, `DictionaryPopupType`: UI popup variants
- `SortMode`, `SortDirection`, `SortField`: Filtering/sorting modes
- `FormationType`: Derivation, Inflection, Nonmorphological

**`io/`** - I/O and persistence layer
- `file_io.rs`: Unified async file I/O (rfd for dialogs, supports desktop + WASM)
- `project.rs`: Load/save JSON; handles `SavedProject` ↔ `Project` conversion with migration
- `json_formatter.rs`: Custom JSON serialization with vocabulary indexing
- `typst.rs`: Export annotated projects to Typst format
- `file_ops.rs`: Font loading and registration

**`ui/`** - Presentation layer (egui immediate-mode rendering)
- `states/state.rs`: `DecryptionApp` struct with all application state
- `states/update.rs`: eframe::App implementation; orchestration/update loop
- `states/mod.rs`: State initialization
- `panels.rs`, `dialogs.rs`: UI components for filters, vocabulary, segments
- `segment.rs`: Token-level rendering with right-click word selection
- `popups/`, `popup_utils.rs`: Modal windows (definitions, references, comments)
- `menu.rs`: Application menu bar (File, Edit, View, Tools)
- `highlight.rs`: Text styling, colors, fonts

### Data Flow

1. **Import**: User imports text file → tokenized into `Segment`s/`Token`s → state updated
2. **Vocabulary**: Right-click word → definition lookup/edit → `Project.vocabulary` updated
3. **Filter/Sort**: User types/changes sort mode → `FilterOperation::apply_filter` + `SortOperation::apply_sort` → `cached_filtered_indices` recalculated
4. **Similarity Search**: On demand → TF-IDF cache validated → ranked results returned (desktop only)
5. **Export**: User exports → `generate_typst_content()` writes Typst markup
6. **Persistence**: Save/load uses `convert_to_saved_project()` / `convert_from_saved_project()` with vocabulary deduplication

## Key Patterns & Conventions

### Command/State Pattern
- **`DecryptionApp` struct** holds all mutable state (project, UI state, caches, popups)
- **`update.rs` orchestrator** processes events → calls domain functions → updates state
- No global state; all state flows through `DecryptionApp`

### Cache Invalidation Strategy
- **`filter_dirty`, `lookups_dirty`, `tfidf_dirty` flags**: Track which caches are stale
- **Lazy validation**: Caches rebuilt only when accessed if flags are set
- **Incremental updates**: Small changes mark relevant caches dirty; all caches recalculated together only when needed

### Vocabulary Deduplication
- **Runtime**: Each `Token` references vocab by string key (`Token.original`)
- **Saved format**: `SavedProject` stores vocab as indexed lookup table
- Reduces file size; same token appearing multiple times shares single definition

### Popup Management
- **`definition_popup`, `reference_popup`, `similar_popup`, etc.**: Modal state fields
- **`pinned_popups: Vec<PinnedPopup>`**: Persistent windows users can keep open
- **`next_popup_id`**: Unique ID for each popup instance
- Right-click → word menu popup positioned at mouse

### Formation Rules (Rhai Scripting)
- **`FormationRule` struct**: Pattern + script for deriving/inflecting words
- **`FormationType`**: Derivation, Inflection, Nonmorphological
- Applied via context menu; previewed before applying; builds vocabulary connections
- Stored in `Project.formation_rules`; referenced by `Token.formation_rule_indices`

### Cross-Platform (Desktop + WASM)
- **Platform-gated code**:
  - `#[cfg(target_arch = "wasm32")]`: WASM-specific main() in [src/main.rs](../src/main.rs) (~line 36)
  - `#[cfg(not(target_arch = "wasm32"))]`: Desktop-specific in [src/main.rs](../src/main.rs) (~line 63)
  - TF-IDF similarity disabled on WASM: `#[cfg(not(target_arch = "wasm32"))]` guards in [src/ui/states/state.rs](../src/ui/states/state.rs) (~line 125)
- **Async I/O**: Uses `rfd` for file dialogs on both platforms; callbacks update `pending_*` fields
- **Web entry**: Canvas ID = `"the_canvas_id"` in [index.html](../index.html)

## Build & Development

### Desktop Build
```bash
cargo build --release
```
Produces standalone executable; full TF-IDF support; custom font loading via system file dialog.

### Web Development (Hot-Reload)
```bash
trunk serve
```
Compiles to WASM; hot-reload enabled; runs at `http://localhost:8080`
Trunk config: [Trunk.toml](../Trunk.toml)

### Web Release Build
```bash
trunk build --release
```
Optimized WASM in `dist/`; ready for static hosting.

## Important Workflows

### Adding a New Feature
1. **Domain logic** → `libs/` module (pure, testable functions)
2. **UI integration** → add enum variant to `AppAction`/`UiAction`/`PopupRequest` in `enums/`
3. **Rendering** → add UI code in `ui/` (use `states/update.rs` orchestration)
4. **Cache handling** → mark dirty flags if feature modifies project state

### Fixing Bugs in Serialization
- Check [src/io/project.rs](../src/io/project.rs) for migration logic
- Ensure `SavedProject` version bumped in [src/consts/domain.rs](../src/consts/domain.rs)
- Vocabulary indexing handled in [src/io/json_formatter.rs](../src/io/json_formatter.rs)

### Extending Filters/Sorting
- Add variant to `SortField`/`SortMode` in [src/enums/sort_mode.rs](../src/enums/sort_mode.rs)
- Implement logic in [src/libs/filtering.rs](../src/libs/filtering.rs) or [src/libs/sorting.rs](../src/libs/sorting.rs)
- Hook in [src/ui/states/update.rs](../src/ui/states/update.rs) render loop

### Adding a Popup Type
1. Add variant to `DictionaryPopupType` or `PopupRequest` in [src/enums/popups.rs](../src/enums/popups.rs)
2. Add state field to `DecryptionApp` in [src/ui/states/state.rs](../src/ui/states/state.rs)
3. Implement rendering in [src/ui/popups/](../src/ui/popups/)
4. Wire up in [src/ui/states/update.rs](../src/ui/states/update.rs) event handling

## Dependencies & Notable Libraries

- **egui 0.33.0**: Immediate-mode GUI (desktop + web)
- **eframe**: egui windowing/rendering backend (supports glow for desktop, WASM for web)
- **serde + serde_json**: Serialization (JSON storage)
- **ndarray 0.17.2**: TF-IDF matrix operations
- **rhai 1.24**: Scripting engine for formation rules
- **scirs2-text 0.1.3**: TF-IDF (desktop only; WASM blocked)
- **rfd 0.17**: File dialog (cross-platform)
- **web-sys + wasm-bindgen**: WASM bindings for browser APIs (file access, canvas)

## Error Handling

- Use `AppError` enum for domain errors (not String errors)
- `AppResult<T> = Result<T, AppError>` throughout libs
- Display errors in UI via `error_message` field in state; shown in status bar
- Async I/O errors stored in `pending_*` fields; checked in `process_pending_file_operations()`

## Performance Considerations

- **TF-IDF caching**: Expensive; memoized in `CachedTfidf`; disabled in WASM
- **Filtering**: Applied to all segments; cached in `cached_filtered_indices`
- **Vocabulary lookups**: Hashed in `LookupCache`
- Avoid rebuilding these unless `*_dirty` flags are set
