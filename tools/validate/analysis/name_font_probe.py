"""Probe deck for embedded faces under their own names in the PowerPoint
desktop app (2026-10-24).

In the typst-pptx decks (`new-computer-modern.pptx`, the Libertinus demo)
PowerPoint drew bold runs in the regular face without synthetic bold, while
the same faces renamed in `order_font_probe.py` worked. Each line is one
family, runs regular, bold, italic, bold italic:

- N1: New Computer Modern R/B/I/BI as typst-pptx embeds it
  ("NewComputerModern10", with typographic family names 16/17).
- N2: the same faces renamed (names 1-6 rewritten, 16/17 dropped).
- N3: converted Libertinus Serif R/B/I as typst-pptx embeds it.
- N4: the same faces renamed.

Open it in a freshly started PowerPoint. Pure Python. Args: template.pptx,
ncm.pptx (a typst-pptx deck embedding New Computer Modern R/B/I/BI),
libertinus.pptx (as for style_font_probe.py), output.pptx.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from style_font_probe import libertinus, rename, write  # noqa: E402
from web_font_probe import build, parse  # noqa: E402

SUBFAMILY = {"regular": "Regular", "bold": "Bold", "italic": "Italic", "boldItalic": "Bold Italic"}


def main(template, ncm_deck, libertinus_deck, output):
    faces = libertinus(ncm_deck)
    ncm = {s: faces[f"NewComputerModern10-{f}"] for s, f in
           (("regular", "Regular"), ("bold", "Bold"), ("italic", "Italic"), ("boldItalic", "BoldItalic"))}
    faces = libertinus(libertinus_deck)
    lib = {s: faces[f"Libertinus Serif {SUBFAMILY[s]}"] for s in ("regular", "bold", "italic")}
    plan = [
        ("N1", "NewComputerModern10", ncm, False, "New Computer Modern as typst-pptx embeds it"),
        ("N2", "Probe N2", ncm, True, "New Computer Modern renamed"),
        ("N3", "Libertinus Serif", lib, False, "converted Libertinus as typst-pptx embeds it"),
        ("N4", "Probe N4", lib, True, "converted Libertinus renamed"),
    ]
    fams = []
    for key, family, data, renamed, label in plan:
        built = {}
        for style, ttf in data.items():
            t = parse(ttf)
            if renamed:
                rename(t, family, SUBFAMILY[style])
            built[style] = build(t)
        fams.append((key, family, built, False, label))
    write(template, fams, output)


if __name__ == "__main__":
    main(*sys.argv[1:5])
