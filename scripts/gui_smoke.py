"""Real native GUI smoke checks shared by the portable Python entry point."""

import hashlib
import json
from pathlib import Path
import sys
import tempfile


def check(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def translation_control(inspection):
    controls = [control["id"] for control in inspection["controls"]
                if control["id"].endswith(":segment:0:translation")]
    check(len(controls) == 1, "The first segment must advertise one translation control.")
    return controls[0]


def run_smoke(client, repo):
    """Exercise a real desktop, keeping all saves temporary and samples unchanged."""
    project = Path(repo) / "sample" / "epigraph.json"
    original_hash = digest(project)
    check(client.session is None, "Smoke requires a fresh client without an existing GUI session.")
    with tempfile.TemporaryDirectory(prefix="tdector-gui-smoke-") as directory:
        temporary = Path(directory)
        try:
            # A failed startup must report its error rather than expose an empty session.
            failure = client.process("gui", "start", "--project", temporary / "missing.json")
            check(failure.returncode != 0, "Starting a missing project must fail.")
            failure_report = json.loads(failure.stdout)
            check(failure_report.get("schema_version") == 1 and not failure_report.get("ok"),
                  "Failed startup must have a structured version-1 error envelope.")

            started = client.start(project, 800, 600)
            main = started["main_handle"]
            check(started["protocol_version"] == 1, "Startup must identify the protocol version.")
            initial = client.gui("status")
            check(initial["project_name"] == "epigraph" and not initial["dirty"],
                  "Startup must load the requested project before reporting readiness.")
            main_window = client.window(main, "main")
            check(all(abs(actual - requested) < 2
                      for actual, requested in zip(main_window["geometry"]["size"], (800, 600))),
                  "Startup dimensions must use logical client units, including on scaled displays.")

            # Change only the token; the locator remains syntactically valid.
            unauthorized = client.session[:-1] + ("1" if client.session.endswith("0") else "0")
            client.error(("gui", "--session", unauthorized, "status"), 7, "authentication_failed")

            # Both explicit exit and stdin EOF detach without terminating the host.
            for input_text in ("status\nexit\n", "status\n"):
                repl = client.process("gui", "--session", client.session, "repl", input_text=input_text)
                check(repl.returncode == 0, f"REPL detach failed: {repl.stderr}")
                reports = [json.loads(line) for line in repl.stdout.splitlines() if line.strip()]
                check(len(reports) == 1 and reports[0].get("ok")
                      and reports[0]["data"]["project_name"] == "epigraph",
                      "REPL status must return one machine-readable report.")
                client.gui("status")

            # The same draft retains its handle; cancel discards edits and invalidates it.
            draft = client.gui("open", "segment-comment", "--segment", 0)["handle"]
            unicode_text = "猫 🐈\nsecond line\r\n"
            client.set_control(draft, "comment", unicode_text)
            client.assert_control(draft, "comment", unicode_text)
            reused = client.gui("open", "segment-comment", "--segment", 0)
            check(reused["handle"] == draft and reused["reused"],
                  "Reopening the same target must preserve its draft and handle.")
            check(not client.gui("status")["dirty"], "An uncommitted comment draft must stay clean.")
            client.gui_error(("open", "segment-comment", "--segment", 1), 3, "already_open")
            client.gui("invoke", draft, "cancel")
            client.gui_error(("inspect", draft), 3, "stale_handle")

            # Pinning and resizing preserve identity and observed geometry across frames.
            word = client.json("--project", project, "segment", "show", 0)["tokens"][0]["text"]
            reference = client.gui("open", "references", "--word", word)["handle"]
            client.gui("invoke", reference, "pin")
            client.geometry(reference, 150, 180, 420, 300)
            client.gui("wait")
            first = client.window(reference, "references", pinned=True)
            client.gui("wait")
            second = client.window(reference, "references", pinned=True)
            check(all(abs(before - after) < 2 for before, after in
                      zip(first["geometry"]["size"], second["geometry"]["size"])),
                  "Requested popup size must persist across frames.")
            check(all(abs(actual - requested) < 50 for actual, requested in
                      zip(second["geometry"]["position"], (150, 180))),
                  "An explicitly moved popup must leave its initial position.")
            if not initial["capabilities"]["native_position"]:
                client.gui_error(("move", main, "--x", 10, "--y", 10), 3, "unsupported_operation")

            inspection = client.gui("inspect", main)
            translation = translation_control(inspection)
            read_only = next((control["id"] for control in inspection["controls"]
                              if control["id"].endswith(":gloss") and control["read_only"]), None)
            check(read_only is not None, "Epigraph's first page must expose a derived, read-only gloss.")
            client.gui_error(("set", main, read_only, "--value", "forbidden"), 3, "read_only_control")
            value_file = temporary / "unicode.txt"
            # Disable newline conversion explicitly, including on Windows and Python 3.9.
            with value_file.open("w", encoding="utf-8", newline="") as output:
                output.write(unicode_text)
            client.gui("set", main, translation, "--value-file", value_file)
            check(client.gui("get", main, translation)["value"] == unicode_text,
                  "UTF-8 multiline values must round-trip without trimming or newline conversion.")
            check(client.gui("status")["dirty"], "Immediate translation changes must dirty the project.")

            close = client.gui("close", main)
            check(not close["closed"], "Closing a dirty main window must require confirmation.")
            confirmation = client.gui("inspect", close["handle"])
            check(confirmation["kind"] == "confirmation",
                  "Dirty close must return the real confirmation dialog handle.")
            client.gui("invoke", close["handle"], "cancel")
            check(client.gui("status")["dirty"], "Cancelling close must preserve unsaved edits.")

            saved = temporary / "saved.json"
            receipt = client.gui("save", "--output", saved)
            check(receipt["saved"] and not receipt["dirty"] and receipt.get("save_token"),
                  "Save must acknowledge an exact snapshot and clear dirty state.")
            round_trip = client.json("--project", saved, "segment", "show", 0)
            check(round_trip["translation"] == unicode_text,
                  "Saved output must contain the exact edited translation.")
            client.gui("load", saved)
            reloaded = client.gui("status")
            check(reloaded["main_handle"] == main and reloaded["generation"] > initial["generation"]
                  and not reloaded["dirty"],
                  "Reload must preserve main identity and replace project generation.")
            client.gui_error(("inspect", reference), 3, "stale_handle")
            client.gui_error(("set", main, translation, "--value", "stale"), 3, "unknown_control")

            # A dirty quit must deliver its accepted confirmation response before shutdown.
            translation = translation_control(client.gui("inspect", main))
            client.gui("set", main, translation, "--value", "unsaved final change")
            quitting = client.gui("quit")
            check(not quitting["closed"], "Dirty quit must return a confirmation.")
            confirmed = client.gui("invoke", quitting["handle"], "confirm")
            check(confirmed["closed"], "Accepted shutdown must return a successful close receipt.")
            client.session = None
            check(digest(project) == original_hash, f"Sample file changed: {project}")
            return {"smoke": "passed", "project": str(project), "sample_sha256": original_hash,
                    "checks": ["startup-failure", "readiness", "logical-size", "authentication",
                               "repl-detach", "drafts", "lifecycle", "geometry", "read-only",
                               "unicode", "confirmation", "save-reload", "shutdown"]}
        finally:
            # Preserve the primary assertion if cleanup also fails, but still report the failure.
            unwinding = sys.exc_info()[0] is not None
            try:
                if client.session:
                    try:
                        client.quit()
                    except Exception as error:
                        if not unwinding:
                            raise
                        print(f"Could not close smoke session during cleanup: {error}", file=sys.stderr)
            finally:
                check(digest(project) == original_hash, f"Sample file changed: {project}")
