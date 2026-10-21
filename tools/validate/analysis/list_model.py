"""PowerPoint list model check: baselines from the IR (0.75 x spacing, then
spacing per line plus space before rounded to whole points) against the
measured PDF (old IR) and against Typst (new IR)."""
import importlib.util
import json
import statistics
import sys

import pypdfium2 as pdfium

spec = importlib.util.spec_from_file_location("tp", "tools/validate/text_positions.py")
tp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tp)


def baselines(item):
    """(text, model baseline) per line of a list box, in pt."""
    top = item["frame"]["y"] / 12700
    out, y, first = [], None, True
    for p in item["paragraphs"]:
        spc = p["line_spacing"] / 100
        before = round(p["list"]["space_before"] / 100)
        for k, line in enumerate(p["lines"]):
            if first:
                y = top + 0.75 * spc
                first = False
            else:
                y += spc + (before if k == 0 else 0)
            out.append(("".join(x["text"] for x in line).strip()[:16], y))
    return out


d, new_dir = sys.argv[1], sys.argv[2]
for name in sys.argv[3:]:
    old = json.load(open(f"{d}/decks/{name}.ir.json"))
    new = json.load(open(f"{new_dir}/{name}.ir.json"))
    W, H = old["width"] / 12700, old["height"] / 12700
    ref = pdfium.PdfDocument(f"{d}/decks/{name}.pdf")
    exp = pdfium.PdfDocument(f"{d}/{name}-windows.pdf")
    fit, before, after = [], [], []
    for n, (so, sn) in enumerate(zip(old["slides"], new["slides"])):
        lists = lambda s: [i for i in s["items"] if i["kind"] == "text_box" and any(p.get("list") for p in i["paragraphs"])]
        for io, inew in zip(lists(so), lists(sn)):
            r, e = tp.Page(ref[n], W, H), tp.Page(exp[n], W, H)
            for (t, y_old), (_, y_new) in zip(baselines(io), baselines(inew)):
                a, b = r.find(t), e.find(t)
                if not a or not b:
                    continue
                fit.append(b[1] - y_old)
                before.append(b[1] - a[1])
                after.append(y_new + (b[1] - y_old) - a[1])
    show = lambda v: f"median {statistics.median(map(abs, v)):.2f} max {max(map(abs, v)):.2f}"
    print(f"{name:18} {len(fit)} list lines  model vs PowerPoint: {show(fit)}")
    print(f"{'':18} PowerPoint vs Typst now: {show(before)}   predicted new: {show(after)}")
