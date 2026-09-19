# CLI interface

Status: implemented by the `tdector-cli` crate. Build only the headless executable
with `cargo build --locked -p tdector-cli`; run it with
`cargo run --locked -p tdector-cli -- COMMAND` or `target/debug/tdector COMMAND`.

The `tdector-cli` crate provides a `tdector` binary. Each invocation creates one
`tdector_app::Session`, optionally loads a project, performs its operation, emits
its result, and exits. The existing desktop binary remains `tdector-gui`.

## Invocation and common options

```text
tdector [--project FILE|-] [--json] COMMAND
```

| Option | Contract |
| --- | --- |
| `-p, --project FILE` | Load an existing project. Required for project queries and edits; no directory discovery or remembered current project. |
| `-p, --project -` | Read the project JSON from stdin. |
| `--json` | Emit a versioned result envelope on stdout for query/edit reports, including failures. Default reports are human-readable text. |
| `-h, --help` | Display help; nested commands have their own help. |
| `-V, --version` | Display CLI version. |

Common options may appear before or after a subcommand. The interface is
noninteractive: missing arguments are errors, and no command opens a dialog.
Text and JSON file inputs use UTF-8. A single leading UTF-8 BOM is accepted.
Arguments after `--` are positional values, allowing paths beginning with `-`.

Only one input source may consume stdin in an invocation. For example,
`--project -` conflicts with `--text-file -`, a script from stdin, or a batch from
stdin. Validate these conflicts before reading any input.

## Saving

Every project mutation requires exactly one of the following:

| Option | Contract |
| --- | --- |
| `-o, --output FILE` | Save the resulting project to a separate file. Existing files require `--overwrite`. |
| `-o, --output -` | Emit the resulting saved-project JSON directly on stdout. |
| `--in-place` | Replace the file named by `--project`; requires a real input path. |
| `--dry-run` | Execute and serialize in memory, then report the proposed change without writing a project. |

`--overwrite` requires an explicit `--output FILE` and conflicts with
`--in-place`, `--dry-run`, and `--output -`. For project mutations, an output
referring to the input file must use `--in-place`. Export always requires a
different destination when writing to a file; same-file export is rejected even
with `--overwrite`. Input and output paths are never inferred from the project's
embedded name.

A regular file write stages the fully serialized result in the destination
directory and commits it using the platform's atomic file replacement mechanism.
A failed command or failed staged write preserves the previous destination.
There must be no delete-original-then-rename fallback. A no-op in-place edit
does not rewrite the source; an explicit separate output still produces a copy.
Call `acknowledge_saved` only after a successful commit.

Before an in-place commit, compare the input bytes with the bytes originally
loaded and reject an observed change with `input_changed`. This detects changes
observed before commit; it does not promise coordination with a simultaneous GUI
writer. Version 1 assumes one writer per project. Session revisions and save
tokens are process-local and must not be advertised as persistent file revisions.

Raw artifact output and reports must not share stdout. `--output -` emits only
the saved project; `export` to stdout emits only its artifact. Reject `--json`
when stdout carries an artifact. Generate the artifact completely before writing
it; an output-stream I/O failure can still leave partial bytes in a pipe.
Normal text-mode errors and script diagnostics go to stderr.

## Command surface

All commands below except fresh import, standalone previews, and help/version
require `--project`. The write options above apply to every command marked edit.

