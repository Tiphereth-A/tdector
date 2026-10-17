# Native GUI control

`gui-control` is an opt-in Cargo feature of `tdector-cli`. The default `tdector` executable stays headless. A GUI-enabled executable retains all ordinary CLI commands and adds `tdector gui`; it starts its own GUI host and does not need an installed `tdector-gui` executable.

```text
cargo build --locked --release -p tdector-cli --features gui-control --target-dir target/gui-control
```

The executable is `target/gui-control/release/tdector` (`tdector.exe` on Windows). Separate output directories prevent a GUI-enabled build from overwriting a headless build. Release assets preserve the existing `tdector-<target>` names and add `tdector-gui-control-<target>` with the platform executable suffix.

## Start and connect

On macOS or Linux, using a Bash-compatible shell with the GUI-enabled `tdector` on `PATH`:

```bash
started=$(tdector gui start --project sample/ginger.json --width 1400 --height 1000 --json)
export TDECTOR_GUI_SESSION=$(printf '%s' "$started" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["session"])')
main=$(printf '%s' "$started" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["main_handle"])')
tdector gui windows --json
tdector gui inspect "$main" --json
```

On Windows, using PowerShell:

```powershell
$started = tdector gui start --project sample/ginger.json --width 1400 --height 1000 --json |
    ConvertFrom-Json
if (-not $started.ok) { throw ($started.error | ConvertTo-Json -Compress) }
$env:TDECTOR_GUI_SESSION = $started.data.session
$main = $started.data.main_handle
tdector gui windows --json
tdector gui inspect $main --json
```

`start` returns only after loading the requested project and font and rendering the initial frame. A controlled session starts with fresh presentation state rather than restoring previous desktop layout. The host survives subsequent commands and REPL disconnection. Only CLI-launched sessions expose this control API; an ordinary desktop or browser session does not listen for requests.

Pass `--session SESSION` to address a specific session; `TDECTOR_GUI_SESSION` is the alternative. Treat the session locator as a capability: it contains the authentication material for that host. Session connection failures, authentication failures, protocol mismatches, and timeouts use exit code `7`. Default startup and command timeouts are 30 seconds; `--timeout SECONDS` accepts a positive number. Both options can appear anywhere after `gui`. Do not automatically retry an edit after a timeout: execution may have begun even if its response was lost.

Commands return the standard CLI report envelope with `--json`:

```json
{
  "schema_version": 1,
  "ok": true,
  "data": { "handle": "w-opaque-id", "kind": "word-comment", "reused": false }
}
```

Default output remains human-readable text. GUI commands operate on the host's current project; they do not use the ordinary headless CLI's per-invocation project session or write-mode flags.

## Commands

Use `tdector gui COMMAND --help` for argument details.

| Command | Effect |
| --- | --- |
| `start [--project FILE] [--font FILE] [--width W --height H]` | Launch a host and return its session and main-window handle. |
| `repl` | Attach an interactive prompt. |
| `status` | Inspect project name, dirty state, generation, and capabilities. |
| `windows` | Discover open windows, dialogs, and context menus. |
| `open KIND [--word WORD] [--segment N] [--token N] [--pinned]` | Open a view or return its existing matching handle. |
| `inspect HANDLE` | Read target, geometry, controls, current values, and actions. |
| `move HANDLE --x X --y Y` | Move the main window or an internal view. |
| `resize HANDLE --width W --height H` | Resize a resizable window; inspect the resulting constraints. |
| `focus HANDLE [--control CONTROL]` | Raise a window and optionally focus a control. |
| `scroll HANDLE --area AREA --x X --y Y` | Request a named scroll area's offset. |
| `get HANDLE CONTROL` | Read the current immediate value or draft. |
| `set HANDLE CONTROL --value VALUE` | Set a typed control; text also accepts `--value-file FILE|-` or `--clear`. |
| `invoke HANDLE ACTION` | Run an advertised action. |
| `close HANDLE` | Close a popup or request shutdown of the main window. |
| `load FILE` | Replace the live project from an explicit path. |
| `import FILE` | Read text and open Import Options. |
| `font FILE` | Load a sentence font and repaint. |
| `save [--output FILE] [--overwrite]` | Explicitly save the live project. |
| `export typst --output FILE [--overwrite]` | Export the current project. |
| `wait` | Wait for earlier changes to reach a rendered frame. |
| `screenshot --output FILE [--overwrite]` | Save the rendered GUI framebuffer as PNG for visual checks. |
| `quit [--discard]` | Request shutdown; `--discard` explicitly abandons unsaved edits. |

