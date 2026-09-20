# Application and adapter boundaries

The desktop, browser, command-line, and MCP interfaces share a headless `tdector-app::Session`. Each CLI invocation owns one session until it exits. The MCP server owns a long-lived session on a dedicated worker thread and calls this application API directly. Native persistence is shared by the CLI and MCP.

## Crates

| Crate | Owns |
| --- | --- |
| `tdector-core` | Runtime `Project`, `Segment`, and `Token` models, indices, filtering, sorting, and cache types |
| `tdector-eval` | Rhai rules, evaluator execution, compilation caches, and evaluation errors |
| `tdector-file` | Saved-format DTOs, migration, JSON conversion, and Typst generation |
| `tdector-text` | Tokenization orchestration, text metrics, and similarity algorithms |
| `tdector-app` | Sessions, validated commands, queries, revisions, cache invalidation, and reusable API request/response types |
| `tdector-io` | Native UTF-8 decoding, file identities, bounded project bindings, conflict checks, and atomic file replacement |
| `tdector-cli` | The `tdector` binary: argument parsing, stdin/stdout handling, persistence policy, text/JSON reports, and exit codes |
| `tdector-mcp` | The `tdector-mcp` binary: stdio protocol, generated schemas, bounded request queue, session envelopes, worker ownership, and launch-bound persistence policy |
| `tdector-gui` | Widgets, drafts, popup state, fonts, and the platform/controller adapters |
| `tdector-wasm` | Browser startup and the instance-local before-unload notification |

The runtime models live in core. File conversion depends on core, never the reverse. The app crate composes the reusable libraries without depending on any GUI crate. The GUI's `platform` module owns file dialogs and native/browser execution, and `controller` delivers their results to the application API. Rendering receives immutable project data and emits explicit commands; it cannot borrow the session's project mutably.

The CLI depends on the application, evaluator, and native I/O crates without linking a GUI. Its argument parser validates option conflicts and counts stdin consumers before reading input. It converts arguments into requests for `tdector_app::api`; that module owns rule selector resolution, coordinate validation, query response types, and the versioned batch request schema. Saved-project JSON remains a separate format owned by `tdector-file`; the runtime project and caches are not serialized as the CLI query schema.

## Commands and queries

`Session::execute(Command)` validates an edit and returns whether stored data changed. A successful no-op does not change the revision or dirty state. Rejected operations leave the project, revisions, and cached query results intact. Formation changes are committed as a complete operation, including vocabulary, comments, and every affected token occurrence.

Available commands cover glosses, translations, word and segment comments, formation rule creation/application, and removal of the last formation step. Removing a step recomputes the preceding surface form and affects occurrences sharing the selected surface, base, and chain. Existing preceding steps are retained.

`project()` provides a read-only domain view. Filtering, lookups, similarity, related words, script previews, and formation-chain queries work without a frame, window, or GUI event loop. Query results have no popup or pointer-coordinate fields. Token annotation queries share base gloss, formation descriptions, inherited display comments, and editable comment ownership across adapters. Lookup snapshots share immutable indexes rather than copying the full maps every frame. Token-text changes have a separate revision so GUI result windows can refresh without recalculating similarity on each translation/comment edit.

Segment, token, and rule indices are zero-based positions in the current project. They are not persistent identifiers across imports or project replacement. MCP requests include a session ID and can require a specific revision; successful reload rotates that ID and invalidates old indices. Rules are sorted when saved, so their indices can also change after save/reload. Description selectors require an exact, unique match; ambiguous descriptions return their matching indices so the caller can use a snapshot-local rule index.

## Loading and importing

`load_json` parses, migrates, validates references, and reconstructs the entire candidate before replacing the current project. A successful load starts clean. Invalid references and script failures are returned as errors rather than silently substituting token text. `prepare_load_json` and `commit_load` expose the same candidate/installation boundary for the MCP worker, which checks cancellation, deadlines, and response size before installing a reload. Failed or cancelled preparation retains the live project, revisions, save authority, and caches.

`import_text` tokenizes all input before changing the session. On success it replaces segments and the project name while preserving the existing vocabulary, comments, and formation rules, matching the editor's import behavior. A failed import retains the current project. GUI presentation state is reset only after success.

Custom tokenization must return strings. Mixed arrays are rejected rather than silently dropping non-string values.

## Saving and platform I/O

`save_snapshot()` returns JSON bytes and an opaque `SaveToken`. It does not write a file or mark the session clean. The adapter chooses a path or browser download and, only after success, calls `acknowledge_saved(token)`.

