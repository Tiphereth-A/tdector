#!/usr/bin/env python3
"""Portable GUI reproductions and smoke tests; Python 3.9+, standard library only."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
from types import SimpleNamespace


def check(condition, message):
    if not condition:
        raise AssertionError(message)


def sample_hash(project):
    return hashlib.sha256(project.read_bytes()).hexdigest()


def log(message):
    print(message, file=sys.stderr)


class Client:
    check = staticmethod(check)

    def __init__(self, executable):
        resolved = shutil.which(executable)
        if resolved is None:
            raise FileNotFoundError(f"Cannot find tdector executable: {executable}")
        self.executable = str(Path(resolved).resolve())
        self.session = None

    def process(self, *args, input_text=None):
        # Argument arrays preserve Unicode, empty text, quotes, and paths on every OS.
        options = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}
        process = subprocess.Popen(
            [self.executable, "--json", *(str(arg) for arg in args)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            **options,
        )
        streams = {}

        def read_stream(name, stream):
            streams[name] = stream.read()
            stream.close()

        # Daemon readers also detect accidental pipe inheritance by a detached host.
        readers = [threading.Thread(target=read_stream, args=(name, stream), daemon=True)
                   for name, stream in (("stdout", process.stdout), ("stderr", process.stderr))]
        for reader in readers:
            reader.start()
        try:
            if input_text is not None:
                process.stdin.write(input_text.encode("utf-8"))
            process.stdin.close()
            process.wait(timeout=45)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            raise RuntimeError("tdector client exceeded 45 seconds; its operation outcome may be unknown.")
        for reader in readers:
            reader.join(timeout=5)
        check(not any(reader.is_alive() for reader in readers),
              "tdector exited but a child retained its redirected output pipe.")
        return SimpleNamespace(returncode=process.returncode,
                               stdout=streams["stdout"].decode("utf-8"),
                               stderr=streams["stderr"].decode("utf-8"))

    def json(self, *args):
        result = self.process(*args)
        check(result.returncode == 0,
              f"tdector exited with {result.returncode}: {result.stdout} {result.stderr}")
        report = json.loads(result.stdout)
        check(report.get("schema_version") == 1 and report.get("ok"),
              f"Expected a successful version-1 report: {result.stdout}")
        return report["data"]

    def gui(self, *args):
        check(self.session, "No started GUI session.")
        return self.json("gui", "--session", self.session, *args)

    def error(self, args, exit_code, error_code):
        result = self.process(*args)
        check(result.returncode == exit_code,
              f"Expected exit {exit_code}, received {result.returncode}: {result.stdout}")
        report = json.loads(result.stdout)
        check(not report["ok"] and report["error"]["code"] == error_code,
              f"Expected {error_code}: {result.stdout}")

    def gui_error(self, args, exit_code, error_code):
        return self.error(("gui", "--session", self.session, *args), exit_code, error_code)

    def start(self, project, width, height, font=None):
        args = ["gui", "start", "--project", project, "--width", width, "--height", height]
        if font:
            args += ["--font", font]
        started = self.json(*args)
        self.session = started["session"]
        return started

    def quit(self):
        if self.session:
            self.gui("quit", "--discard")
            self.session = None

    def control(self, handle, name):
        controls = [item for item in self.gui("inspect", handle)["controls"] if item["name"] == name]
        check(len(controls) == 1, f"Expected one advertised control {name!r} on {handle}.")
        return controls[0]

    def set_control(self, handle, name, value):
        control = self.control(handle, name)
        check(not control["read_only"] and not control["disabled"], f"Control {name!r} must be writable.")
        self.gui("set", handle, control["id"], "--value", json.dumps(value, ensure_ascii=False))

    def assert_control(self, handle, name, expected):
        actual = self.control(handle, name)["value"]
        check(actual == expected, f"Control {name!r}: expected {expected!r}, received {actual!r}.")

    def geometry(self, handle, x, y, width, height):
        # Size first: the old larger default rectangle can otherwise clamp the move.
        self.gui("resize", handle, "--width", width, "--height", height)
        self.gui("move", handle, "--x", x, "--y", y)

    def viewport(self, main, width, height):
        size = self.gui("inspect", main)["geometry"]["size"]
        if any(abs(actual - requested) >= 2 for actual, requested in zip(size, (width, height))):
            log(f"Startup viewport was constrained to {size}; requesting the {width} by {height} reproduction canvas.")
            self.gui("resize", main, "--width", width, "--height", height)
            size = self.gui("inspect", main)["geometry"]["size"]
        check(all(abs(actual - requested) < 2 for actual, requested in zip(size, (width, height))),
              f"Reproduction requires a {width} by {height} logical client viewport; check display constraints.")

    def window(self, handle, kind, pinned=False):
        view = self.gui("inspect", handle)
        check(view["handle"] == handle and view["kind"] == kind, f"Window {handle} must remain {kind!r}.")
        check(view["pinned"] == pinned, f"Window {handle} has unexpected pin state.")
        check(view.get("geometry") and all(value > 0 for value in view["geometry"]["size"]),
              f"Window {handle} has invalid rendered geometry.")
        return view

    def open(self, kind, *args):
        return self.gui("open", kind, *args)["handle"]


def target(view, **expected):
    for key, value in expected.items():
        check(view["target"][key] == value, f"Window {view['handle']} target {key!r} must be {value!r}.")


def complete(client, main, project, original_hash, handles):
    check(not client.gui("status")["dirty"], "Presentation and draft changes must leave the live project clean.")
    check(sample_hash(project) == original_hash, f"Sample file changed: {project}")
    windows = client.gui("windows")["windows"]
    for handle in handles.values():
        check(sum(window["handle"] == handle for window in windows) == 1, f"Missing window {handle} in discovery.")
    return dict(session=client.session, main_handle=main, handles=handles,
                project=str(project), sample_sha256=original_hash)


def epigraph(client, repo, validate_only):
    project = repo / "sample" / "epigraph.json"
    font = repo / "sample" / "epigraph.otf"
    original_hash = sample_hash(project)
    try:
        seventh = client.json("--project", project, "segment", "show", 6)
        check(seventh["translation"] == "Next Of Nari", "Epigraph [7] must translate to Next Of Nari.")
        eighth = client.json("--project", project, "segment", "show", 7)
        gods = [token for token in eighth["tokens"] if token["token_index"] == 1]
        check(len(gods) == 1, "Epigraph [8], token 1 must exist.")
        god = gods[0]
        base = chr(0xF000A) + chr(0xF0015)
        plural = base + chr(0xF0001)
        check((god["base_gloss"], god["base_word"], god["text"]) == ("god", base, plural),
              "Epigraph [8], token 1 must be the bundled plural god.")
        check(god["is_derived"] and god["formation_descriptions"] == ["Plural Noun"],
              "Epigraph god must have exactly the Plural Noun step.")
        rule = client.json("--project", project, "rule", "show", "--rule", "Plural Noun")["rule_index"]
        chain = client.json("--project", project, "formation", "chain", "--segment", 7, "--token", 1)
        check((chain["word"], chain["base_word"], chain["steps"][0]["result"]) == (plural, base, plural),
              "Epigraph plural god chain changed.")
        check(font.is_file(), "Epigraph sentence font is missing.")
        if validate_only:
            return dict(fixture="epigraph", segment=7, token=1, rule=rule, validated=True)

        main = client.start(project, 1280, 960, font)["main_handle"]
        client.viewport(main, 1280, 960)
        settings = {"theme": "light", "sort": "index-asc", "page-size": 10, "page": 0}
        for name, value in settings.items():
            client.set_control(main, name, value)
        client.gui("wait")
        check(any(area["id"] == "main" for area in client.gui("inspect", main)["scroll_areas"]),
              "Main scroll area was not advertised.")
        client.gui("scroll", main, "--area", "main", "--x", 0, "--y", 100000)
        similar = client.open("similar-segments", "--segment", 6)
        client.geometry(similar, 400, 300, 450, 620)
        references = client.open("references", "--word", plural)
        client.geometry(references, 760, 0, 500, 650)
        formation = client.open("word-formation", "--word", plural, "--segment", 7, "--token", 1)
        client.set_control(formation, "base-word", base)
        client.set_control(formation, "rule", rule)
        client.geometry(formation, 690, 700, 270, 225)
        chain_window = client.open("formation-chain", "--segment", 7, "--token", 1)
        client.geometry(chain_window, 980, 760, 280, 160)
        handles = dict(similarity=similar, references=references, formation=formation, chain=chain_window)
        for handle in handles.values():
            client.gui("focus", handle)
        client.gui("wait")
        for name, value in settings.items():
            client.assert_control(main, name, value)
        for name, value in {"base-word": base, "rule": rule, "preview": plural}.items():
            client.assert_control(formation, name, value)
        similar_view = client.window(similar, "similar-segments")
        reference_view = client.window(references, "references")
        formation_view = client.window(formation, "word-formation")
        chain_view = client.window(chain_window, "formation-chain")
        target(similar_view, segment=6)
        target(reference_view, word=plural)
        target(formation_view, word=plural)
        target(chain_view, segment=7, token=1)
        check(client.gui("inspect", main)["displayed_segments"] == list(range(10)),
              "Epigraph must display the first ten segment records.")
        check(reference_view["geometry"]["position"][0] > similar_view["geometry"]["position"][0],
              "References must sit to the right of segment similarity.")
        check(formation_view["geometry"]["position"][1] > reference_view["geometry"]["position"][1],
              "Formation draft must sit below references.")
        check(chain_view["geometry"]["position"][0] > formation_view["geometry"]["position"][0],
              "Formation chain must sit to the right of the draft.")
        return complete(client, main, project, original_hash, handles)
    finally:
        check(sample_hash(project) == original_hash, f"Sample file changed: {project}")


def ginger(client, repo, validate_only):
    project = repo / "sample" / "ginger.json"
    original_hash = sample_hash(project)
    try:
        selected = client.json("--project", project, "segment", "show", 274)
        check(selected["text"] == "ehu k. ehut ji", "Ginger [275] must contain ehu k. ehut ji.")
        definition = client.json("--project", project, "segment", "show", 930)["tokens"][0]
        check((definition["text"], definition["base_word"]) == ("subteta", "subtet"),
              "Ginger [931], token 0 must define subteta derived from subtet.")
        chain = client.json("--project", project, "formation", "chain", "--segment", 187, "--token", 2)
        check((chain["word"], chain["base_word"]) == ("subtetaf", "subtet"),
              "Ginger [188], token 2 must be subtetaf derived from subtet.")
        check([step["result"] for step in chain["steps"]] == ["subteta", "subtetaf"],
              "Ginger chain must be subtet -> subteta -> subtetaf.")
        page = client.json("--project", project, "segment", "list", "--filter", "k.",
                           "--sort", "token-count", "--limit", 50)
        indices = [item["segment_index"] for item in page["items"]]
        check(274 in indices, "Ginger [275] must remain on the first filtered shortest-length page.")
        preceding = indices.index(274)
        if validate_only:
            return dict(fixture="ginger", segment=187, token=2, word="subtetaf", validated=True)

        main = client.start(project, 1400, 1000)["main_handle"]
        client.viewport(main, 1400, 1000)
        settings = {"theme": "dark", "filter": "k.", "sort": "length-asc", "page-size": 50, "page": 0}
        for name, value in settings.items():
            client.set_control(main, name, value)
        log(f"The current Ginger first page contains {preceding} records before [275]; scrolling to the reference target.")
        client.gui("scroll", main, "--area", "main", "--x", 0, "--y", preceding * 134)
        similar = client.open("similar-segments", "--segment", 274)
        client.geometry(similar, 560, 70, 420, 450)
        new_rule = client.open("new-formation-rule")
        draft = {"description": "-a", "type": "derivation",
                 "script": 'fn transform(word) { word + "a" }', "test-word": "subtet"}
        for name, value in draft.items():
            client.set_control(new_rule, name, value)
        client.geometry(new_rule, 450, 390, 600, 550)
        definition_window = client.open("definition", "--word", "subteta", "--pinned")
        client.geometry(definition_window, 880, 0, 500, 130)
        references = client.open("references", "--word", "subteta", "--pinned")
        client.geometry(references, 880, 120, 500, 620)
        tokens = client.open("similar-tokens", "--word", "subteta")
        client.geometry(tokens, 720, 710, 310, 240)
        chain_window = client.open("formation-chain", "--segment", 187, "--token", 2)
        client.geometry(chain_window, 1050, 710, 330, 240)
        handles = dict(similarity=similar, new_rule=new_rule, definition=definition_window,
                       references=references, tokens=tokens, chain=chain_window)
        for handle in handles.values():
            client.gui("focus", handle)
        client.gui("wait")
        for name, value in settings.items():
            client.assert_control(main, name, value)
        for name, value in {**draft, "preview": "subteta"}.items():
            client.assert_control(new_rule, name, value)
        similar_view = client.window(similar, "similar-segments")
        new_rule_view = client.window(new_rule, "new-formation-rule")
        definition_view = client.window(definition_window, "definition", True)
        reference_view = client.window(references, "references", True)
        token_view = client.window(tokens, "similar-tokens")
        chain_view = client.window(chain_window, "formation-chain")
        target(similar_view, segment=274)
        for view in (definition_view, reference_view, token_view):
            target(view, word="subteta")
        target(chain_view, segment=187, token=2)
        check(chain_view["results"]["word"] == "subtetaf" and
              chain_view["results"]["steps"][0]["result"] == "subteta", "Rendered chain must describe subtetaf.")
        check(client.gui("inspect", main)["displayed_segments"] == indices,
              "GUI and fixture queries must agree on filtered first-page ordering, including [275].")
        check(reference_view["geometry"]["position"][0] > similar_view["geometry"]["position"][0],
              "References must sit to the right of segment similarity.")
        check(reference_view["geometry"]["position"][1] > definition_view["geometry"]["position"][1],
              "References must sit below definition.")
        check(new_rule_view["geometry"]["position"][1] > similar_view["geometry"]["position"][1],
              "The new-rule draft must sit below similarity.")
        check(chain_view["geometry"]["position"][0] > token_view["geometry"]["position"][0],
              "Formation chain must sit to the right of token similarity.")
        return complete(client, main, project, original_hash, handles)
    finally:
        check(sample_hash(project) == original_hash, f"Sample file changed: {project}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="scenario", required=True)
    for name in ("epigraph", "ginger", "smoke"):
        command = commands.add_parser(name)
        command.add_argument("--tdector", default="tdector", help="Executable path or command on PATH.")
        if name != "smoke":
            command.add_argument("--validate-only", action="store_true", help="Check fixtures without opening a GUI.")
            command.add_argument("--close", action="store_true", help="Close the demo session after checking it.")
            command.add_argument("--capture", type=Path, help="Save a framebuffer PNG; the output must not exist.")
    args = parser.parse_args(argv)
    if getattr(args, "validate_only", False) and (args.close or args.capture):
        parser.error("--validate-only cannot be combined with --close or --capture")
    client = None
    try:
        client = Client(args.tdector)
        repo = Path(__file__).resolve().parent.parent
        if args.scenario == "smoke":
            from gui_smoke import run_smoke
            result = run_smoke(client, repo)
        else:
            result = {"epigraph": epigraph, "ginger": ginger}[args.scenario](client, repo, args.validate_only)
            if args.capture:
                capture = args.capture.resolve()
                client.gui("screenshot", "--output", capture)
                result["capture"] = str(capture)
            if args.close:
                client.quit()
                result["closed"] = True
            elif client.session:
                log("Reproduction checks passed. The GUI session remains open; no sample file was saved.")
                log(f"Attach: tdector gui repl --session {client.session}")
        # ASCII escaping keeps JSON valid even with a legacy Windows console code page.
        print(json.dumps(result, ensure_ascii=True))
        return 0
    except (AssertionError, OSError, RuntimeError, ValueError, KeyError) as error:
        log(f"GUI check failed: {error}")
        if client and client.session:
            try:
                client.quit()
            except Exception as cleanup_error:
                log(f"Could not close session {client.session}: {cleanup_error}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
