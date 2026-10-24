"""Text-fit check (plan §11.4): where each exported text line lands compared
with Typst.

    uv run --project tools/validate python -I tools/validate/text_positions.py \\
        DECK.ir.json TYPST.pdf EXPORT.pdf [--max-dy PT] [--lines]
        [--model M --built-with M]

For every line of every text box in the IR, finds the line's text on the
same page of both PDFs and compares the baseline (dy) and start (dx) of its
first character, and the width up to its last character (dw), in pt
(positive = export lower / further right / wider). EXPORT.pdf
may be PowerPoint's (slide centred on the paper) or LibreOffice's. Matches
more than 2 pt apart horizontally are counted as unmatched (the same text
found elsewhere on the slide), not as errors. With --max-dy, exits 1 if a
matched line is further off vertically; with --max-dw, if a line's width
differs more.

With --model, also prints the residual after moving each line by the
difference between that baseline model and the one the IR was built with
(pptx-ir/src/text.rs), i.e. what dy would be if the exporter used it.
"""

import argparse
import ctypes
import json
import pathlib
import statistics
import sys

import pypdfium2 as pdfium
import pypdfium2.raw as raw

EMU_PER_PT = 12700


def first_baseline(model: str, size: float, spacing: float | None) -> float:
    """Distance from the box top to the first baseline, pt."""
    if model == "libreoffice":
        return (spacing if spacing is not None else 1.2 * size) - 0.2 * size
    if spacing is None:
        return 0.949 * size
    return 0.75 * spacing


class Page:
    def __init__(self, page, slide_w: float, slide_h: float):
        self.text = page.get_textpage()
        self.chars = self.text.count_chars()
        # Whitespace runs collapsed (PDFs differ in how they keep spaces),
        # with a map back to character indices.
        # Built per character index: get_text_range() would merge UTF-16
        # surrogate pairs (math italics) and shift every later index.
        self.string, self.index = "", []
        units = [raw.FPDFText_GetUnicode(self.text, i) for i in range(self.chars)]
        for i, unit in enumerate(units):
            if 0xDC00 <= unit < 0xE000:
                continue
            if 0xD800 <= unit < 0xDC00 and i + 1 < len(units):
                unit = 0x10000 + ((unit - 0xD800) << 10) + (units[i + 1] - 0xDC00)
            char = chr(unit) if unit else " "
            if char.isspace():
                if self.string.endswith(" ") or not self.string:
                    continue
                char = " "
            self.string += char
            self.index.append(i)
        width, height = page.get_width(), page.get_height()
        self.scale = width / slide_w
        self.height = height
        self.top = (height - slide_h * self.scale) / 2
        self.cursor = 0

    def find(self, needle: str, last: bool = False) -> tuple[float, float] | None:
        """Slide position of the first (or last) character of the next
        occurrence."""
        needle = " ".join(needle.split())
        index = self.string.find(needle, self.cursor)
        if index < 0:
            index = self.string.find(needle)
        if index < 0:
            return None
        self.cursor = index + 1
        index = self.index[index + len(needle) - 1 if last else index]
        x, y = ctypes.c_double(), ctypes.c_double()
        raw.FPDFText_GetCharOrigin(self.text, index, x, y)
        return x.value / self.scale, (self.height - y.value - self.top) / self.scale


