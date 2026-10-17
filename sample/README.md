## Regenerate screenshots

[generate_screenshots.py](../scripts/generate_screenshots.py) regenerates `epigraph.png` and `ginger.png` using the current GUI and the shared [reproduction scenarios](../scripts/gui_control.py). It requires Python 3.9 or later, no third-party Python packages, and a working graphical desktop.

From the repository root, build the GUI-enabled CLI:

```text
cargo build --locked --release -p tdector-cli --features gui-control --target-dir target/gui-control
```

On macOS or Linux:

```bash
python3 scripts/generate_screenshots.py --tdector ./target/gui-control/release/tdector
```

On Windows:

```powershell
py -3 scripts/generate_screenshots.py --tdector ./target/gui-control/release/tdector.exe
```

Use `python` instead of `py -3` if that is how Python is installed. If the GUI-enabled `tdector` is on `PATH`, omit `--tdector`. The generator locates sample files relative to its own location; a supplied executable path is relative to your current directory.

The generator verifies each scenario, captures its rendered framebuffer, and closes its GUI session. It overwrites the two PNG files only after both captures succeed; a failed capture leaves the existing images in place. The sample project files remain unchanged. The compositions use logical client sizes of 1280 × 960 for Epigraph and 1400 × 1000 for Ginger, so output pixel dimensions and font rendering vary with display DPI and platform. See the [GUI-control guide](../docs/gui-control.md#readme-reproduction) for display constraints and scenario details.

## Acknowledgments

[ginger.txt](ginger.txt) and it's translation come from:

- https://discord.com/channels/1201585304964976820/1293941074464735262/1355309770264936549
- https://discord.com/channels/1201585304964976820/1293941074464735262/1355308821056192802
- https://github.com/DesiQuintans/gingergame
- https://github.com/jcoglan/ginger/blob/main/translations.txt
- https://github.com/kai-tk/Ginger

[epigraph.txt](epigraph.txt) and it's translation come from:

- https://steamcommunity.com/sharedfiles/filedetails/?id=3428231195
- https://steamcommunity.com/sharedfiles/filedetails/?id=3171688772

[epigraph.otf](epigraph.otf) is made by myself using FontForge, you can access the source file in [NewNariSans.sfd](NewNariSans.sfd).
