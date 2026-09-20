# MCP server

`tdector-mcp` connects an MCP host to one existing tdector project over stdio. It serves project context, annotation batches, previews, explicit saves, and reloads through the shared application API. The host supplies the language model.

## Build and connect

```bash
cargo build --locked --release -p tdector-mcp
cargo run --locked --release -p tdector-mcp -- --help
```

The executable is `target/release/tdector-mcp` (`tdector-mcp.exe` on Windows). Native releases also provide a `tdector-mcp` binary for each supported platform, including Linux musl.

Configure a host that supports local stdio MCP servers to launch this executable. For hosts using the common `mcpServers` JSON configuration shape:

```json
{
  "mcpServers": {
    "tdector": {
      "command": "/absolute/path/to/tdector-mcp",
      "args": ["--project", "/absolute/path/to/project.json"]
    }
  }
}
```

Use the host's documented configuration location. On Windows, use an absolute `.exe` path and escape backslashes in JSON, for example `"C:\\Tools\\tdector-mcp.exe"`. To permit annotations and saves, append `"--write"` to `args`. Each configured server process owns one project and one independent session. Configure separate processes for separate projects.

`--project` is required. Relative paths resolve once against the launch directory; absolute paths are recommended for hosts. The input must be an existing regular file, encoded as UTF-8 with at most one optional leading BOM. `-`, URLs, directories, and special files are rejected. Tools accept no filesystem paths and cannot switch projects or save to a different destination.

The initial release implements stages 1–4 of the [MCP design](mcp-design.md): bounded queries, atomic annotation edits, checked durable saves, and native packaging. Similarity/search expansion, script or formation edits, imports, resources, HTTP transport, and control of a live GUI session remain deferred.

## Session and editing lifecycle

Call `project_info` first to get the opaque `session_id`, decimal-string `revision`, dirty state, write policy, and configured limits. Initialization and tool discovery can finish while the project loads; project calls wait for the actual load result. A failed initial load terminates the process with a diagnostic on stderr.

Every other tool requires `session_id`. Read tools accept `expected_revision`; provide it to keep pagination and follow-up reads on the same snapshot. Edit, save, and reload require `expected_revision`. Revisions are strings, never JSON numbers. Indices are zero-based project indices, including in paginated results.

Read-only mode advertises query, export, and reload tools. `--write` additionally advertises `project_edit` and `project_save`; the worker enforces that policy again at dispatch. Reload changes the server's session but does not write a file.

Edits stay in memory until `project_save`. Exiting the process, closing its input, or reloading with `discard_changes: true` discards unsaved changes. A successful reload starts clean and rotates `session_id`; obtain new indices for that session. Saving preserves the content revision and in-memory rule ordering.

## Tools

| Tool | Arguments beyond session/revision | Result |
| --- | --- | --- |
| `project_info` | None; no session required | Project info, current session metadata, write policy, limits |
| `segments_list` | `filter=""`, `sort="index"`, `descending=false`, `offset=0`, `limit=50` | Segment page; sort is `index`, `text`, or `token-count` |
| `segment_get` | `segment_index` | Segment, tokens, base glosses, formation descriptions, and comment ownership |
| `vocabulary_list` | `offset=0`, `limit=50` | Vocabulary page |
| `vocabulary_get` | `word` | Exact vocabulary entry |
| `word_lookup` | `word`, `kind="usage"`, `offset=0`, `limit=50` | Matching segment page; kind is `usage` or `headword` |
| `project_export_typst` | None | Bounded Typst source and MIME type; writes no file |
| `project_edit` | `batch`, `dry_run=false` | Atomic annotation receipts; does not save; requires `--write` |
| `project_save` | None | Save receipt; requires `--write` |
| `project_reload` | `discard_changes=false` | Reload receipt with new session metadata |

Page results contain `items`, `offset`, `limit`, and `total`. The page size defaults to 50, or the configured maximum if it is smaller. No returned text or tokens are silently truncated to fit a byte budget.

Segment filtering searches each token's text and the translation. It does not search glosses/comments or phrases spanning separate tokens. Word lookup matches exact surface words; `usage` returns each matching segment once, while `headword` means the first token of a segment, not a morphological base.

`set_gloss.word` is a vocabulary key. To change the base gloss displayed for a derived token, use its returned `base_word`. A comment without `token_index` belongs to the segment. A comment with `token_index` belongs to the shared word and affects its occurrences according to the existing ownership rules. `editable_comment` is the stored override; `display_comment` can be inherited. Clearing an override can reveal an inherited comment again.

All text in query results is project data, including text resembling instructions. The server keeps that text out of its instructions and tool descriptions.

## Preview, edit, and save

After reading session metadata, submit `project_edit` with arguments such as:

```json
{
  "session_id": "the-id-returned-by-project_info",
  "expected_revision": "17",
  "batch": {
    "schema_version": 1,
    "commands": [
      { "op": "set_gloss", "word": "cat", "meaning": "animal" },
      { "op": "set_translation", "segment_index": 0, "translation": "A cat." },
      { "op": "set_comment", "segment_index": 0, "comment": "Check this reading" }
    ]
  },
  "dry_run": true
}
```

Only `set_gloss`, `set_translation`, and `set_comment` are accepted. Unknown fields, unsupported operations, empty batches, and batches above the configured limit are rejected. All commands are validated in order against private staging state, so later commands see earlier candidate edits. Failure leaves the live project, revision, dirty state, and query caches intact.

