"""Probe deck for embedded fonts in PowerPoint for the web (2026-10-24).

The web editor's canvas showed embedded IBM Plex but substituted the
CFF-to-TrueType conversions of Libertinus Serif. This writes one slide with
one line per font variant, each changing one property, all under new family
names (so no installed or Microsoft cloud font can stand in):

- P*: IBM Plex Mono variants. Line `iiiiiiiiii|WWWWWWWWWW|`: the two halves
  are equally wide only in the embedded monospace font.
- L*: the converted Libertinus Serif Regular taken from a typst-pptx deck.
  Serif means embedded; sans means substituted.

Pure Python (no fontTools), so it runs anywhere. Args: template.pptx (a
one-slide typst-pptx deck), libertinus.pptx (a typst-pptx deck embedding
converted Libertinus Serif Regular), output.pptx.
"""
import re
import struct
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MONO = ROOT / "fixtures/fonts/ibm-plex/IBMPlexMono-Regular.ttf"


def u16(d, o):
    return struct.unpack_from(">H", d, o)[0]


def u32(d, o):
    return struct.unpack_from(">I", d, o)[0]


def parse(d):
    return {
        d[12 + 16 * i : 16 + 16 * i].decode("latin1"): d[u32(d, 20 + 16 * i) : u32(d, 20 + 16 * i) + u32(d, 24 + 16 * i)]
        for i in range(u16(d, 4))
    }


def checksum(b):
    b += b"\0" * (-len(b) % 4)
    return sum(struct.unpack(f">{len(b) // 4}I", b)) & 0xFFFFFFFF


def build(t):
    t = dict(t)
    head = bytearray(t["head"])
    head[8:12] = b"\0\0\0\0"
    t["head"] = bytes(head)
    tags = sorted(t)
    n = len(tags)
    es = n.bit_length() - 1
    out = bytearray(struct.pack(">IHHHH", 0x00010000, n, 16 << es, es, n * 16 - (16 << es)))
    offset = 12 + 16 * n
    body = bytearray()
    for tag in tags:
        data = t[tag]
        out += struct.pack(">4sIII", tag.encode("latin1"), checksum(data), offset + len(body), len(data))
        body += data + b"\0" * (-len(data) % 4)
    out += body
    at = 12 + 16 * n + sum(len(t[x]) + (-len(t[x]) % 4) for x in tags[: tags.index("head")])
    struct.pack_into(">I", out, at + 8, (0xB1B0AFBA - checksum(bytes(out))) & 0xFFFFFFFF)
    return bytes(out)


def names(nm):
    so = u16(nm, 4)
    out = []
    for i in range(u16(nm, 2)):
        p, e, l, nid, ln, off = struct.unpack_from(">6H", nm, 6 + 12 * i)
        raw = nm[so + off : so + off + ln]
        out.append((p, e, l, nid, raw.decode("latin1") if p == 1 else raw.decode("utf-16-be")))
    return out


def name_table(records):
    records = sorted(records)
    head = bytearray(struct.pack(">HHH", 0, len(records), 6 + 12 * len(records)))
    strings = bytearray()
    for p, e, l, nid, s in records:
        raw = s.encode("latin1") if p == 1 else s.encode("utf-16-be")
        head += struct.pack(">6H", p, e, l, nid, len(raw), len(strings))
        strings += raw
    return bytes(head + strings)


def rename(t, family):
    keep = [r for r in names(t["name"]) if r[3] not in (1, 2, 3, 4, 6, 16, 17)]
    plats = sorted({r[:3] for r in names(t["name"])})
    ps = family.replace(" ", "")
    for p, e, l in plats:
        for nid, s in ((1, family), (2, "Regular"), (3, "probe;" + ps), (4, family), (6, ps)):
            keep.append((p, e, l, nid, s))
    t["name"] = name_table(keep)


def glyphs(t):
    long_loca = u16(t["head"], 50)
    n = u16(t["maxp"], 4)
    loca = t["loca"]
    offs = [u32(loca, 4 * i) if long_loca else 2 * u16(loca, 2 * i) for i in range(n + 1)]
    return [t["glyf"][offs[i] : offs[i + 1]] for i in range(n)]


def set_glyphs(t, gs):
    glyf, loca = bytearray(), bytearray()
    for g in gs:
        loca += struct.pack(">I", len(glyf))
        glyf += g + b"\0" * (-len(g) % 4)
    loca += struct.pack(">I", len(glyf))
    t["glyf"], t["loca"] = bytes(glyf), bytes(loca)
    head = bytearray(t["head"])
    struct.pack_into(">h", head, 50, 1)
    t["head"] = bytes(head)


