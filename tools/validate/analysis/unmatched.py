"""Lines text_positions.py can't match, and why (not found in the
reference, in the export, or more than 2 pt apart). Args: IR json, Typst
PDF, exported PDF."""
import json, sys, importlib.util
spec = importlib.util.spec_from_file_location("tp", "tools/validate/text_positions.py"); tp = importlib.util.module_from_spec(spec); spec.loader.exec_module(tp)
import pypdfium2 as pdfium
ir, ref, exp = sys.argv[1:4]
deck = json.load(open(ir)); W, H = deck["width"]/12700, deck["height"]/12700
rp, ep = pdfium.PdfDocument(ref), pdfium.PdfDocument(exp)
for n, slide in enumerate(deck["slides"]):
    r, e = tp.Page(rp[n], W, H), tp.Page(ep[n], W, H)
    for item in slide["items"]:
        if item["kind"] != "text_box": continue
        for p in item["paragraphs"]:
            for line in p["lines"]:
                t = "".join(x["text"] for x in line).strip()[:16]
                if len(t) < 3: continue
                a, b = r.find(t), e.find(t)
                why = "ref" if a is None else "exp" if b is None else ("dx" if abs(b[0]-a[0]) > 2 else None)
                if why: print(f"s{n+1} {why}: {t!r}" + "")