| Command | Principal options | Behavior |
| --- | --- | --- |
| `info` | none | Project name, segment/token/vocabulary/rule counts, and supported project format. |
| `validate` | none | Parse, migrate, validate references, and reconstruct derived words. Does not rewrite the input. |
| `import INPUT` | `--name NAME`, tokenizer options; optional `--project BASE --replace-text`; write options | Create a project from text, or explicitly replace the text of a loaded project. Edit. |
| `segment list` | `--filter TEXT`, `--sort index|text|token-count`, `--descending`, pagination | List matching segments with their original indices and translations. |
| `segment show INDEX` | none | Segment, translation, comment, and annotated tokens, including each token index. |
| `segment translate INDEX` | text options; write options | Set or clear the segment translation. Edit. |
| `vocab list` | pagination | List vocabulary entries in lexical word order. |
| `vocab get WORD` | none | Exact base vocabulary entry, including its gloss and own comment. |
| `vocab set WORD` | text options; write options | Set a gloss, creating a vocabulary entry when needed. Edit. |
| `vocab search TEXT` | `--limit N` | Case-insensitive substring search of vocabulary words, prefix matches first. |
| `comment get` | `--segment N`, optional `--token N` | Read a segment comment or the selected token's editable comment target. |
| `comment set` | same target options; text options; write options | Set or clear that comment. Edit. |
| `lookup WORD` | `--kind usage|headword`, pagination | Exact surface-word lookup; usage is the default. |
| `similar segments INDEX` | `--limit N` | TF-IDF similarity to other segments. |
| `similar tokens WORD` | `--limit N` | Token spelling similarity; version 1 accepts limits from 1 through 20. |
| `rule list` | pagination | Rule indices, descriptions, and types. |
| `rule show` | rule selector | Show the selected rule, including its script. |
| `rule add` | `--description TEXT`, `--type TYPE`, `--script-file FILE|-`; write options | Register a formation rule. Edit. |
| `rule preview` | rule selector or `--script-file FILE|-`; `--word WORD` | Evaluate a transformation without applying it. |
| `formation apply` | rule selector, `--word WORD`, `--base WORD`; write options | Associate existing occurrences with a derivation. Edit. |
| `formation chain` | `--segment N --token N` | Show the selected token's base and transformation steps. |
| `formation pop` | `--segment N --token N`; write options | Remove one final formation step from all matching occurrences. Edit. |
| `tokenize preview` | `--line TEXT`, tokenizer options | Tokenize a single line without loading or changing a project. |
| `export json|typst` | `-o, --output FILE|-`, `--overwrite` | Export an artifact; output defaults to stdout. |
| `batch FILE|-` | write options | Apply a versioned JSON command list and commit once. Edit. |

Text options mean exactly one of `--text TEXT`, `--text-file FILE|-`, or
`--clear`. A text file's contents, including trailing newlines, are preserved;
the CLI does not silently trim translations or comments. `--clear` means the
empty string. For glosses this retains the vocabulary entry; for comments it
removes the comment. There is no vocabulary-delete command in version 1.

Paginated queries default to `--offset 0 --limit 50`; `--all` conflicts with
both pagination options. Similarity and vocabulary search default to
`--limit 20`. Limits must be positive, and negative indices/offsets are errors.
Empty list/search results succeed with an empty list. A missing object requested
by an exact get/show operation produces an error.

Only the three unambiguous existing segment sort modes are exposed initially.
`text` sorts the concatenated token text using the shared sorter.
`token-count` counts tokens. Segment filtering matches token surfaces and
translations, excluding glosses and comments.

## Indices and selectors

Segment, token, and rule indices are zero-based in arguments and JSON. Machine
fields are named `segment_index`, `token_index`, and `rule_index`; text tables
label their index columns. The GUI currently displays segment numbers starting
at one, so CLI help must call out this difference.

A rule selector is exactly one of:

```text
--rule DESCRIPTION
--rule-index N
```

`--rule` uses an exact, case-sensitive description match and requires exactly
one match. A duplicate description is an `ambiguous_rule` error with matching
indices; `--rule-index` lets the caller disambiguate the loaded project.
`rule add` uses `--description`, not an implied unique or persistent name.

The saved-project exporter sorts rules by type and description. Consequently
rule indices can change when a modified project is saved and reloaded.
`rule add` must not report its appended in-memory index as a persistent ID.
Its receipt reports the description and type; `rule list` reports indices in
the currently loaded snapshot. Segment/token indices also cease to identify the
same data after replacement imports. No `--if-revision` flag is exposed.

## Import and scripting semantics

Tokenizer selection is mutually exclusive:

```text
--tokenizer whitespace|character    # default: whitespace
--tokenizer-script FILE|-
```

Fresh import starts with an empty session. Its default name is the input
filename stem, or `Untitled` for stdin. An explicit `--name` overrides it.
Supplying `--project` to import requires `--replace-text`; supplying
`--replace-text` without a project is an error. Replacement import preserves
the current project name unless `--name` is specified.

Replacement import retains vocabulary, formation rules, and word-comment maps
but replaces all segments, including their translations and segment comments.
The stored rules are retained; they are not automatically applied to new tokens.
Each nonblank input line is tokenized separately. Lines producing no tokens are
omitted. There is no append/merge mode.

