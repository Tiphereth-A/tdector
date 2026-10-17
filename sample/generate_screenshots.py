#!/usr/bin/env python3
"""Rebuild epigraph.png and ginger.png with the GUI-enabled tdector (Python 3.9+)."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


SAMPLE = Path(__file__).resolve().parent


class Gui:
    def __init__(self, executable):
        self.executable = executable
        self.session = None

    def command(self, *args):
        result = subprocess.run(
            [self.executable, "--json", *map(str, args)],
            stdin=subprocess.DEVNULL, capture_output=True, text=True, encoding="utf-8",
            timeout=45,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
        )
        if result.returncode:
            raise RuntimeError(result.stdout.strip() or result.stderr.strip())
        return json.loads(result.stdout)["data"]

    def gui(self, *args):
        return self.command("gui", "--session", self.session, *args)

    def start(self, name, width, height, font=None):
        args = ["gui", "start", "--project", SAMPLE / f"{name}.json",
                "--width", width, "--height", height]
        if font:
            args += ["--font", SAMPLE / font]
        started = self.command(*args)
        self.session = started["session"]
        main = started["main_handle"]
        size = self.gui("inspect", main)["geometry"]["size"]
        if any(abs(actual - requested) >= 2 for actual, requested in zip(size, (width, height))):
            self.gui("resize", main, "--width", width, "--height", height)
            self.gui("wait")
            size = self.gui("inspect", main)["geometry"]["size"]
            if any(abs(actual - requested) >= 2 for actual, requested in zip(size, (width, height))):
                raise RuntimeError(f"Screenshot needs a {width} by {height} viewport; got {size}.")
        return main

    def set(self, handle, **values):
        controls = {item["name"]: item["id"] for item in self.gui("inspect", handle)["controls"]}
        for name, value in values.items():
            self.gui("set", handle, controls[name], "--value", json.dumps(value))

    def open(self, kind, bounds, *args):
        handle = self.gui("open", kind, *args)["handle"]
        x, y, width, height = bounds
        # Resize first so the old, larger rectangle does not constrain movement.
        self.gui("resize", handle, "--width", width, "--height", height)
        self.gui("move", handle, "--x", x, "--y", y)
        return handle


def epigraph(client):
    project = SAMPLE / "epigraph.json"
    segment = client.command("--project", project, "segment", "show", 7)
    god = next(token for token in segment["tokens"] if token["token_index"] == 1)
    plural = god["text"]
    rule = client.command("--project", project, "rule", "show", "--rule", "Plural Noun")["rule_index"]
    main = client.start("epigraph", 1280, 960, "epigraph.otf")
    client.set(main, **{"theme": "light", "sort": "index-asc", "page-size": 10, "page": 0})
    client.gui("wait")
    client.gui("scroll", main, "--area", "main", "--x", 0, "--y", 100000)
    similar = client.open("similar-segments", (400, 300, 450, 620), "--segment", 6)
    references = client.open("references", (760, 0, 500, 650), "--word", plural)
    formation = client.open("word-formation", (690, 700, 270, 225),
                            "--word", plural, "--segment", 7, "--token", 1)
    client.set(formation, **{"base-word": god["base_word"], "rule": rule})
    chain = client.open("formation-chain", (980, 760, 280, 160), "--segment", 7, "--token", 1)
    for handle in (similar, references, formation, chain):
        client.gui("focus", handle)


def ginger(client):
    page = client.command("--project", SAMPLE / "ginger.json", "segment", "list",
                          "--filter", "k.", "--sort", "token-count", "--limit", 50)
    preceding = [item["segment_index"] for item in page["items"]].index(274)
    main = client.start("ginger", 1400, 1000)
    client.set(main, **{"theme": "dark", "filter": "k.", "sort": "length-asc", "page-size": 50, "page": 0})
    client.gui("scroll", main, "--area", "main", "--x", 0, "--y", preceding * 134)
    similar = client.open("similar-segments", (560, 70, 420, 450), "--segment", 274)
    new_rule = client.open("new-formation-rule", (450, 390, 600, 550))
    client.set(new_rule, **{"description": "-a", "type": "derivation",
                           "script": 'fn transform(word) { word + "a" }', "test-word": "subtet"})
    definition = client.open("definition", (880, 0, 500, 130), "--word", "subteta", "--pinned")
    references = client.open("references", (880, 120, 500, 620), "--word", "subteta", "--pinned")
    tokens = client.open("similar-tokens", (720, 710, 310, 240), "--word", "subteta")
    chain = client.open("formation-chain", (1050, 710, 330, 240), "--segment", 187, "--token", 2)
    for handle in (similar, new_rule, definition, references, tokens, chain):
        client.gui("focus", handle)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tdector", default="tdector", help="GUI-enabled executable path or command on PATH.")
    args = parser.parse_args()
    client = Gui(args.tdector)
    try:
        # Capture both before replacing either image; a failed capture keeps the originals.
        with tempfile.TemporaryDirectory(prefix=".screenshots-", dir=SAMPLE) as temporary:
            captures = Path(temporary)
            for scene in (epigraph, ginger):
                print(f"Capturing {scene.__name__}...", file=sys.stderr, flush=True)
                try:
                    scene(client)
                    client.gui("wait")
                    client.gui("screenshot", "--output", captures / f"{scene.__name__}.png")
                finally:
                    if client.session:
                        client.gui("quit", "--discard")
                        client.session = None
            for name in ("epigraph", "ginger"):
                destination = SAMPLE / f"{name}.png"
                (captures / destination.name).replace(destination)
                print(f"Updated {destination}")
    except (OSError, RuntimeError, ValueError, KeyError, StopIteration, subprocess.SubprocessError) as error:
        print(f"Screenshot generation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
