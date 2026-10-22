"""Structure of a .pptx: per deck totals of shapes, text, fonts, lists, tables."""
import collections, re, sys, zipfile

for path in sys.argv[1:]:
    z = zipfile.ZipFile(path)
    slides = sorted(n for n in z.namelist() if re.fullmatch(r"ppt/slides/slide\d+\.xml", n))
    c = collections.Counter()
    fonts = collections.Counter()
    for s in slides:
        x = z.read(s).decode()
        c["sp"] += x.count("<p:sp>") + x.count("<p:sp ")
        c["txBody"] += x.count("<p:txBody>")
        c["paragraphs"] += x.count("<a:p>") + x.count("<a:p ")
        c["pic"] += x.count("<p:pic>")
        c["grpSp"] += x.count("<p:grpSp>")
        c["custGeom"] += x.count("<a:custGeom>")
        c["tbl"] += x.count("<a:tbl>")
        c["buChar/buAutoNum"] += x.count("<a:buChar") + x.count("<a:buAutoNum")
        c["wrap=none"] += x.count('wrap="none"')
        c["hlinkClick"] += x.count("<a:hlinkClick")
        for f in re.findall(r'<a:latin typeface="([^"]+)"', x):
            fonts[f] += 1
    media = [n for n in z.namelist() if n.startswith("ppt/media/")]
    notes = [n for n in z.namelist() if re.fullmatch(r"ppt/notesSlides/notesSlide\d+\.xml", n)]
    pres = z.read("ppt/presentation.xml").decode()
    print(f"== {path.split('/')[-1]}  slides {len(slides)}  media {len(media)}  notes {len(notes)}  embedded fonts {pres.count('<p:embeddedFont>')}  size {sum(i.file_size for i in z.infolist())//1024} KiB")
    print("  ", dict(c))
    print("   fonts:", dict(fonts.most_common(6)))
