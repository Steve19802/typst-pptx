"""Extracts the TrueType data of every embedded font in a .pptx (EOT header
stripped) to out_dir and checks it: fontTools decodes every glyph, OTS (the
browsers' font sanitizer) must accept it. Args: pptx out_dir.

    uv run -q --with fonttools --with opentype-sanitizer python -I \
      tools/validate/analysis/embedded_fonts.py deck.pptx out/scratch/fonts
"""
import pathlib
import struct
import subprocess
import sys
import zipfile

from fontTools.ttLib import TTFont

pptx, out = sys.argv[1], pathlib.Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
z = zipfile.ZipFile(pptx)
for name in sorted(n for n in z.namelist() if n.endswith(".fntdata")):
    eot = z.read(name)
    size = struct.unpack_from("<I", eot, 4)[0]
    ttf = eot[-size:]
    path = out / (pathlib.Path(name).stem + ".ttf")
    path.write_bytes(ttf)
    font = TTFont(path)
    family = font["name"].getDebugName(4)
    tables = sorted(font.keys())
    glyf = font["glyf"] if "glyf" in font else None
    # Decompile every glyph (raises on malformed data).
    if glyf is not None:
        for g in font.getGlyphOrder():
            glyf[g].getCoordinates(glyf)
    ots = subprocess.run(
        [sys.executable, "-m", "ots", str(path), str(path) + ".ots"],
        capture_output=True,
        text=True,
    )
    print(
        f"{name}: {family!r} {len(ttf) // 1024} KiB glyf={'yes' if glyf else 'no'} "
        f"CFF={'CFF ' in font} maxp={font['maxp'].tableVersion:#x} "
        f"ots={'OK' if ots.returncode == 0 else 'FAIL ' + (ots.stderr or ots.stdout).strip()[:300]}"
    )
