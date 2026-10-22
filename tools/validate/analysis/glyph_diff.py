"""Outline check of fonts converted from CFF to TrueType: renders every
mapped character of the original and the converted font one at a time,
unhinted (FreeType FT_LOAD_NO_HINTING via freetype-py, 200 px), and reports
the share of ink pixels that differ by more than 25 %. Unhinted because
FreeType grid-fits CFF with its stem hints and leaves the (uninstructed)
TrueType copy alone, which alone differs by 6-9 %. Typst's originals are in
the cargo registry: typst-assets-<version>/files/fonts. Args: pairs of
original and converted font.

    uv run -q --with fonttools --with freetype-py --with numpy python -I \
      tools/validate/analysis/glyph_diff.py orig.otf converted.ttf
"""
import sys

import freetype
import numpy as np
from fontTools.ttLib import TTFont

SIZE = 200
FLAGS = freetype.FT_LOAD_RENDER | freetype.FT_LOAD_NO_HINTING


def render(face, c):
    face.load_char(chr(c), FLAGS)
    g = face.glyph
    bm = g.bitmap
    canvas = np.zeros((3 * SIZE, 3 * SIZE), np.uint8)
    if bm.width and bm.rows:
        a = np.array(bm.buffer, np.uint8).reshape(bm.rows, bm.pitch)[:, : bm.width]
        x, y = SIZE + g.bitmap_left, 2 * SIZE - g.bitmap_top
        canvas[y : y + bm.rows, x : x + bm.width] = a
    return canvas


pairs = sys.argv[1:]
for orig, conv in zip(pairs[::2], pairs[1::2]):
    chars = sorted(TTFont(orig).getBestCmap())
    faces = [freetype.Face(p) for p in (orig, conv)]
    for f in faces:
        f.set_pixel_sizes(0, SIZE)
    ink = off = 0
    worst = []
    for c in chars:
        a, b = (render(f, c) for f in faces)
        n = int((a > 128).sum())
        d = int((np.abs(a.astype(int) - b.astype(int)) > 64).sum())
        ink += n
        off += d
        if n:
            worst.append((d / n, chr(c)))
    worst.sort(reverse=True)
    print(
        f"{orig.split('/')[-1]:32} {len(chars)} chars  differing {100 * off / ink:.3f} % of ink  "
        f"worst: {', '.join(f'{ch!r} {100 * r:.1f}%' for r, ch in worst[:4])}"
    )
