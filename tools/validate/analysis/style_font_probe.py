"""Probe deck for embedded bold/italic faces in the PowerPoint desktop app
(2026-10-24).

The Libertinus demo exported from PowerPoint drew bold text in the regular
face: its PDF has one Libertinus font where the IBM Plex demo has three.
IBM Plex is a Microsoft cloud font, so its bold may never have come from the
file. Each line here is one family under a new name (no installed or cloud
font can stand in), with runs "Regular", bold, italic, bold italic:

- S0: IBM Plex Sans regular/bold/italic, original TrueType, renamed: do our
  non-regular embedded faces work at all?
- S1: converted Libertinus Serif regular/bold/italic, renamed.
- S2: converted Libertinus as exported ("Libertinus Serif").
- S3: like S0, `p:font` with pitchFamily/charset as PowerPoint writes them.
- S4: like S1, same.
- S5: converted Libertinus Serif Semibold as exported (its own family).

Real bold has wider, heavier letters; PowerPoint's synthetic bold only
thickens the regular ones. Pure Python. Args: template.pptx (a one-slide
typst-pptx deck), libertinus.pptx (a typst-pptx deck embedding converted
Libertinus Serif Regular, Bold, Italic and Semibold), output.pptx.
"""
import re
import struct
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from web_font_probe import build, eot, esc, name_table, names, parse  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]
PLEX = ROOT / "fixtures/fonts/ibm-plex"


def rename(t, family, style):
    keep = [r for r in names(t["name"]) if r[3] not in (1, 2, 3, 4, 6, 16, 17)]
    plats = sorted({r[:3] for r in names(t["name"])})
    full = family if style == "Regular" else f"{family} {style}"
    ps = family.replace(" ", "") + "-" + style.replace(" ", "")
    for p, e, l in plats:
        for nid, s in ((1, family), (2, style), (3, "probe;" + ps), (4, full), (6, ps)):
            keep.append((p, e, l, nid, s))
    t["name"] = name_table(keep)


def libertinus(pptx):
    z = zipfile.ZipFile(pptx)
    out = {}
    for n in z.namelist():
        if n.endswith(".fntdata"):
            d = z.read(n)
            ttf = d[-struct.unpack_from("<I", d, 4)[0] :]
            full = next(s for p, e, l, nid, s in names(parse(ttf)["name"]) if nid == 4 and p == 3)
            out[full] = ttf
    return out


STYLES = ("regular", "bold", "italic")
SUBFAMILY = {"regular": "Regular", "bold": "Bold", "italic": "Italic"}


def families(lib):
    plex = {s: (PLEX / f"IBMPlexSans-{SUBFAMILY[s]}.ttf").read_bytes() for s in STYLES}
    libs = {s: lib[f"Libertinus Serif {SUBFAMILY[s]}"] for s in STYLES}
    plan = [
        ("S0", "Probe S0", plex, True, False, "IBM Plex Sans R/B/I renamed"),
        ("S1", "Probe S1", libs, True, False, "converted Libertinus R/B/I renamed"),
        ("S2", "Libertinus Serif", libs, False, False, "converted Libertinus as exported"),
        ("S3", "Probe S3", plex, True, True, "S0 + pitchFamily/charset"),
        ("S4", "Probe S4", libs, True, True, "S1 + pitchFamily/charset"),
        ("S5", "Libertinus Serif Semibold", {"regular": lib["Libertinus Serif Semibold"]}, False, False,
         "converted Semibold as exported (own family)"),
    ]
    for key, family, faces, renamed, attrs, label in plan:
        built = {}
        for style, data in faces.items():
            t = parse(data)
            if renamed:
                rename(t, family, SUBFAMILY[style])
            built[style] = build(t)
        yield key, family, built, attrs, label


def run(typeface, text, b, i, size=2400):
    return (
        f'<a:r><a:rPr lang="en-US" sz="{size}" b="{b}" i="{i}" dirty="0"><a:solidFill><a:srgbClr val="000000"/></a:solidFill>'
        f'<a:latin typeface="{esc(typeface)}"/><a:ea typeface="{esc(typeface)}"/><a:cs typeface="{esc(typeface)}"/></a:rPr>'
        f"<a:t>{esc(text)}</a:t></a:r>"
    )