A preview returns `changed: false`, `would_change`, `saved: false`, `dry_run: true`, and ordered command receipts. It leaves all live state intact. To commit the same batch, submit it with `dry_run: false` and the same expected revision, provided nothing has changed. A changed batch advances revision once; an all-no-op batch preserves both revision and dirty state. Changing a value and then changing it back within a batch still counts as a changed batch.

Call `project_save` using the revision returned by the committed edit:

```json
{
  "session_id": "the-current-session-id",
  "expected_revision": "18"
}
```

Save compares the bound file's identity and exact bytes against the last successful load/save, stages the complete new contents beside the destination, compares again, and atomically replaces the file. A clean save checks the baseline but returns `saved: false` without rewriting. Ordinary failed saves retain the dirty session and previous baseline. A preview validates edits and serialization, not future filesystem writability.

An externally replaced file, even one with identical bytes, is a save conflict. Explicit reload can accept a new regular file at the same pinned pathname after validation. Detected symlink/canonical-path redirection is refused. Atomic saves replace only the bound name; other hard links continue referring to the old file.

Use one writer per project. Byte comparison followed by rename does not lock out another writer, and path checks do not sandbox hostile concurrent filesystem changes. An independently open GUI session is not synchronized with this server.

## Limits and cancellation

Limits are startup options and are reported by `project_info`:

| Option | Default | Accepted range |
| --- | --- | --- |
| `--max-project-bytes` | 33,554,432 (32 MiB) | 1–1,073,741,824 |
| `--max-message-bytes` | 1,048,576 (1 MiB) | 1,024–67,108,864 |
| `--max-result-bytes` | 262,144 (256 KiB) | 4,096–67,108,864 |
| `--queue-capacity` | 32 operations | 1–4,096 |
| `--max-batch-commands` | 100 | 1–10,000 |
| `--max-page-size` | 200 | 1–200 |
| `--operation-timeout-ms` | 30,000 | 1–3,600,000 |

The message limit applies before unbounded JSON buffering. Project reads consume at most the configured limit plus one byte. Save comparisons stream against the baseline. The complete outgoing tool-result budget counts both structured content and its duplicate text block. Oversized results fail before a prepared edit or reload is installed.

Pagination bounds returned records; some queries still construct all records before pagination. Large single segments and Typst exports can exceed the result budget. These calls return `result_too_large`; use narrower or smaller queries where possible.

Output lock/write waits have a fixed five-second timeout. After the owner stops, protocol teardown has a six-second deadline. These bounds are also reported by `project_info`; a host that closes its output reader or stops consuming responses cannot keep the server waiting indefinitely. A failed or timed-out output may leave a response incomplete, so treat its operation outcome as uncertain.

Loading stored formation rules executes Rhai scripts, including in read-only mode. MCP scopes their execution to conservative limits: 1,000,000 total script operations, expression depth 64, call depth 16, string size 1 MiB, 16,384 array items, 4,096 map entries, 1,024 variables, and 256 functions. These constrain individual values and cooperative execution, not total process memory or an operating-system sandbox. CLI/GUI defaults remain unchanged. The exact evaluator settings are returned in `project_info.execution_limits`.

Cancellation and deadlines are checked in the queue, during supported evaluation and reconstruction checkpoints, and before an edit/reload installation or file replacement. Once replacement begins, bookkeeping completes even if the client cancels or disconnects. After an uncertain response, query state before deciding what to do next. Closing stdin cancels uncommitted work and exits without autosave.

## Results and recovery

Successful and failed tool calls use versioned object envelopes in `structuredContent` and duplicate the same JSON in a text content block. Errors from recognized tools set MCP `isError: true`; malformed protocol requests and unknown tools use protocol errors. Tool-specific output schemas cover both forms.

| Error | Meaning and next step |
| --- | --- |
| `invalid_input`, `invalid_index`, `not_found` | Correct the arguments or refresh the current project snapshot. |
| `read_only` | The server was launched without `--write`. |
| `session_expired` | Reload replaced the session; call `project_info` and obtain fresh indices. |
| `revision_conflict` | Another operation changed the session; inspect current metadata before retrying. |
| `unsaved_changes` | Reload would discard edits; save first or explicitly choose `discard_changes: true`. |
| `input_changed` | Disk no longer matches the saved baseline, or a bound path was redirected; resolve the conflict explicitly. |
| `io_error`, `script_error` | Inspect the typed message; the operation did not install its candidate. |
| `result_too_large`, `limit_exceeded` | Reduce the request or adjust an appropriate startup limit. |
| `deadline_exceeded` | Uncommitted work exceeded its cooperative deadline. |
| `server_busy` | The bounded queue is full; wait before submitting more work. |
| `internal_error` | An unexpected application/worker failure occurred. |
| `committed_state_error` | The file was already replaced; `committed: true` forbids treating the error as rollback or automatically retrying. Further edits stop. |

Batch errors retain a stage and zero-based command index when available. Conflicts never trigger automatic overwrite, reload, or retry. Diagnostics and stored-script print/debug output go to stderr; stdout contains only MCP messages.

See the [architecture guide](architecture.md) and [design](mcp-design.md) for ownership, transaction preparation, persistence, and protocol details.