def strip_instructions(g):
    if not g:
        return g
    nc = struct.unpack_from(">h", g)[0]
    if nc >= 0:
        at = 10 + 2 * nc
        return g[:at] + b"\0\0" + g[at + 2 + u16(g, at) :]
    g = bytearray(g)
    at, more = 10, True
    while more:
        flags = u16(g, at)
        struct.pack_into(">H", g, at, flags & ~0x100)
        more = bool(flags & 0x20)
        at += 4 + (4 if flags & 1 else 2)
        at += 2 if flags & 8 else 4 if flags & 0x40 else 8 if flags & 0x80 else 0
    return bytes(g[:at])


def no_hinting(t):
    for tag in ("fpgm", "cvt ", "prep"):
        t.pop(tag, None)
    set_glyphs(t, [strip_instructions(g) for g in glyphs(t)])
    maxp = bytearray(t["maxp"])
    struct.pack_into(">7H", maxp, 14, 1, 0, 0, 0, 0, 0, 0)
    t["maxp"] = bytes(maxp)


def post_v3(t):
    t["post"] = b"\x00\x03\x00\x00" + t["post"][4:32]


def subtable_len(c, off):
    fmt = u16(c, off)
    if fmt in (0, 2, 4, 6):
        return u16(c, off + 2)
    if fmt == 14:
        return u32(c, off + 2)
    return u32(c, off + 4)


def no_mac(t):
    t["name"] = name_table([r for r in names(t["name"]) if r[0] != 1])
    c = t["cmap"]
    recs = [struct.unpack_from(">HHI", c, 4 + 8 * i) for i in range(u16(c, 2))]
    recs = [r for r in recs if r[0] != 1]
    body, moved = bytearray(), {}
    base = 4 + 8 * len(recs)
    for _, _, off in recs:
        if off not in moved:
            moved[off] = base + len(body)
            body += c[off : off + subtable_len(c, off)]
    t["cmap"] = struct.pack(">HH", 0, len(recs)) + b"".join(
        struct.pack(">HHI", p, e, moved[off]) for p, e, off in recs
    ) + bytes(body)


def zero_panose(t):
    os2 = bytearray(t["OS/2"])
    os2[32:42] = bytes(10)
    t["OS/2"] = bytes(os2)


def head_flags(t):
    head = bytearray(t["head"])
    struct.pack_into(">H", head, 16, 0x0003)
    t["head"] = bytes(head)


def cmap4_glyphs(t, chars):
    c = t["cmap"]
    off = next(
        o for p, e, o in (struct.unpack_from(">HHI", c, 4 + 8 * i) for i in range(u16(c, 2)))
        if (p, e) == (3, 1)
    )
    seg = u16(c, off + 6) // 2
    ends = [u16(c, off + 14 + 2 * i) for i in range(seg)]
    starts = [u16(c, off + 16 + 2 * seg + 2 * i) for i in range(seg)]
    deltas = [struct.unpack_from(">h", c, off + 16 + 4 * seg + 2 * i)[0] for i in range(seg)]
    ro_at = off + 16 + 6 * seg
    out = set()
    for ch in chars:
        for i in range(seg):
            if starts[i] <= ch <= ends[i]:
                ro = u16(c, ro_at + 2 * i)
                if ro == 0:
                    out.add((ch + deltas[i]) & 0xFFFF)
                else:
                    g = u16(c, ro_at + 2 * i + ro + 2 * (ch - starts[i]))
                    if g:
                        out.add((g + deltas[i]) & 0xFFFF)
                break
    return out


def ascii_only(t):
    keep = cmap4_glyphs(t, range(0x20, 0x7F)) | {0}
    set_glyphs(t, [g if i in keep else b"" for i, g in enumerate(glyphs(t))])


def eot(ttf):
    t = parse(ttf)
    os2, head = t["OS/2"], t["head"]
    recs = names(t["name"])

    def name(nid):
        s = next((s for p, e, l, i, s in recs if i == nid and p == 3), "")
        return s.encode("utf-16-le")

    out = bytearray(struct.pack("<III I", 0, len(ttf), 0x00020001, 0))
    out += os2[32:42]
    out += struct.pack("<BBIHH", 1, u16(os2, 62) & 1, u16(os2, 4), u16(os2, 8), 0x504C)
    for at in (42, 46, 50, 54, 78, 82):
        out += struct.pack("<I", u32(os2, at) if at + 4 <= len(os2) else 0)
    out += struct.pack("<I", u32(head, 8)) + bytes(16)
    for nid in (1, 2, 5, 4):
        s = name(nid)
        out += struct.pack("<HH", 0, len(s)) + s
    out += struct.pack("<HH", 0, 0) + ttf
    struct.pack_into("<I", out, 0, len(out))
    return bytes(out)


def libertinus_regular(pptx):
    z = zipfile.ZipFile(pptx)
    for n in z.namelist():
        if n.endswith(".fntdata"):
            d = z.read(n)
            ttf = d[-struct.unpack_from("<I", d, 4)[0] :]
            if any(s == "Libertinus Serif Regular" for *_, nid, s in names(parse(ttf)["name"]) if nid == 4):
                return ttf
    raise SystemExit("no Libertinus Serif Regular in " + pptx)


