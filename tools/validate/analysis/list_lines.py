"""Per list paragraph line: spcBef, lnSpc and PowerPoint's dy."""
import importlib.util
import json
import sys

import pypdfium2 as pdfium

spec = importlib.util.spec_from_file_location("tp", "tools/validate/text_positions.py")
tp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tp)

d = sys.argv[1]
for name in sys.argv[2:]:
    deck = json.load(open(f"{d}/decks/{name}.ir.json"))
    W, H = deck["width"] / 12700, deck["height"] / 12700
    ref = pdfium.PdfDocument(f"{d}/decks/{name}.pdf")
    exp = pdfium.PdfDocument(f"{d}/{name}-windows.pdf")
    for n, slide in enumerate(deck["slides"]):
        r, e = tp.Page(ref[n], W, H), tp.Page(exp[n], W, H)
        for item in slide["items"]:
            if item["kind"] != "text_box" or not any(p.get("list") for p in item["paragraphs"]):
                continue
            print(f"{name} s{n + 1} {item['name']}")
            prev = None
            for p in item["paragraphs"]:
                for k, line in enumerate(p["lines"]):
                    t = "".join(x["text"] for x in line).strip()[:16]
                    a, b = r.find(t), e.find(t)
                    if not a or not b:
                        continue
                    dy = b[1] - a[1]
                    step = "" if prev is None else f"{dy - prev:+.2f}"
                    before = p["list"]["space_before"] / 100 if k == 0 else 0
                    print(
                        f"   spc {p['line_spacing'] / 100:5.2f} before {before:5.2f}"
                        f"  dy {dy:+.2f}  step {step:>6}  {t}"
                    )
                    prev = dy