Formation types are `derivation`, `inflection`, and `nonmorphological`.
A formation script defines `fn transform(word)` and returns a string.
A tokenizer script defines `fn tokenize(line)` and returns only strings in an
array. Scripts are loaded as file contents; the adapter passes data as function
arguments without interpolating it into the script.

`rule preview --script-file` and `tokenize preview` can run without a project.
`rule preview` with a stored-rule selector requires one. `tokenize preview`
accepts exactly one line, rejecting embedded newline characters; it does not
claim to preview the complete document-import pipeline.

`validate` and loading may execute stored formation scripts when reconstructing
derived words. Validation does not test every unused rule against arbitrary
inputs. Rule creation checks compilation and the function signature; preview
tests runtime behavior for its supplied word.

## Formation and comment scope

`formation apply --word cats --base cat --rule Plural` requires `cats` to
already occur and requires the rule to transform `cat` to exactly `cats`.
It updates formation metadata on all occurrences of that surface word.
It does not append newly generated words. The base must be in the vocabulary
or resolve through an unambiguous existing derived chain.

`formation pop --segment 0 --token 1` identifies a surface/base/chain combination.
It removes the last step on every matching occurrence, recomputes their surface
text, and preserves preceding steps. It does not delete a stored rule.
The help and operation receipt must state this scope. Do not expose
`--single-occurrence` without a shared application operation supporting it.

`comment get/set --segment N` targets that segment. Adding `--token N` resolves
the editable target through `Session::token_comment`: the base word for a plain
token or the derived surface for a formed token. This is a shared word comment,
not an occurrence-only note. A get result distinguishes the editable comment
from any inherited comment displayed by token annotations.

Version 1 deliberately uses token coordinates for word-comment writes. The
current application command accepts arbitrary nonempty word keys, but comments
without a persistable vocabulary/derived entry can disappear on export.
Arbitrary word-target flags require shared target validation before exposure.

Headword lookup means the first token of a segment, not a morphological base.
Usage lookup returns each matching segment once. Both use exact surface text.

## Structured results and errors

`--json` produces exactly one UTF-8 JSON document followed by a newline on stdout.
Clients parse stdout; stderr may contain script diagnostics and is not a JSON
stream. Buffer the report until the operation has succeeded or failed.
JSON output never changes automatically according to whether stdout is a TTY.

```json
{
  "schema_version": 1,
  "ok": true,
  "data": {
    "changed": true,
    "saved": true,
    "output": "project.json"
  }
}
```

A dry-run receipt sets `saved` to false and includes `dry_run: true`. `changed`
records whether application commands changed state; an explicit output copy
can be saved even when it is false. Paginated list results use `items`, `offset`,
`limit`, and `total` fields; `total` is the number of matching items before
pagination. With `--all`, `offset` is zero and `limit` is null. Index-addressed
records carry explicit index fields.
Do not serialize the runtime Project or its caches as a public query schema;
saved-project JSON and CLI result JSON are different formats.

```json
{
  "schema_version": 1,
  "ok": false,
  "error": {
    "code": "invalid_index",
    "message": "Invalid segment index: 99",
    "details": { "kind": "segment", "index": 99 }
  }
}
```

| Exit code | Meaning |
| --- | --- |
| `0` | Success, including no-op edits and empty search results. |
| `1` | Unexpected internal failure. |
| `2` | CLI syntax, missing options, or conflicting option combinations. |
| `3` | Invalid project, invalid index/target, ambiguous selector, or rejected domain operation. |
| `4` | Script compilation or execution failure. |
| `5` | Input/output failure. |
| `6` | Persistence conflict: output exists without `--overwrite`, or source changed before an in-place commit. |

Expected errors get stable codes such as `invalid_project`, `invalid_index`,
`invalid_input`, `rule_not_found`, `ambiguous_rule`, `script_error`, `io_error`,
`output_exists`, and `input_changed`. Map typed errors; never infer a code by
parsing an English error message. With a recognized `--json` flag, normal
argument-validation errors use the same error envelope. Help/version are text.

## Batch operations

```text
tdector -p project.json batch edits.json --in-place --json
```

```json
{
  "schema_version": 1,
  "commands": [
    { "op": "set_gloss", "word": "cat", "meaning": "animal" },
    { "op": "set_translation", "segment_index": 0, "translation": "a cat" }
  ]
}
```

