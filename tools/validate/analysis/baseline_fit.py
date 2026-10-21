"""Phase 0 fit of the first-baseline models (single and exact spacing):
PowerPoint's first baseline below the box top per paragraph, against the
IR. Args: triples of IR json, Typst PDF, PowerPoint PDF."""
import json, sys, statistics, importlib.util
spec = importlib.util.spec_from_file_location("tp", "tools/validate/text_positions.py"); tp = importlib.util.module_from_spec(spec); spec.loader.exec_module(tp)
import pypdfium2 as pdfium
single, exact = [], []
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
                for k, line in enumerate(p["lines"]):
                    t = "".join(x["text"] for x in line).strip()[:16]
                    if len(t) < 3: continue
                    a, b = r.find(t), e.find(t)
                    if not a or not b or abs(b[0]-a[0]) > 2: continue
                    first = b[1] - top - (k * S if S else 0)
                    (single if S is None else exact).append((size, S, first))
print("single: n", len(single))
for size in sorted({s for s,_,_ in single}):
    v = [f/s0 for s0,_,f in single if s0 == size]
    print(f"  size {size:5.1f}: first/em median {statistics.median(v):.4f}  (n={len(v)}, pt diff vs 0.935: {statistics.median(v)*size-0.935*size:+.2f})")
print("exact: n", len(exact))
by = {}
for size, S, f in exact: by.setdefault((size, round(S,2)), []).append(f)
for (size, S), v in sorted(by.items()):
    m = statistics.median(v)
    print(f"  size {size:5.1f} S {S:6.2f} ({S/size:.3f} em): first {m:7.3f} pt = {m/S:.4f} S;  0.75S {0.75*S:7.3f}  S-0.2em {S-0.2*size:7.3f}  (n={len(v)})")
import numpy as np
s = np.array([x[0] for x in single]); f = np.array([x[2] for x in single])
for name, A in [("a*size", np.c_[s]), ("a*size + b", np.c_[s, np.ones_like(s)])]:
    coef, *_ = np.linalg.lstsq(A, f, rcond=None); r = f - A @ coef
    print(f"single {name:22}: {np.round(coef,4)}  residual |median| {np.median(np.abs(r)):.3f} max {np.abs(r).max():.3f} pt")
s = np.array([x[0] for x in exact]); S = np.array([x[1] for x in exact]); f = np.array([x[2] for x in exact])
for name, A in [("a*S", np.c_[S]), ("a*S + b*size", np.c_[S, s]), ("a*S + b*size + c", np.c_[S, s, np.ones_like(s)])]:
    coef, *_ = np.linalg.lstsq(A, f, rcond=None); r = f - A @ coef
    print(f"exact  {name:22}: {np.round(coef,4)}  residual |median| {np.median(np.abs(r)):.3f} max {np.abs(r).max():.3f} pt")
r = f - (S - 0.2 * s); print(f"exact  LibreOffice S-0.2em  : residual |median| {np.median(np.abs(r)):.3f} max {np.abs(r).max():.3f} pt")
print("robust (|residual| > 2 pt dropped):")
s = np.array([x[0] for x in single]); f = np.array([x[2] for x in single])
keep = np.abs(f - 0.9456 * s) < 2
coef, *_ = np.linalg.lstsq(np.c_[s[keep]], f[keep], rcond=None); r = f[keep] - s[keep] * coef[0]
print(f"single a*size: {coef[0]:.4f}  n={keep.sum()} dropped={(~keep).sum()}  residual |median| {np.median(np.abs(r)):.3f} max {np.abs(r).max():.3f} pt")
for size, S, ff in single:
    if abs(ff - 0.9456*size) >= 2: print("   dropped:", size, round(ff,2))