def text_paragraphs(items: list) -> list:
    """Paragraphs of text boxes and of table cells (each cell once)."""
    out = []
    for item in items:
        if item["kind"] == "text_box":
            out.extend(item["paragraphs"])
        elif item["kind"] == "table":
            for row in item["cells"]:
                for cell in row:
                    if cell["merge"]["type"] == "origin":
                        out.extend(cell["paragraphs"])
        elif item["kind"] == "group":
            out.extend(text_paragraphs(item["items"]))
    return out


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("ir", type=pathlib.Path)
    parser.add_argument("reference", type=pathlib.Path)
    parser.add_argument("export", type=pathlib.Path)
    parser.add_argument("--model", choices=["powerpoint", "libreoffice"])
    parser.add_argument("--built-with", choices=["powerpoint", "libreoffice"], default="powerpoint")
    parser.add_argument("--max-dy", type=float)
    parser.add_argument("--max-dw", type=float)
    parser.add_argument("--lines", action="store_true", help="print every line")
    args = parser.parse_args()

    deck = json.loads(args.ir.read_text())
    slide_w, slide_h = deck["width"] / EMU_PER_PT, deck["height"] / EMU_PER_PT
    ref_pdf = pdfium.PdfDocument(str(args.reference))
    exp_pdf = pdfium.PdfDocument(str(args.export))
    # Browser "Save as PDF" (e.g. PowerPoint for the web) rounds the page to
    # whole 96 dpi pixels, which shifts every line by a few tenths of a pt.
    page_w, page_h = exp_pdf[0].get_width(), exp_pdf[0].get_height()
    pixels = all(abs(v / 0.75 - round(v / 0.75)) < 0.01 for v in (page_w, page_h))
    if pixels and (abs(page_w - slide_w) > 0.1 or abs(page_h - slide_h) > 0.1):
        print(
            f"warning: {args.export.name} is {page_w:.2f} x {page_h:.2f} pt, rounded to"
            f" pixels (slide {slide_w:.2f} x {slide_h:.2f}): not PowerPoint's own PDF"
            " export, positions are skewed"
        )

    all_dy, all_dx, all_dw, all_res = [], [], [], []
    unmatched = 0
    for number, slide in enumerate(deck["slides"], start=1):
        ref = Page(ref_pdf[number - 1], slide_w, slide_h)
        exp = Page(exp_pdf[number - 1], slide_w, slide_h)
        dys, dxs, dws, residuals = [], [], [], []
        # Text moved to the slide's layout is drawn there too.
        layout = slide.get("layout")
        items = (deck["layouts"][layout]["items"] if layout is not None else []) + slide["items"]
        for paragraph in text_paragraphs(items):
            if not paragraph["lines"]:
                continue
            if True:
                spacing = paragraph["line_spacing"]
                spacing = spacing / 100 if spacing is not None else None
                first_size = max(run["size"] for run in paragraph["lines"][0]) / 100
                for k, line in enumerate(paragraph["lines"]):
                    text = "".join(run["text"] for run in line).strip()
                    needle = text[:16]
                    if len(needle) < 3:
                        continue
                    a, b = ref.find(needle), exp.find(needle)
                    if a is None or b is None:
                        unmatched += 1
                        continue
                    dx, dy = b[0] - a[0], b[1] - a[1]
                    if abs(dx) > 2:
                        unmatched += 1
                        continue
                    dys.append(dy)
                    dxs.append(dx)
                    dw = 0.0
                    tail = text[-16:]
                    a_end, b_end = ref.find(tail, last=True), exp.find(tail, last=True)
                    if len(text) >= 8 and a_end and b_end and a_end[0] - a[0] > 20:
                        dw = (b_end[0] - b[0]) - (a_end[0] - a[0])
                        dws.append(dw)
                    line_report = f"  s{number:<3} {dy:+6.2f} {dx:+6.2f} {dw:+6.2f}  {needle}"
                    if args.model:
                        shift = first_baseline(args.model, first_size, spacing) - first_baseline(
                            args.built_with, first_size, spacing
                        )
                        residuals.append(dy - shift)
                        line_report += f"   residual {dy - shift:+6.2f}"
                    if args.lines:
                        print(line_report)
        if not dys:
            continue
        summary = (
            f"slide {number:>3}: {len(dys):3} lines  dy median {statistics.median(dys):+6.2f}"
            f" max {max(dys, key=abs):+6.2f}  dx max {max(dxs, key=abs):+6.2f}"
            f"  dw max {max(dws, key=abs, default=0.0):+6.2f}"
        )
        if residuals:
            summary += f"  | residual median {statistics.median(residuals):+6.2f} max {max(residuals, key=abs):+6.2f}"
        print(summary)
        all_dy += dys
        all_dx += dxs
        all_dw += dws
        all_res += residuals

    print(
        f"all: {len(all_dy)} lines ({unmatched} unmatched)  |dy| median"
        f" {statistics.median(map(abs, all_dy)):.2f} max {max(map(abs, all_dy)):.2f}"
        f"  |dx| max {max(map(abs, all_dx)):.2f}"
        f"  |dw| median {statistics.median(map(abs, all_dw or [0])):.2f}"
        f" max {max(map(abs, all_dw), default=0):.2f}"
    )
    if all_res:
        print(
            f"residual with {args.model} model: |dy| median {statistics.median(map(abs, all_res)):.2f}"
            f" max {max(map(abs, all_res)):.2f}"
        )


    failed = False
    if args.max_dy is not None and max(map(abs, all_dy)) > args.max_dy:
        print(f"FAIL: a line is more than {args.max_dy} pt off vertically")
        failed = True
    if args.max_dw is not None and max(map(abs, all_dw), default=0) > args.max_dw:
        print(f"FAIL: a line's width differs by more than {args.max_dw} pt")
        failed = True
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