The directly openable kinds are `definition`, `references`, `similar-segments`, `similar-tokens`, `word-menu`, `segment-menu`, `word-formation`, `formation-chain`, `remove-formation-rule`, `new-formation-rule`, `word-comment`, and `segment-comment`. Import Options, Custom Tokenization, Error, and Confirmation dialogs arise from their corresponding actions and can be found with `windows`.

Segment and token arguments are zero-based. Displayed segment `[7]` is `--segment 6`; displayed segment `[275]` is `--segment 274`. A word's surface form and its base form can differ. Query `segment show` or inspect the selected token before reusing a coordinate from a fixture.

For `word-formation`, supply both `--segment` and `--token` to initialize a new draft from that token's base word and formation rule. The supplied `--word` must match the token. Mouse-opened formation editors use the clicked token's coordinates; a word-only request uses the first matching occurrence.

## Handles, controls, and edits

Handles are opaque application identifiers. Moving, resizing, focusing, or pinning preserves a handle. Closing invalidates it permanently. Reopening gets a new handle. The main handle survives project replacement; project-specific views and control identifiers do not. Controls that address project data include the project generation, so stale identifiers fail instead of silently editing replacement data. Discover current controls using `inspect` after loading or importing.

Ordinary dialogs are singletons. Opening the same kind and target returns the existing handle without resetting the draft. A different target in an occupied singleton returns `already_open` and the existing handle. Close that view or pin it where supported before opening another target. Pinned definition, reference, and segment-similarity views can coexist. Mouse-opened views participate in the same discovery and lifecycle.

Word-comment identity includes the full comment target: base-word and derived-word comments remain separate even when their spelling is identical. Inspection exposes this as `target.comment_target` with `kind` (`base_word` or `formatted_word`) and `word`. Transient word and segment context menus retarget on mouse right-click; changing the target invalidates the previous menu handle. Direct control `open` requests retain the singleton behavior above.

`inspect` supplies control types, enum choices, read-only/disabled state, and commit behavior. Use those returned identifiers and choices rather than treating displayed labels or egui widget IDs as an API. Project fields use generation-qualified identifiers, for example `gN:segment:7:translation`; obtain `N` and the full identifier from the current inspection. The main `page` control is zero-based; the `jump` draft uses one-based page numbers and takes effect when the `jump` action is invoked.

| Surface | Behavior |
| --- | --- |
| Main filter, all 12 sort modes, page size/current page, jump navigation, theme | Presentation changes apply immediately and do not dirty the project. |
| Segment translation and editable base-token gloss | Project changes apply immediately and stay unsaved. Derived glosses are read-only. |
| Definition, reference, and similarity results | Displayed results and annotations are read-only; supported views expose pinning and scrolling. |
| Word/segment comment | Edits change a draft; Save commits and Cancel/close discards it. |
| Apply formation rule | Base word, search, and selected rule update the preview; Apply commits. |
| New formation rule | Description, type, script, and test word update a draft and preview; Create commits. |
| Formation chain/removal | Chain inspection is read-only; removal requires the advertised confirmation action. |
| Custom tokenizer | Script and test text are drafts; Test previews and Apply imports. |
| Import Options | Select word-based, character-based, or custom-script tokenization, or Cancel. |
| Context menus, Error, Confirmation | Invoke the same available operations as the visible UI. |

Unavailable actions return `action_disabled`; writes to derived or otherwise read-only values return `read_only_control`. Closing a dirty main window returns `closed: false` with a confirmation handle. Invoke the advertised confirmation/cancellation action on that handle; it is associated with the pending operation. A successful shutdown replies before terminating the host.

Direct file paths remove the need to automate OS file pickers. Controlled project saves use native atomic replacement and conflict checks and acknowledge the exact saved snapshot only after success. As with headless CLI and MCP saves, use one writer per project; these checks are not interprocess locking. Project edits never autosave. GUI handles, layout, and session locators are not stored in project JSON, and the saved-project format is unchanged.

## Geometry and rendering

| Item | Units and origin |
| --- | --- |
| Main position | Outer-window top-left in desktop logical coordinates. |
| Main size | Client-area width and height. |
| Internal position | Top-left in egui points relative to the main viewport. |
| Internal size | Content width and height; inspect also returns observed outer bounds. |

