# Application and adapter boundaries

The desktop, browser, command-line, and MCP interfaces share a headless `tdector-app::Session`. Each headless CLI invocation owns one session until it exits. With the optional native `gui-control` feature, the CLI can also connect to a long-lived GUI host. The MCP server owns a long-lived session on a dedicated worker thread and calls this application API directly. Native persistence is shared by the CLI, controlled GUI, and MCP.

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

The default CLI depends on the application, evaluator, and native I/O crates without linking a GUI. Its argument parser validates option conflicts and counts stdin consumers before reading input. It converts arguments into requests for `tdector_app::api`; that module owns rule selector resolution, coordinate validation, query response types, and the versioned batch request schema. Saved-project JSON remains a separate format owned by `tdector-file`; the runtime project and caches are not serialized as the CLI query schema. The optional `gui-control` dependency adds `tdector-gui` and `shlex` only to native builds. GUI protocol DTOs, handles, lifecycle metadata, widget identifiers, and transport remain in the GUI adapter, outside `tdector-app`.

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

## Controlled native sessions

The existing desktop binary and CLI-controlled host use the same native runner. `gui start` spawns the GUI-enabled CLI executable with a hidden host command. Its main thread owns the native event loop, application session, typed dialog drafts, and all egui state. Startup configuration travels through a private parent/child channel; startup reports stay separate from diagnostics. Readiness follows the first frame after requested project/font loading and main-window registration. Ordinary desktop launches do not start a listener; controlled launches disable saved layout restoration.

The host binds authenticated loopback TCP to `127.0.0.1` on an allocated port. Each session has a fresh random capability token and a versioned newline-delimited JSON protocol. Requests are limited to 8 MiB; the pending queue is bounded to 64 entries. Startup and command deadlines default to 30 seconds. The listener decodes owned request data, queues it, requests repaint, and writes replies; it never accesses the non-`Send` Rhai session.

`eframe::App::logic` drains requests in order with bounded work per update, including repaint-driven logic while the native UI is hidden. Rendering records observed geometry separately. Commands execute once despite extra egui layout passes. Expired queued commands are rejected before execution. Once execution has started, a lost reply is an unknown outcome, so the client does not retry mutations automatically. State operations can reply immediately; `gui wait` supplies a render barrier.

GUI lifecycle metadata associates an opaque instance handle with its kind, target, pin state, requested geometry, and observed geometry. Drafts remain in the existing typed UI states. Mouse interactions and control operations share lifecycle and edit actions; rendering's temporary `.take()` of a draft is not a close. Close permanently invalidates a handle, pinning preserves it, and project replacement retains the main handle while invalidating project views and generation-scoped controls.

Internal movement and resizing are pending requests consumed by rendering, so subsequent dragging and resizing remain available. Explicit movement removes centered anchoring. Internal coordinates are egui points relative to the viewport; native position is outer-window desktop logical coordinates and native size is client-area size. Inspection reports actual geometry and platform capabilities. Wayland cannot set native outer position and returns `unsupported_operation`; internal movement remains available.

Control bindings preserve immediate versus draft semantics. Translation and base gloss edits call validated application commands immediately. Comment, formation, and tokenizer fields remain drafts until their UI commit action. Derived annotations and previews are read-only. Direct path operations use native I/O for explicit project/font loading, text import, save, and export. Successful saves acknowledge their exact snapshot token. Pending dirty-close confirmations remain associated with the actual operation; shutdown replies before exiting.

The CLI and REPL share Clap definitions. `shlex` only tokenizes REPL input; it runs no shell and performs no expansion. The session locator selects and authenticates the long-lived host. EOF and `exit` detach, while `quit` requests shutdown. See [the GUI-control reference](gui-control.md) for public behavior and reproduction scripts.

## Validation

Application tests exercise atomic failures, no-op and revision behavior, independent sessions, formation chains and comments, query freshness, persistence, and stale save completions. File tests preserve the v1 migration fixture and validate malformed references and rule failures. CLI tests cover process arguments, UTF-8 and stream handling, result envelopes, project round trips, formation/comment scope, and failed or conflicting saves. Native I/O tests cover bounded reads, exact byte and identity baselines, reload candidates, repeated saves, staging cleanup, symlink redirection, hard links, and platform replacement failures. MCP process tests exercise protocol initialization, tool discovery, schemas, revision conflicts, annotation transactions, reload/save behavior, and stdout integrity.

The headless CI job checks the default app, CLI, I/O, and MCP graphs for GUI packages. It also keeps MCP dependencies out of the CLI/GUI graphs and native persistence and MCP dependencies out of the app/WASM graphs. Native tests cover both default and GUI-control CLI builds. GUI-enabled tests and separate GUI-control artifact builds use a distinct target directory. Release builds package `tdector-gui`, headless `tdector`, `tdector-gui-control`, and `tdector-mcp` across the Windows/macOS/Linux matrix, including Alpine musl. WASM build validation remains separate and does not enable the native feature.
