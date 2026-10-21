"""Phase 0 fit: first baseline and per-line pitch error by font size and
exact spacing (numpy). Args: triples of IR json, Typst PDF, PowerPoint
PDF."""
import json, sys, importlib.util, numpy as np
spec = importlib.util.spec_from_file_location("tp", "tools/validate/text_positions.py"); tp = importlib.util.module_from_spec(spec); spec.loader.exec_module(tp)
import pypdfium2 as pdfium
rows = []  # size, S, k, first, pitch_err
for ir, ref, exp in zip(*[iter(sys.argv[1:])]*3):
    deck = json.load(open(ir)); W, H = deck["width"]/12700, deck["height"]/12700
    rp, ep = pdfium.PdfDocument(ref), pdfium.PdfDocument(exp)
    for n, slide in enumerate(deck["slides"]):
        r, e = tp.Page(rp[n], W, H), tp.Page(ep[n], W, H)
        for item in slide["items"]:
            if item["kind"] != "text_box": continue
            top = item["frame"]["y"]/12700
            for p in item["paragraphs"]:
                S = p["line_spacing"]/100 if p["line_spacing"] else None
                size = max(x["size"] for x in p["lines"][0])/100
                ys = []
                for k, line in enumerate(p["lines"]):
                    t = "".join(x["text"] for x in line).strip()[:16]
                    a, b = (r.find(t), e.find(t)) if len(t) >= 3 else (None, None)
                    ys.append(b[1] if a and b and abs(b[0]-a[0]) <= 2 and abs(b[1]-a[1]) < 5 else None)
                if S is None or ys[0] is None: continue
                pitches = [ys[k+1]-ys[k] for k in range(len(ys)-1) if ys[k] is not None and ys[k+1] is not None]
                rows.append((size, S, ys[0]-top, np.mean(pitches) - S if pitches else np.nan, len(p["lines"])))
rows.sort(key=lambda x: x[1]/x[0])
print(" size      S   S/em  first/S   first-0.744S  pitch-S  lines")
seen=set()
for size, S, f, pe, n in rows:
    key=(round(size,1), round(S,1))
    if key in seen: continue
    seen.add(key)
    print(f"{size:5.1f} {S:6.2f} {S/size:6.3f}  {f/S:7.4f}  {f-0.744*S:+7.2f}  {pe:+6.2f}  {n}")
print()
f = np.array([x[2] for x in rows]); S = np.array([x[1] for x in rows]); size = np.array([x[0] for x in rows])
print(f"{len(rows)} exact-spacing paragraphs")
for a in (0.744, 0.748, 0.750, 0.752, 0.755):
    r = f - a * S
    print(f"  a={a}: |residual| median {np.median(np.abs(r)):.2f} p90 {np.percentile(np.abs(r),90):.2f} max {np.abs(r).max():.2f} pt")
coef, *_ = np.linalg.lstsq(np.c_[S], f, rcond=None); print("  lstsq a", coef)
med = np.median(f / S); print("  median ratio", round(med, 4))
