"""Visual diff of a Typst reference PDF and LibreOffice's rendering of the
exported pptx (plan §11.3).

    uv run --project tools/validate python -I tools/validate/compare.py \
        REFERENCE.pdf EXPORT.pdf OUT_DIR [--dpi 72]

Writes OUT_DIR/page-N.png (reference | export | difference) and prints SSIM
per page. With --min-ssim, exits 1 if a page scores lower or the page
counts differ.
"""

import argparse
import sys
import pathlib

import numpy as np
import pypdfium2 as pdfium
from PIL import Image, ImageChops
from skimage.metrics import structural_similarity


def render(path: pathlib.Path, dpi: int) -> list[Image.Image]:
    pdf = pdfium.PdfDocument(str(path))
    return [page.render(scale=dpi / 72).to_pil().convert("RGB") for page in pdf]


def crop_to_aspect(image: Image.Image, aspect: float) -> Image.Image:
    """PowerPoint prints slides centred on the paper (e.g. 16:9 on A4);
    cut the slide back out."""
    width, height = image.size
    if abs(width / height - aspect) < 0.01:
        return image
    if width / height < aspect:
        slide = round(width / aspect)
        top = (height - slide) // 2
        return image.crop((0, top, width, top + slide))
    slide = round(height * aspect)
    left = (width - slide) // 2
    return image.crop((left, 0, left + slide, height))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("reference", type=pathlib.Path)
    parser.add_argument("export", type=pathlib.Path)
    parser.add_argument("out", type=pathlib.Path)
    parser.add_argument("--dpi", type=int, default=72)
    parser.add_argument("--min-ssim", type=float)
    args = parser.parse_args()
    failed = False

    reference = render(args.reference, args.dpi)
    export = render(args.export, args.dpi)
    if len(reference) != len(export):
        print(f"page count differs: reference {len(reference)}, export {len(export)}")
        failed = True

    args.out.mkdir(parents=True, exist_ok=True)
    for number, (ref, exp) in enumerate(zip(reference, export), start=1):
        exp = crop_to_aspect(exp, ref.width / ref.height).resize(ref.size)
        diff = ImageChops.invert(ImageChops.difference(ref, exp))
        sheet = Image.new("RGB", (ref.width * 3, ref.height), "white")
        for column, image in enumerate((ref, exp, diff)):
            sheet.paste(image, (column * ref.width, 0))
        sheet.save(args.out / f"page-{number}.png")

        score = structural_similarity(
            np.asarray(ref.convert("L")), np.asarray(exp.convert("L")), data_range=255
        )
        low = args.min_ssim is not None and score < args.min_ssim
        failed |= low
        print(f"page {number:>3}: ssim {score:.4f}{'  BELOW THRESHOLD' if low else ''}")
    if args.min_ssim is not None and failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
