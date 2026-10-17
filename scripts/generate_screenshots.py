#!/usr/bin/env python3
"""Regenerate epigraph.png and ginger.png with the current GUI (Python 3.9+)."""

import argparse
from pathlib import Path
import subprocess
import sys
import tempfile


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tdector", default="tdector",
        help="GUI-enabled executable path or command on PATH (default: tdector).",
    )
    args = parser.parse_args(argv)
    scripts_dir = Path(__file__).resolve().parent
    sample_dir = scripts_dir.parent / "sample"
    runner = scripts_dir / "gui_control.py"
    scenarios = ("epigraph", "ginger")

    try:
        # Capture both successfully before replacing either existing sample image.
        # Keep temporary images beside the targets so each replacement is atomic.
        with tempfile.TemporaryDirectory(prefix=".screenshots-", dir=sample_dir) as temporary:
            captures = Path(temporary)
            for scenario in scenarios:
                print(f"Capturing {scenario}...", file=sys.stderr, flush=True)
                subprocess.run(
                    [
                        sys.executable, str(runner), scenario,
                        "--tdector", args.tdector,
                        "--capture", str(captures / f"{scenario}.png"),
                        "--close",
                    ],
                    check=True,
                    stdout=subprocess.DEVNULL,
                )
            for scenario in scenarios:
                destination = sample_dir / f"{scenario}.png"
                (captures / destination.name).replace(destination)
                print(f"Updated {destination}")
    except subprocess.CalledProcessError as error:
        print(
            f"Screenshot generation failed (exit {error.returncode}); existing sample images were kept.",
            file=sys.stderr,
        )
        return 1
    except OSError as error:
        print(f"Screenshot generation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