def text_box(id_, x, y, w, runs):
    return (
        f'<p:sp><p:nvSpPr><p:cNvPr id="{id_}" name="Text {id_}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>'
        f'<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="450000"/></a:xfrm>'
        f'<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>'
        f'<p:txBody><a:bodyPr wrap="none" lIns="0" tIns="0" rIns="0" bIns="0"><a:noAutofit/></a:bodyPr><a:lstStyle/>'
        f"<a:p>{''.join(runs)}</a:p></p:txBody></p:sp>"
    )


def main(template, libertinus_deck, output):
    fams = list(families(libertinus(libertinus_deck)))
    shapes = []
    for k, (key, family, _, _, label) in enumerate(fams):
        y = 300000 + k * 720000
        shapes.append(text_box(2 * k + 2, 250000, y, 8600000, [run("Arial", f"{key}: {label}", 0, 0, 1100)]))
        runs = [
            run(family, "Regular Hamburg ", 0, 0),
            run(family, "Bold Hamburg ", 1, 0),
            run(family, "Italic Hamburg ", 0, 1),
            run(family, "BoldItalic", 1, 1),
        ]
        shapes.append(text_box(2 * k + 3, 250000, y + 230000, 8600000, runs))
    tin = zipfile.ZipFile(template)
    slide = tin.read("ppt/slides/slide1.xml").decode()
    head, _, rest = slide.partition("<p:sp>")
    slide = head + "".join(shapes) + rest[rest.rindex("</p:spTree>") :]
    slide = re.sub(r"<p:pic>.*?</p:pic>", "", slide, flags=re.S)
    rels_slide = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>'
    )
    pres = tin.read("ppt/presentation.xml").decode()
    rels = tin.read("ppt/_rels/presentation.xml.rels").decode()
    rels = re.sub(r'<Relationship Id="rId\d+" Type="[^"]*/font" Target="[^"]*"/>', "", rels)
    lst, new_rels, parts = "<p:embeddedFontLst>", "", []
    for key, family, faces, attrs, _ in fams:
        panose = parse(faces["regular"])["OS/2"][32:42].hex().upper()
        extra = ' pitchFamily="18" charset="0"' if attrs else ""
        lst += f'<p:embeddedFont><p:font typeface="{esc(family)}" panose="{panose}"{extra}/>'
        for style, ttf in faces.items():
            rid = f"rId{100 + len(parts)}"
            parts.append(ttf)
            lst += f'<p:{style} r:id="{rid}"/>'
            new_rels += f'<Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/font" Target="fonts/font{len(parts)}.fntdata"/>'
        lst += "</p:embeddedFont>"
    lst += "</p:embeddedFontLst>"
    if "<p:embeddedFontLst>" in pres:
        pres = re.sub(r"<p:embeddedFontLst>.*</p:embeddedFontLst>", lst, pres, flags=re.S)
    else:
        pres = pres.replace("<p:defaultTextStyle", lst + "<p:defaultTextStyle")
    rels = rels.replace("</Relationships>", new_rels + "</Relationships>")
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as out:
        for item in tin.infolist():
            n = item.filename
            if n.startswith(("ppt/fonts/", "ppt/media/")):
                continue
            data = {
                "ppt/slides/slide1.xml": slide.encode(),
                "ppt/slides/_rels/slide1.xml.rels": rels_slide.encode(),
                "ppt/presentation.xml": pres.encode(),
                "ppt/_rels/presentation.xml.rels": rels.encode(),
            }.get(n) or tin.read(n)
            out.writestr(n, data)
        for i, ttf in enumerate(parts):
            out.writestr(f"ppt/fonts/font{i + 1}.fntdata", eot(ttf))
    for key, family, faces, _, label in fams:
        print(f"{key}  {family:26} {'/'.join(faces)}  {label}")


if __name__ == "__main__":
    main(*sys.argv[1:4])