def variants(libertinus):
    mono = MONO.read_bytes()
    plan = [
        ("P0", "IBM Plex Mono", mono, [], "original IBM Plex Mono, own name (cloud font?)"),
        ("PA", "Probe PA", mono, [], "Plex Mono renamed"),
        ("PB", "Probe PB", mono, [no_hinting], "renamed, hinting removed"),
        ("PC", "Probe PC", mono, [post_v3], "renamed, post v3"),
        ("PD", "Probe PD", mono, [no_mac], "renamed, no Mac names/cmap"),
        ("PE", "Probe PE", mono, [zero_panose], "renamed, PANOSE zero"),
        ("PF", "Probe PF", mono, [head_flags], "renamed, head flags 3"),
        ("PG", "Probe PG", mono, [no_hinting, post_v3, no_mac, zero_panose, head_flags], "renamed, all of B-F"),
        ("L0", "Libertinus Serif", libertinus, [], "converted Libertinus as exported"),
        ("L1", "Probe LA", libertinus, [], "converted Libertinus renamed"),
        ("L2", "Probe LB", libertinus, [ascii_only], "renamed, only ASCII glyphs kept (small)"),
    ]
    for key, family, data, edits, label in plan:
        t = parse(data)
        if family not in ("IBM Plex Mono", "Libertinus Serif"):
            rename(t, family)
        for edit in edits:
            edit(t)
        yield key, family, build(t), label


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def text_box(id_, x, y, w, typeface, text):
    rpr = (
        f'<a:rPr lang="en-US" sz="1600" dirty="0"><a:solidFill><a:srgbClr val="000000"/></a:solidFill>'
        f'<a:latin typeface="{esc(typeface)}"/><a:ea typeface="{esc(typeface)}"/><a:cs typeface="{esc(typeface)}"/></a:rPr>'
    )
    return (
        f'<p:sp><p:nvSpPr><p:cNvPr id="{id_}" name="Text {id_}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>'
        f'<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="330000"/></a:xfrm>'
        f'<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>'
        f'<p:txBody><a:bodyPr wrap="none" lIns="0" tIns="0" rIns="0" bIns="0"><a:noAutofit/></a:bodyPr><a:lstStyle/>'
        f'<a:p><a:r>{rpr}<a:t>{esc(text)}</a:t></a:r></a:p></p:txBody></p:sp>'
    )


def main(template, libertinus_deck, output):
    fonts = list(variants(libertinus_regular(libertinus_deck)))
    shapes = []
    for i, (key, family, _, label) in enumerate(fonts):
        y = 250000 + i * 420000
        shapes.append(text_box(2 * i + 2, 250000, y, 3900000, "Arial", f"{key}: {label}"))
        probe = "iiiiiiiiii|WWWWWWWWWW|" if key.startswith("P") else "Hamburg Quixote serif 0123"
        shapes.append(text_box(2 * i + 3, 4300000, y, 4600000, family, probe))
    tin = zipfile.ZipFile(template)
    slide = tin.read("ppt/slides/slide1.xml").decode()
    head, _, rest = slide.partition("<p:sp>")
    tail = rest[rest.rindex("</p:spTree>") :]
    slide = head + "".join(shapes) + tail
    slide = re.sub(r"<p:pic>.*?</p:pic>", "", slide, flags=re.S)
    rels_slide = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>'
    )
    pres = tin.read("ppt/presentation.xml").decode()
    rels = tin.read("ppt/_rels/presentation.xml.rels").decode()
    rels = re.sub(r'<Relationship Id="rId\d+" Type="[^"]*/font" Target="[^"]*"/>', "", rels)
    lst, new_rels = "<p:embeddedFontLst>", ""
    for i, (key, family, ttf, _) in enumerate(fonts):
        rid = f"rId{100 + i}"
        panose = parse(ttf)["OS/2"][32:42].hex().upper()
        lst += f'<p:embeddedFont><p:font typeface="{esc(family)}" panose="{panose}"/><p:regular r:id="{rid}"/></p:embeddedFont>'
        new_rels += f'<Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/font" Target="fonts/font{i + 1}.fntdata"/>'
    lst += "</p:embeddedFontLst>"
    pres = re.sub(r"<p:embeddedFontLst>.*</p:embeddedFontLst>", lst, pres, flags=re.S)
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
        for i, (key, family, ttf, label) in enumerate(fonts):
            out.writestr(f"ppt/fonts/font{i + 1}.fntdata", eot(ttf))
            print(f"{key:3} {family:18} {len(ttf) // 1024:5} KiB  {label}")


if __name__ == "__main__":
    main(*sys.argv[1:4])
