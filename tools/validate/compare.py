"""Visual diff of a Typst reference PDF and LibreOffice's rendering of the
exported pptx (plan §11.3).

    uv run --project tools/validate python -I tools/validate/compare.py \
        REFERENCE.pdf EXPORT.pdf OUT_DIR [--dpi 72]

Writes OUT_DIR/page-N.png (reference | export | difference) and prints SSIM
per page. Exit status 0; thresholds are applied by the caller once tuned.
"""

import argparse
import pathlib

import numpy as np
import pypdfium2 as pdfium
from PIL import Image, ImageChops
from skimage.metrics import structural_similarity


def render(path: pathlib.Path, dpi: int) -> list[Image.Image]:
    pdf = pdfium.PdfDocument(str(path))
    return [page.render(scale=dpi / 72).to_pil().convert("RGB") for page in pdf]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("reference", type=pathlib.Path)
    parser.add_argument("export", type=pathlib.Path)
    parser.add_argument("out", type=pathlib.Path)
    parser.add_argument("--dpi", type=int, default=72)
    args = parser.parse_args()

    reference = render(args.reference, args.dpi)
    export = render(args.export, args.dpi)
    if len(reference) != len(export):
        print(f"page count differs: reference {len(reference)}, export {len(export)}")

    args.out.mkdir(parents=True, exist_ok=True)
    for number, (ref, exp) in enumerate(zip(reference, export), start=1):
        exp = exp.resize(ref.size)
        diff = ImageChops.invert(ImageChops.difference(ref, exp))
        sheet = Image.new("RGB", (ref.width * 3, ref.height), "white")
        for column, image in enumerate((ref, exp, diff)):
            sheet.paste(image, (column * ref.width, 0))
        sheet.save(args.out / f"page-{number}.png")

        score = structural_similarity(
            np.asarray(ref.convert("L")), np.asarray(exp.convert("L")), data_range=255
        )
        print(f"page {number:>3}: ssim {score:.4f}")


if __name__ == "__main__":
    main()
