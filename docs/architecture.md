# Application and UI boundaries

The desktop and browser interfaces share a headless `tdector-app::Session`.
CLI and MCP executables are intentionally left for follow-up changes.

## Crates

| Crate | Owns |
| --- | --- |
| `tdector-core` | Runtime `Project`, `Segment`, and `Token` models, indices, filtering, sorting, and cache types |
| `tdector-eval` | Rhai rules, evaluator execution, compilation caches, and evaluation errors |
| `tdector-file` | Saved-format DTOs, migration, JSON conversion, and Typst generation |
| `tdector-text` | Tokenization orchestration, text metrics, and similarity algorithms |
| `tdector-app` | Sessions, validated commands, queries, revisions, and cache invalidation |
| `tdector-gui` | Widgets, drafts, popup state, fonts, and the platform/controller adapters |
| `tdector-wasm` | Browser startup and the instance-local before-unload notification |

The runtime models live in core. File conversion depends on core, never the reverse.
The app crate composes the reusable libraries without depending on any GUI crate.
The GUI's `platform` module owns file dialogs and native/browser execution, and
`controller` delivers their results to the application API. Rendering receives
immutable project data and emits explicit commands; it cannot borrow the session's
project mutably.

## Commands and queries

`Session::execute(Command)` validates an edit and returns whether stored data changed.
A successful no-op does not change the revision or dirty state. Rejected operations
leave the project, revisions, and cached query results intact. Formation changes
are committed as a complete operation, including vocabulary, comments, and every
affected token occurrence.

Available commands cover glosses, translations, word and segment comments, formation
rule creation/application, and removal of the last formation step. Removing a step
recomputes the preceding surface form and affects occurrences sharing the selected
surface, base, and chain. Existing preceding steps are retained.

`project()` provides a read-only domain view. Filtering, lookups, similarity,
related words, script previews, and formation-chain queries work without a frame,
window, or GUI event loop. Query results have no popup or pointer-coordinate fields.
Token annotation queries share base gloss, formation descriptions, inherited display
comments, and editable comment ownership across adapters. Lookup snapshots share
immutable indexes rather than copying the full maps every frame.
Token-text changes have a separate revision so GUI result windows can refresh
without recalculating similarity on each translation/comment edit.

Segment, token, and rule indices are zero-based positions in the current project.
They are not persistent identifiers across imports or project replacement. Future
protocol adapters should associate requests with their session and revision.

## Loading and importing

`load_json` parses, migrates, validates references, and reconstructs the entire
candidate before replacing the current project. A successful load starts clean.
Invalid references and script failures are returned as errors rather than silently
substituting token text.

`import_text` tokenizes all input before changing the session. On success it replaces
segments and the project name while preserving the existing vocabulary, comments,
and formation rules, matching the editor's import behavior. A failed import retains
the current project. GUI presentation state is reset only after success.

Custom tokenization must return strings. Mixed arrays are rejected rather than
silently dropping non-string values.

## Saving and platform I/O

`save_snapshot()` returns JSON bytes and an opaque `SaveToken`. It does not write a
file or mark the session clean. The adapter chooses a path or browser download and,
only after success, calls `acknowledge_saved(token)`.

The token identifies the session, project generation, and revision. A completion
from another session, an older edit, or a replaced project cannot clear current
unsaved changes. Cancellation and I/O failure do not acknowledge the snapshot.
Typst export likewise returns text; writing it belongs to the adapter.

Custom fonts, filenames, popup drafts, filtering input, and pagination are GUI
state. Browser unload handling uses a flag associated with that GUI instance;
there is no thread-global project dirty flag.

Open and text-import requests capture the current session revision. If the project
changes while a browser file read is pending, the adapter rejects that old result
and asks the user to retry, preserving the newer project and its unsaved changes.

## Execution ownership

Rhai rules currently contain `Rc<OnceCell<AST>>`, so a session has a single owner
and is not `Send` or `Sync`. A future concurrent adapter can create the session
on an owning worker and pass data-only commands/results to it. It must not assume
that wrapping the project in a mutex makes it transferable.

Evaluator print/debug output is directed away from native stdout so scripts cannot
mix diagnostics into future machine-readable output. Browser evaluation does not
write native protocol output.

## Validation

Application tests exercise atomic failures, no-op and revision behavior, independent
sessions, formation chains and comments, query freshness, persistence, and stale save
completions. File tests preserve the v1 migration fixture and validate malformed
references and rule failures. The headless CI job checks the app dependency graph
for GUI packages before testing the reusable crates.
