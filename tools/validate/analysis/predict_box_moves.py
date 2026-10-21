"""PowerPoint dy per line after moving boxes: measured dy + (new top - old top)."""
import importlib.util
import json
import statistics
import sys

import pypdfium2 as pdfium

spec = importlib.util.spec_from_file_location("tp", "tools/validate/text_positions.py")
tp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tp)

d, new_dir = sys.argv[1], sys.argv[2]
for name in sys.argv[3:]:
    old = json.load(open(f"{d}/decks/{name}.ir.json"))
    new = json.load(open(f"{new_dir}/{name}.ir.json"))
    W, H = old["width"] / 12700, old["height"] / 12700
    ref = pdfium.PdfDocument(f"{d}/decks/{name}.pdf")
    exp = pdfium.PdfDocument(f"{d}/{name}-windows.pdf")
    before, after = [], []
    for n, (so, sn) in enumerate(zip(old["slides"], new["slides"])):
        r, e = tp.Page(ref[n], W, H), tp.Page(exp[n], W, H)
        new_top = {i["name"]: i["frame"]["y"] for i in sn["items"] if i["kind"] == "text_box"}
        for item in so["items"]:
            if item["kind"] != "text_box" or item["name"] not in new_top:
                continue
            shift = (new_top[item["name"]] - item["frame"]["y"]) / 12700
            for p in item["paragraphs"]:
                for line in p["lines"]:
                    t = "".join(x["text"] for x in line).strip()[:16]
                    if len(t) < 3:
                        continue
                    a, b = r.find(t), e.find(t)
                    if not a or not b or abs(b[0] - a[0]) > 2:
                        continue
                    before.append(b[1] - a[1])
                    after.append(b[1] - a[1] + shift)
    ab = lambda v: sorted(abs(x) for x in v)
    for label, v in (("now", before), ("new", after)):
        s = ab(v)
        print(
            f"{name:20} {label}: {len(s)} lines  |dy| median {statistics.median(s):.2f}"
            f"  p90 {s[int(0.9 * len(s))]:.2f}  max {s[-1]:.2f}"
        )
