"""Probe decks: does the PowerPoint desktop app remember embedded families
across restarts (2026-10-24)?

`name_font_probe.py` showed that New Computer Modern and converted
Libertinus lose their bold face under their own family names but not
renamed, with identical name-table structure. The first deck ever opened
with "Libertinus Serif" (`web-font-probe.pptx`) embedded its regular face
only. Two decks, IBM Plex Sans under new names:

- persist-a.pptx: family "Probe P", regular face only.
- persist-b.pptx: "Probe P" and "Probe Q", regular/bold/italic/bold italic.

Open A, quit PowerPoint, open B. If P's bold is drawn regular in B and Q's
isn't, PowerPoint keeps the faces of the first deck that embedded a family.
Pure Python. Args: template.pptx, out_dir.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from style_font_probe import PLEX, rename, write  # noqa: E402
from web_font_probe import build, parse  # noqa: E402

FILES = {"regular": "Regular", "bold": "Bold", "italic": "Italic", "boldItalic": "BoldItalic"}
SUBFAMILY = {"regular": "Regular", "bold": "Bold", "italic": "Italic", "boldItalic": "Bold Italic"}


def family(name, styles):
    out = {}
    for style in styles:
        t = parse((PLEX / f"IBMPlexSans-{FILES[style]}.ttf").read_bytes())
        rename(t, name, SUBFAMILY[style])
        out[style] = build(t)
    return out


def main(template, out_dir):
    out = Path(out_dir)
    write(template, [("P", "Probe P", family("Probe P", ["regular"]), False, "deck A: regular face only")],
          out / "persist-a.pptx")
    write(template, [
        ("P", "Probe P", family("Probe P", FILES), False, "all four faces; deck A embedded regular only"),
        ("Q", "Probe Q", family("Probe Q", FILES), False, "all four faces, new name (control)"),
    ], out / "persist-b.pptx")


if __name__ == "__main__":
    main(*sys.argv[1:3])
