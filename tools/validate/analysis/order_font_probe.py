"""Probe deck for the order of embedded faces in the PowerPoint desktop app
(2026-10-24).

In typst-pptx decks the bold face's part came before the regular one (faces
are written in the order the deck first uses them) and PowerPoint drew bold
text in the regular face; in `style_font_probe.py`, with the regular face
first, bold worked. Each line is one family under a new name, runs regular,
bold, italic, bold italic; only the order of the face parts (and their
relationship ids) changes:

- O1: IBM Plex Sans, regular, bold, italic, bold italic.
- O2: IBM Plex Sans, bold, regular, italic, bold italic (typst-pptx order).
- O3: IBM Plex Sans, regular, italic, bold, bold italic.
- O4: converted Libertinus Serif, regular, bold, italic.
- O5: converted Libertinus Serif, bold, regular, italic.

Open it in a freshly started PowerPoint: fonts embedded by decks opened
earlier in the session stand in for families of the same name. Pure
Python. Args: template.pptx, libertinus.pptx (as for style_font_probe.py),
output.pptx.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from style_font_probe import PLEX, libertinus, rename, write  # noqa: E402
from web_font_probe import build, parse  # noqa: E402

SUBFAMILY = {"regular": "Regular", "bold": "Bold", "italic": "Italic", "boldItalic": "Bold Italic"}
PLEX_FILE = {"regular": "Regular", "bold": "Bold", "italic": "Italic", "boldItalic": "BoldItalic"}


def main(template, libertinus_deck, output):
    lib = libertinus(libertinus_deck)
    plex = {s: (PLEX / f"IBMPlexSans-{f}.ttf").read_bytes() for s, f in PLEX_FILE.items()}
    libs = {s: lib[f"Libertinus Serif {SUBFAMILY[s]}"] for s in ("regular", "bold", "italic")}
    plan = [
        ("O1", plex, ("regular", "bold", "italic", "boldItalic"), "Plex: regular, bold, italic, bold italic"),
        ("O2", plex, ("bold", "regular", "italic", "boldItalic"), "Plex: bold first (typst-pptx order)"),
        ("O3", plex, ("regular", "italic", "bold", "boldItalic"), "Plex: regular, italic, bold, bold italic"),
        ("O4", libs, ("regular", "bold", "italic"), "converted Libertinus: regular, bold, italic"),
        ("O5", libs, ("bold", "regular", "italic"), "converted Libertinus: bold first (typst-pptx order)"),
    ]
    fams = []
    for key, faces, order, label in plan:
        family = f"Probe {key}"
        built = {}
        for style in order:
            t = parse(faces[style])
            rename(t, family, SUBFAMILY[style])
            built[style] = build(t)
        fams.append((key, family, built, False, label))
    write(template, fams, output)


if __name__ == "__main__":
    main(*sys.argv[1:4])