Requested and observed geometry can differ because of display limits and content minimum sizes. Open/move/resize changes may need a rendered frame; call `wait`, then `inspect` to compare the actual result. Value changes can be acknowledged before their frame is drawn. Focus and scrolling are one-time requests, allowing normal mouse interaction afterward. Opening or focusing restores a minimized main window.

Capabilities are reported by `status`. Native outer-window movement is unsupported on Wayland and returns `unsupported_operation`; internal views remain movable. Windows, macOS, and Linux/X11 use the native backend's supported positioning. Window decorations, font rasterization, DPI, and minimum sizes vary by platform; do not compare exact screenshot pixels across those environments.

## Interactive prompt and quoting

```text
tdector gui repl --session SESSION
```

Inside the prompt:

```text
windows
open segment-comment --segment 0
set w-opaque-id comment --value "Check this translation"
get w-opaque-id comment
invoke w-opaque-id save
font 'sample/epigraph.otf'
exit
```

The prompt uses the same Clap commands with `shlex` tokenization on every platform. Single and double quotes group arguments. Use absolute POSIX paths such as `'/home/me/Fonts/epigraph.otf'` or single-quoted Windows paths such as `'C:\Fonts\epigraph.otf'`; the latter preserve backslashes. There is no shell expansion, environment-variable substitution, pipeline, or command execution. Empty quoted text is an empty value. Use a UTF-8 file for multiline text, such as `set HANDLE CONTROL --value-file 'notes/comment.txt'`. Relative paths resolve from the CLI's current working directory.

The REPL reserves stdin for commands and rejects `--value-file -`. Ordinary shell commands can read that value from stdin. `exit` or EOF detaches; `quit` requests GUI shutdown. A session can be reattached from a later prompt or controlled by separate shell commands.

`set --value` parses valid JSON values and otherwise treats its argument as text. For a text field that should contain a number or another JSON literal, encode it as a JSON string (for example `--value '"123"'`) or use `--value-file` to preserve literal text. The screenshot generator encodes values with Python's `json.dumps` and passes argument arrays without a shell.

## README reproduction

[sample/generate_screenshots.py](../sample/generate_screenshots.py) rebuilds both README screenshots on Windows, macOS, and Linux. It requires Python 3.9 or later and uses only the standard library. The GUI-enabled executable itself does not require Python.

On macOS or Linux:

```bash
python3 sample/generate_screenshots.py --tdector ./target/gui-control/release/tdector
```

On Windows with the Python launcher (use `python` if preferred):

```powershell
py -3 sample/generate_screenshots.py --tdector ./target/gui-control/release/tdector.exe
```

The only option is `--tdector`, which defaults to `tdector` on `PATH`. A supplied executable path resolves from the current directory. Sample projects, the font, and output PNGs are located relative to the script, so the current directory need not be the repository root. The generator performs no project save, export, or draft commit action.

The Epigraph composition uses a 1280 by 960 logical client area; Ginger uses 1400 by 1000. At high DPI, these can exceed the physical display. If the window manager constrains the startup size, the generator requests the full composition size with `resize` and checks the observed dimensions. It stops if the platform still refuses that size. Renderer screenshots include the full client canvas even when a window extends beyond the desktop.

Epigraph loads `sample/epigraph.otf`, sets light theme/index ascending/page size 10, scrolls to the latter half of page one, and opens similarity for index 6, references for the plural god at segment 7/token 1, its `Plural Noun` formation draft, and its chain.

Ginger sets dark theme/filter `k.`/shortest length/page size 50, scrolls to displayed segment `[275]`, and opens similarity for index 274, pinned definition and references for `subteta`, token similarity, and the `subtetaf` chain at segment 187/token 2. The new rule has description `-a`, derivation type, an append-`a` script, test word `subtet`, and preview `subteta`. That draft remains open for the capture without invoking Create.

The generator captures the app's rendered framebuffer, excluding native window decorations and other applications, and closes each session afterward. It replaces `sample/epigraph.png` and `sample/ginger.png` only after both captures succeed. A failed capture leaves the existing images in place. See the [sample instructions](../sample/README.md#regenerate-screenshots) for the build command.

For a separate session you control yourself, use `tdector gui screenshot --session SESSION --output capture.png`. An existing output requires `--overwrite`.
