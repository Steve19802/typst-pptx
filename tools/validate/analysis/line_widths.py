"""Line widths (first to last character) of every text-box line, export vs
Typst: how far an application draws a line wider or narrower (basis of the
width check). Args: triples of IR json, Typst PDF, exported PDF."""
import json, sys, ctypes, statistics, importlib.util
spec = importlib.util.spec_from_file_location("tp", "tools/validate/text_positions.py"); tp = importlib.util.module_from_spec(spec); spec.loader.exec_module(tp)
import pypdfium2 as pdfium, pypdfium2.raw as raw

def last_x(page, text):
    """x of the last character of the next occurrence of text."""
    needle = " ".join(text.split())
    i = page.string.find(needle, page.cursor)
    if i < 0: i = page.string.find(needle)
    if i < 0: return None
    page.cursor = i + 1
    j = page.index[i + len(needle) - 1]
    x, y = ctypes.c_double(), ctypes.c_double()
    raw.FPDFText_GetCharOrigin(page.text, j, x, y)
    return x.value / page.scale

rows = []
for ir, ref, exp in zip(*[iter(sys.argv[1:])]*3):
    deck = json.load(open(ir)); W, H = deck["width"]/12700, deck["height"]/12700
    rp, ep = pdfium.PdfDocument(ref), pdfium.PdfDocument(exp)
    for n, slide in enumerate(deck["slides"]):
        r, e = tp.Page(rp[n], W, H), tp.Page(ep[n], W, H)
        for item in slide["items"]:
            if item["kind"] != "text_box": continue
            for p in item["paragraphs"]:
                for line in p["lines"]:
                    t = "".join(x["text"] for x in line).strip()
                    if len(t) < 8: continue
                    a, b = tp.Page.find(r, t[:16]), tp.Page.find(e, t[:16])
                    if not a or not b or abs(b[0]-a[0]) > 2: continue
                    ae, be = last_x(r, t[-16:]), last_x(e, t[-16:])
                    if ae is None or be is None: continue
                    width = ae - a[0]
                    if width < 20: continue
                    drift = (be - b[0]) - width
                    rows.append((drift, width, len(t), ir.split("/")[-1][:18], n+1, t[:40]))
d = [x[0] for x in rows]
print(f"{len(rows)} lines: end drift |median| {statistics.median(map(abs,d)):.2f} pt, p90 {sorted(map(abs,d))[int(len(d)*0.9)]:.2f}, max {max(map(abs,d)):.2f}; relative max {max(abs(x[0])/x[1] for x in rows)*100:.2f}%")
for x in sorted(rows, key=lambda x: -abs(x[0]))[:8]:
    print(f"  {x[0]:+6.2f} pt over {x[1]:6.1f} pt ({x[0]/x[1]*100:+.2f}%)  {x[3]} s{x[4]}  {x[5]!r}")