The token identifies the session, project generation, and revision. A completion from another session, an older edit, or a replaced project cannot clear current unsaved changes. Cancellation and I/O failure do not acknowledge the snapshot. Typst export likewise returns text; writing it belongs to the adapter.

The CLI requires an explicit output, in-place save, or dry run for every edit. It serializes completely before staging a file in the destination directory and commits with the platform replacement operation, without deleting the original first. Existing separate outputs require `--overwrite`. Identity checks reject separate outputs that alias the source, including existing hard links. An in-place save compares the source bytes with those originally loaded immediately before commit; observed changes reject the save. This assumes one writer and does not coordinate with a simultaneous GUI writer. A no-op in-place edit does not rewrite the file. Revisions and save tokens stay process-local.

Batch requests are parsed completely, then applied sequentially to the CLI's private session. A command or serialization failure discards that invocation's session and leaves the destination unchanged; a successful batch commits once. This is a file-level guarantee. For the long-lived MCP session, app-owned `prepare_batch` clones runtime project data into private staging state and `commit_batch` installs it only after the complete batch, serialization, response budget, and cancellation checks succeed. A preview drops staging. Preparation never round-trips through saved JSON, which could reorder rules or run scripts. Changed annotation batches advance the content revision once while retaining text-analysis caches; no-op batches preserve revision and dirty state.

MCP binds one existing project at launch and defaults to read-only. `--write` enables in-memory annotations and explicit saves. `tdector-io::BoundProject` retains the canonical pathname, file identity, and exact loaded bytes, including an optional UTF-8 BOM. Save checks this baseline before staging and again before replacement; clean saves check without rewriting. Replacement refreshes the baseline, and only then does the worker acknowledge its save token. A failure in bookkeeping after replacement is reported as committed and stops further edits.

Reload reads a bounded candidate and allows an external file replacement at the same pinned pathname. It rejects detected path redirection and refuses dirty state unless the caller explicitly discards changes. Neither comparison plus rename nor path checks provide interprocess locking or a filesystem sandbox. Separate GUI/CLI/MCP processes remain single-writer workflows.

Artifact output to stdout is generated completely before writing. It cannot share stdout with a report, and `--json` is rejected for that combination. Stream errors can still leave partial artifact bytes in a pipe. Versioned JSON reports distinguish typed domain, script, I/O, and persistence failures; script diagnostics use stderr.

Custom fonts, filenames, popup drafts, filtering input, and pagination are GUI state. Browser unload handling uses a flag associated with that GUI instance; there is no thread-global project dirty flag.

Open and text-import requests capture the current session revision. If the project changes while a browser file read is pending, the adapter rejects that old result and asks the user to retry, preserving the newer project and its unsaved changes.

## Execution ownership

Rhai rules contain `Rc<OnceCell<AST>>`, so a session has a single owner and is not `Send` or `Sync`. MCP constructs and loads the session on a dedicated owner thread; a bounded channel carries owned requests, cancellation flags, and reply channels. Only owned data and error DTOs cross the thread boundary. Identity, revision, write policy, and cancellation are checked when work is dequeued. Protocol initialization and tool discovery do not wait for loading; project calls wait for the real load result rather than observing an empty default session.

The worker applies a scoped evaluator policy with expression/call depth, allocation, cancellation, and deadline bounds, including during initial load and reload. Other adapters retain their existing evaluator defaults. Cancellation is cooperative: after file replacement begins, the worker completes bookkeeping and acknowledges the save. EOF cancels uncommitted work and never autosaves edits.

Evaluator print/debug output is directed away from native stdout so scripts cannot mix diagnostics into CLI machine-readable output. Browser evaluation does not write native protocol output.

## Validation

Application tests exercise atomic failures, no-op and revision behavior, independent sessions, formation chains and comments, query freshness, persistence, and stale save completions. File tests preserve the v1 migration fixture and validate malformed references and rule failures. CLI tests cover process arguments, UTF-8 and stream handling, result envelopes, project round trips, formation/comment scope, and failed or conflicting saves. Native I/O tests cover bounded reads, exact byte and identity baselines, reload candidates, repeated saves, staging cleanup, symlink redirection, hard links, and platform replacement failures. MCP process tests exercise protocol initialization, tool discovery, schemas, revision conflicts, annotation transactions, reload/save behavior, and stdout integrity.

The headless CI job checks the app, CLI, I/O, and MCP graphs for GUI packages. It also keeps MCP dependencies out of the CLI/GUI graphs and native persistence and MCP dependencies out of the app/WASM graphs. Native release builds package `tdector-gui`, `tdector`, and `tdector-mcp` across the Windows/macOS/Linux matrix, including the Alpine musl target. WASM build validation remains separate.
