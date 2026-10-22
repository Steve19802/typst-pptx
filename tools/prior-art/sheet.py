"""Side-by-side PNG: Typst | typst-pptx | typ2pptx for given pages.
Args: out_dir typst.pdf ours.pdf theirs.pdf page..."""
import sys
import pypdfium2 as pdfium
from PIL import Image, ImageDraw

out, *pdfs = sys.argv[1:5]
pages = [int(p) for p in sys.argv[5:]]
docs = [pdfium.PdfDocument(p) for p in pdfs]
labels = ["Typst", "typst-pptx (LibreOffice)", "typ2pptx (LibreOffice)"]
for n in pages:
    imgs = []
    for d in docs:
        pg = d[n - 1]
        imgs.append(pg.render(scale=700 / pg.get_width()).to_pil())
    h = max(i.height for i in imgs)
    sheet = Image.new("RGB", (sum(i.width for i in imgs) + 20, h + 30), "white")
    x = 0
    for i, lab in zip(imgs, labels):
        sheet.paste(i, (x, 30))
        ImageDraw.Draw(sheet).text((x + 5, 8), lab, fill="black")
        x += i.width + 10
    sheet.save(f"{out}-p{n}.png")
