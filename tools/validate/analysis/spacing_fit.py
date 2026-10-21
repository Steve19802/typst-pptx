"""Per paragraph: exact line spacing vs PowerPoint's measured line pitch."""
import importlib.util
import json
import sys

import pypdfium2 as pdfium

spec = importlib.util.spec_from_file_location("tp", "tools/validate/text_positions.py")
tp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tp)

rows = []
for d, name in zip(sys.argv[1::2], sys.argv[2::2]):
    deck = json.load(open(f"{d}/decks/{name}.ir.json"))
    W, H = deck["width"] / 12700, deck["height"] / 12700
    ref = pdfium.PdfDocument(f"{d}/decks/{name}.pdf")
    exp = pdfium.PdfDocument(f"{d}/{name}-windows.pdf")
    for n, slide in enumerate(deck["slides"]):
        r, e = tp.Page(ref[n], W, H), tp.Page(exp[n], W, H)
        for item in slide["items"]:
            if item["kind"] != "text_box":
                continue
            # Lines across paragraphs of one box share the pitch.
            lines = [l for p in item["paragraphs"] for l in p["lines"]]
            spacing = item["paragraphs"][0]["line_spacing"]
            if spacing is None or len(lines) < 2:
                continue
            top = item["frame"]["y"] / 12700
            ys = []
            for line in lines:
                t = "".join(x["text"] for x in line).strip()[:16]
                a, b = r.find(t), e.find(t)
                if a and b:
                    ys.append((a[1], b[1]))
            if len(ys) < 2:
                continue
            k = len(ys) - 1
            typst = (ys[-1][0] - ys[0][0]) / k
            ppt = (ys[-1][1] - ys[0][1]) / k
            size = max(x["size"] for x in lines[0]) / 100
            sizes = {x["size"] for l in lines for x in l if x["text"].strip()}
            if len(sizes) > 1:
                continue
            first = ys[0][1] - top
            rows.append((spacing / 100, typst, ppt, size, name[:12], n + 1, first))

for s, t, p, size, name, n, first in sorted(rows):
    print(
        f"spc {s:6.2f} size {size:5.1f}  first {first:7.3f}  /S {first / s:.4f}"
        f"  /round(S) {first / round(s):.4f}  0.75S {first - 0.75 * s:+.3f}"
        f"  0.75rS {first - 0.75 * round(s):+.3f}  {name} s{n}"
    )