Define explicit versioned request DTOs for `set_gloss`, `set_translation`,
`set_comment`, `create_rule`, `apply_formation`, and `pop_formation`. Comments
use segment/token coordinates; formation requests use description/index
selectors. `create_rule` embeds script contents rather than a host file path.
Reject unknown operation names and fields.

The version-1 command fields are:

| `op` | Required fields | Optional fields |
| --- | --- | --- |
| `set_gloss` | `word`, `meaning` | none |
| `set_translation` | `segment_index`, `translation` | none |
| `set_comment` | `segment_index`, `comment` | `token_index` for a shared word comment |
| `create_rule` | `description`, `type`, `script` | none |
| `apply_formation` | `word`, `base`, exactly one of `rule` or `rule_index` | none |
| `pop_formation` | `segment_index`, `token_index` | none |

`type` uses the same formation types as `rule add --type`. Text values are
literal strings; use an empty string to clear text. Batch requests do not read
additional files. Successful batch reports include `commands`, an ordered list
of individual operation receipts, together with the overall save receipt.
Batch errors put `stage` and optional `command_index` on the `error` object.

Parse the entire request before execution. Apply commands sequentially to the
invocation's private session and persist only if every command and serialization
succeeds. On failure discard that session and leave the destination unchanged.
Errors identify a `stage`: `input`, `command`, `serialize`, or `commit`. Include
a zero-based `command_index` when a particular command fails decoding, selector
resolution, or execution; omit it for whole-document parsing, serialization,
and commit failures. Indices/selectors are resolved against the evolving session
at each step. There is no intermediate save.

This guarantees a single file commit for a CLI batch, not rollback of script
diagnostic output. A future long-lived MCP session needs an application-owned
transaction operation before it can promise the same in-memory atomicity.

## Examples

```text
tdector import source.txt --name Example --tokenizer whitespace -o project.json
tdector -p project.json info --json
tdector -p project.json segment list --filter cat --limit 10
tdector -p project.json segment show 0 --json
tdector -p project.json vocab set cat --text animal --in-place
tdector -p project.json segment translate 0 --text "a cat and a dog" --in-place
tdector -p project.json comment set --segment 0 --token 1 --text-file note.txt --in-place
tdector -p project.json rule add --description Plural --type inflection --script-file plural.rhai --in-place
tdector -p project.json rule preview --rule Plural --word cat
tdector -p project.json formation apply --rule Plural --word cats --base cat --dry-run --json
tdector -p project.json similar segments 0 --limit 10 --json
tdector -p project.json export typst -o project.typ
tdector -p project.json vocab set cat --text feline -o -
```

The last command produces raw saved-project JSON suitable for a pipeline.
Use `--in-place` when editing the input file; shell redirection back to that
file can truncate it before the CLI starts.

## Implementation boundaries and acceptance checks

Keep argument parsing, path/stdin handling, file commit logic, exit codes, and
text/JSON rendering in `tdector-cli`. Put reusable request/response DTOs, rule
selector resolution, and any new validation in a GUI-free application API module
so CLI and MCP can share them. MCP should call application operations directly.
Neither adapter should call or parse the other executable.

Keep session ownership on one thread, matching the existing Rhai cache model.
The initial CLI does not need an async runtime. Add neither a server mode nor
persistent current-project state in this change.

Before release, test process-level behavior for the following:

- All command/option conflicts, zero-based index validation, and every exit-code category.
- A headless dependency graph with no GUI crates.
- Import/edit/save/reload/export using the bundled project fixtures.
- File/stdin and file/stdout round trips, including Unicode, spaces, BOMs, and only one stdin consumer.
- JSON envelopes on success/failure and script output remaining off stdout.
- Existing-output refusal, atomic commit failure, observed input changes, and no-op in-place behavior.
- Batch failure after a valid earlier command leaving the destination unchanged.
- Rule selection after save-time sorting, including duplicate descriptions.
- Formation edits affecting the documented occurrences and comments surviving persistence.
- Token-similarity limits above 20 rejected rather than silently truncated.

Append/merge import, project renaming, rule editing/deletion, vocabulary deletion,
single-occurrence formation edits, unrestricted token similarity, and persistent
object IDs remain separate application features.
