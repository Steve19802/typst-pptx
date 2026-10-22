//! TrueType outlines for CFF-flavoured OpenType fonts, so PowerPoint can
//! embed them: it accepts `glyf` outlines only, and Typst's own fonts
//! (Libertinus Serif, New Computer Modern), used whenever a deck's fonts
//! aren't found, are CFF.
//!
//! Every glyph's cubic Béziers are approximated by quadratic ones within
//! [`TOLERANCE`] and its contours are reversed to TrueType's clockwise
//! direction. All other tables are kept except `CFF `, `CFF2`, `VORG` and
//! `DSIG`; `maxp` becomes version 1.0, `head` gets long `loca` offsets, the
//! new bounding box and checksum adjustment, and `hmtx`/`hhea` the new left
//! side bearings: TrueType rasterisers place a glyph so that its `xMin` sits
//! at the bearing, which CFF fonts needn't keep exact (Libertinus Serif
//! Italic's `@` is 23 units off). The result has no hinting.

use ttf_parser::{GlyphId, OutlineBuilder};

/// Maximum distance between a cubic and its quadratic approximation, in
/// font units (0.001 em at 1000 units per em; coordinates are rounded to
/// whole units anyway).
const TOLERANCE: f32 = 1.0;

/// Upper bound on the quadratics per cubic.
const MAX_PIECES: usize = 32;

/// Tables not copied: CFF outlines and their companions, and the two
/// tables that are rewritten.
const DROPPED: [&[u8; 4]; 8] = [
    b"CFF ", b"CFF2", b"VORG", b"DSIG", b"head", b"hhea", b"hmtx", b"maxp",
];

/// Whether the font has CFF outlines instead of TrueType ones.
pub fn is_cff(data: &[u8]) -> bool {
    directory(data).is_some_and(|tables| {
        tables
            .iter()
            .any(|(tag, _)| tag == b"CFF " || tag == b"CFF2")
            && !tables.iter().any(|(tag, _)| tag == b"glyf")
    })
}

/// The font with TrueType outlines, or `None` if it can't be parsed or a
/// coordinate doesn't fit TrueType's 16 bits.
pub fn from_cff(data: &[u8]) -> Option<Vec<u8>> {
    let face = ttf_parser::Face::parse(data, 0).ok()?;
    let source = directory(data)?;
    let count = face.number_of_glyphs();

    let mut glyf = Vec::new();
    let mut loca = Vec::with_capacity(4 * (usize::from(count) + 1));
    let (mut max_points, mut max_contours) = (0u16, 0u16);
    let mut boxes = Vec::with_capacity(usize::from(count));
    for id in 0..count {
        loca.extend_from_slice(&u32::try_from(glyf.len()).ok()?.to_be_bytes());
        let mut pen = Pen::default();
        let mut bbox = None;
        if face.outline_glyph(GlyphId(id), &mut pen).is_some() {
            pen.close();
            if !pen.contours.is_empty() {
                let glyph = encode(&pen.contours, &mut glyf)?;
                max_points = max_points.max(glyph.points);
                max_contours = max_contours.max(glyph.contours);
                bbox = Some(glyph.bbox);
            }
        }
        boxes.push(bbox);
        glyf.resize(glyf.len().next_multiple_of(4), 0);
    }
    loca.extend_from_slice(&u32::try_from(glyf.len()).ok()?.to_be_bytes());

    let table = |tag: &[u8; 4]| {
        source
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, d)| d.to_vec())
    };
    let (mut hhea, mut hmtx) = (table(b"hhea")?, table(b"hmtx")?);
    let metrics = usize::from(u16::from_be_bytes(hhea.get(34..36)?.try_into().ok()?));
    let (mut min_lsb, mut min_rsb, mut max_extent) = (i16::MAX, i16::MAX, i16::MIN);
    let mut advance = 0;
    for (id, bbox) in boxes.iter().enumerate() {
        // Long metrics (advance, bearing), then bearings only, the last
        // advance repeating.
        let at = if id < metrics {
            advance = u16::from_be_bytes(hmtx.get(4 * id..4 * id + 2)?.try_into().ok()?);
            4 * id + 2
        } else {
            4 * metrics + 2 * (id - metrics)
        };
        let Some([x_min, _, x_max, _]) = *bbox else {
            continue;
        };
        hmtx.get_mut(at..at + 2)?
            .copy_from_slice(&x_min.to_be_bytes());
        let width = i32::from(x_max) - i32::from(x_min);
        min_lsb = min_lsb.min(x_min);
        min_rsb = min_rsb.min(i16::try_from(i32::from(advance) - i32::from(x_max)).ok()?);
        max_extent = max_extent.max(i16::try_from(i32::from(x_min) + width).ok()?);
    }
    if max_extent > i16::MIN {
        for (at, value) in [(12, min_lsb), (14, min_rsb), (16, max_extent)] {
            hhea.get_mut(at..at + 2)?
                .copy_from_slice(&value.to_be_bytes());
        }
    }

    let mut head = table(b"head")?;
    head.get_mut(8..12)?.fill(0); // checkSumAdjustment, set below
    let font_box = boxes
        .iter()
        .flatten()
        .fold(None, |acc: Option<[i16; 4]>, b| {
            Some(acc.map_or(*b, |a| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            }))
        });
    if let Some(font_box) = font_box {
        for (k, value) in font_box.iter().enumerate() {
            head.get_mut(36 + 2 * k..38 + 2 * k)?
                .copy_from_slice(&value.to_be_bytes());
        }
    }
    head.get_mut(50..52)?.copy_from_slice(&1i16.to_be_bytes()); // long loca

    let mut maxp = Vec::with_capacity(32);
    maxp.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    for value in [count, max_points, max_contours, 0, 0, 2] {
        maxp.extend_from_slice(&value.to_be_bytes());
    }
    maxp.resize(32, 0); // twilight points .. component depth: no hinting

    let mut tables: Vec<([u8; 4], &[u8])> = source
        .iter()
        .filter(|(tag, _)| !DROPPED.contains(&tag))
        .copied()
        .collect();
    tables.push((*b"glyf", &glyf));
    tables.push((*b"head", &head));
    tables.push((*b"hhea", &hhea));
    tables.push((*b"hmtx", &hmtx));
    tables.push((*b"loca", &loca));
    tables.push((*b"maxp", &maxp));
    tables.sort_by_key(|(tag, _)| *tag);
    let mut font = sfnt(&tables)?;

    let head_at = offset_of(&font, b"head")?;
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&font));
    font[head_at + 8..head_at + 12].copy_from_slice(&adjustment.to_be_bytes());
    Some(font)
}

fn record(data: &[u8], i: usize) -> Option<([u8; 4], usize, usize)> {
    let record = data.get(12 + 16 * i..28 + 16 * i)?;
    let word = |k: usize| u32::from_be_bytes(record[k..k + 4].try_into().unwrap()) as usize;
    Some((record[..4].try_into().ok()?, word(8), word(12)))
}

fn table_count(data: &[u8]) -> Option<usize> {
    Some(usize::from(u16::from_be_bytes(
        data.get(4..6)?.try_into().ok()?,
    )))
}

/// The table directory: (tag, data) per table.
fn directory(data: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    (0..table_count(data)?)
        .map(|i| {
            let (tag, offset, length) = record(data, i)?;
            Some((tag, data.get(offset..offset + length)?))
        })
        .collect()
}

fn offset_of(data: &[u8], tag: &[u8; 4]) -> Option<usize> {
    (0..table_count(data)?).find_map(|i| record(data, i).filter(|r| &r.0 == tag).map(|r| r.1))
}

fn sfnt(tables: &[([u8; 4], &[u8])]) -> Option<Vec<u8>> {
    let count = u16::try_from(tables.len()).ok()?;
    let log2 = 15 - count.leading_zeros() as u16;
    let search_range = 16u16 << log2;
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    for value in [count, search_range, log2, count * 16 - search_range] {
        out.extend_from_slice(&value.to_be_bytes());
    }
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&u32::try_from(offset).ok()?.to_be_bytes());
        out.extend_from_slice(&u32::try_from(data.len()).ok()?.to_be_bytes());
        offset += data.len().next_multiple_of(4);
    }
    for (_, data) in tables {
        out.extend_from_slice(data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    Some(out)
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

#[derive(Clone, Copy, PartialEq)]
struct Point {
    x: f32,
    y: f32,
    on: bool,
}

/// Collects a glyph's contours as TrueType points (on and off curve).
#[derive(Default)]
struct Pen {
    contours: Vec<Vec<Point>>,
    current: Vec<Point>,
}

impl Pen {
    fn push(&mut self, x: f32, y: f32, on: bool) {
        self.current.push(Point { x, y, on });
    }
}

impl OutlineBuilder for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.push(x, y, true);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.push(x, y, true);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.push(x1, y1, false);
        self.push(x, y, true);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let start = self.current.last().map_or((0.0, 0.0), |p| (p.x, p.y));
        for [control, end] in quadratics([start, (x1, y1), (x2, y2), (x, y)]) {
            self.push(control.0, control.1, false);
            self.push(end.0, end.1, true);
        }
    }

    /// Ends the contour: drops the closing point if it repeats the first and
    /// reverses the direction (CFF outer contours run counter-clockwise,
    /// TrueType's clockwise).
    fn close(&mut self) {
        let mut contour = std::mem::take(&mut self.current);
        if contour.len() > 1 && contour.first() == contour.last() {
            contour.pop();
        }
        if contour.len() > 1 {
            contour[1..].reverse();
            self.contours.push(contour);
        }
    }
}

type Vec2 = (f32, f32);

/// Quadratics (control, end) approximating a cubic: the cubic is split into
/// `n` equal parameter ranges, each replaced by the quadratic with control
/// point `(3 (c1 + c2) − (p0 + p3)) / 4`. That quadratic's error is at most
/// `√3/36 · |p3 − 3 c2 + 3 c1 − p0|`, and the third difference shrinks with
/// `n³`, which gives `n` directly.
fn quadratics(cubic: [Vec2; 4]) -> Vec<[Vec2; 2]> {
    let [p0, p1, p2, p3] = cubic;
    let third = (
        p3.0 - 3.0 * p2.0 + 3.0 * p1.0 - p0.0,
        p3.1 - 3.0 * p2.1 + 3.0 * p1.1 - p0.1,
    );
    let error = 3f32.sqrt() / 36.0 * third.0.hypot(third.1);
    let n = ((error / TOLERANCE).cbrt().ceil() as usize).clamp(1, MAX_PIECES);
    (0..n)
        .map(|i| {
            let (t0, t1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            let [q0, q1, q2, q3] = piece(cubic, t0, t1);
            let control = (
                (3.0 * (q1.0 + q2.0) - (q0.0 + q3.0)) / 4.0,
                (3.0 * (q1.1 + q2.1) - (q0.1 + q3.1)) / 4.0,
            );
            [control, q3]
        })
        .collect()
}

/// The part of a cubic between `t0` and `t1`, as a cubic.
fn piece(c: [Vec2; 4], t0: f32, t1: f32) -> [Vec2; 4] {
    let combine = |w: &[f32], p: &[Vec2]| {
        (
            w.iter().zip(p).map(|(w, p)| w * p.0).sum::<f32>(),
            w.iter().zip(p).map(|(w, p)| w * p.1).sum::<f32>(),
        )
    };
    let at = |t: f32| {
        let s = 1.0 - t;
        combine(
            &[s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t],
            &c,
        )
    };
    let diff = [
        (c[1].0 - c[0].0, c[1].1 - c[0].1),
        (c[2].0 - c[1].0, c[2].1 - c[1].1),
        (c[3].0 - c[2].0, c[3].1 - c[2].1),
    ];
    let tangent = |t: f32| {
        let s = 1.0 - t;
        combine(&[3.0 * s * s, 6.0 * s * t, 3.0 * t * t], &diff)
    };
    let k = (t1 - t0) / 3.0;
    let (a, b) = (at(t0), at(t1));
    let (da, db) = (tangent(t0), tangent(t1));
    [
        a,
        (a.0 + k * da.0, a.1 + k * da.1),
        (b.0 - k * db.0, b.1 - k * db.1),
        b,
    ]
}

struct Glyph {
    points: u16,
    contours: u16,
    /// x min, y min, x max, y max
    bbox: [i16; 4],
}

/// Appends a simple glyph.
fn encode(contours: &[Vec<Point>], out: &mut Vec<u8>) -> Option<Glyph> {
    const ON_CURVE: u8 = 0x01;
    const X_SHORT: u8 = 0x02;
    const Y_SHORT: u8 = 0x04;
    const X_SAME_OR_POSITIVE: u8 = 0x10;
    const Y_SAME_OR_POSITIVE: u8 = 0x20;

    let round = |v: f32| i16::try_from(v.round() as i32).ok();
    let mut points = Vec::new();
    let mut ends = Vec::new();
    for contour in contours {
        for p in contour {
            points.push((round(p.x)?, round(p.y)?, p.on));
        }
        ends.push(u16::try_from(points.len() - 1).ok()?);
    }
    let x_min = points.iter().map(|p| p.0).min()?;
    let y_min = points.iter().map(|p| p.1).min()?;
    let x_max = points.iter().map(|p| p.0).max()?;
    let y_max = points.iter().map(|p| p.1).max()?;

    out.extend_from_slice(&i16::try_from(contours.len()).ok()?.to_be_bytes());
    for value in [x_min, y_min, x_max, y_max] {
        out.extend_from_slice(&value.to_be_bytes());
    }
    for end in &ends {
        out.extend_from_slice(&end.to_be_bytes());
    }
    out.extend_from_slice(&0u16.to_be_bytes()); // no instructions

    // A coordinate delta: its flag bits and bytes.
    let delta = |d: i32, short: u8, same: u8| -> (u8, Vec<u8>) {
        if d == 0 {
            (same, Vec::new())
        } else if d.abs() < 256 {
            (
                short | if d > 0 { same } else { 0 },
                vec![d.unsigned_abs() as u8],
            )
        } else {
            (0, (d as i16).to_be_bytes().to_vec())
        }
    };
    let (mut flags, mut xs, mut ys) = (Vec::new(), Vec::new(), Vec::new());
    let (mut x, mut y) = (0i32, 0i32);
    for &(px, py, on) in &points {
        let (fx, bx) = delta(i32::from(px) - x, X_SHORT, X_SAME_OR_POSITIVE);
        let (fy, by) = delta(i32::from(py) - y, Y_SHORT, Y_SAME_OR_POSITIVE);
        flags.push(fx | fy | if on { ON_CURVE } else { 0 });
        xs.extend(bx);
        ys.extend(by);
        (x, y) = (i32::from(px), i32::from(py));
    }
    out.extend(flags);
    out.extend(xs);
    out.extend(ys);
    Some(Glyph {
        points: u16::try_from(points.len()).ok()?,
        contours: u16::try_from(contours.len()).ok()?,
        bbox: [x_min, y_min, x_max, y_max],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn libertinus_regular() -> Vec<u8> {
        typst_kit::fonts::embedded()
            .find(|(_, info)| {
                info.family == "Libertinus Serif"
                    && info.variant.weight.to_number() == 400
                    && format!("{:?}", info.variant.style) == "Normal"
            })
            .map(|(font, _)| font.data().to_vec())
            .expect("Typst embeds Libertinus Serif")
    }

    /// Signed area of a glyph's outline, curves flattened (positive when
    /// counter-clockwise).
    fn signed_area(face: &ttf_parser::Face, id: GlyphId) -> f32 {
        #[derive(Default)]
        struct Area {
            sum: f32,
            start: (f32, f32),
            last: (f32, f32),
        }
        impl OutlineBuilder for Area {
            fn move_to(&mut self, x: f32, y: f32) {
                self.start = (x, y);
                self.last = (x, y);
            }
            fn line_to(&mut self, x: f32, y: f32) {
                self.sum += (self.last.0 * y - x * self.last.1) / 2.0;
                self.last = (x, y);
            }
            fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
                let p0 = self.last;
                for k in 1..=16 {
                    let t = k as f32 / 16.0;
                    let s = 1.0 - t;
                    let w = [s * s, 2.0 * s * t, t * t];
                    self.line_to(
                        w[0] * p0.0 + w[1] * x1 + w[2] * x,
                        w[0] * p0.1 + w[1] * y1 + w[2] * y,
                    );
                }
            }
            fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
                let c = [self.last, (x1, y1), (x2, y2), (x, y)];
                for k in 1..=16 {
                    let end = piece(c, 0.0, k as f32 / 16.0)[3];
                    self.line_to(end.0, end.1);
                }
            }
            fn close(&mut self) {
                self.line_to(self.start.0, self.start.1);
            }
        }
        let mut area = Area::default();
        face.outline_glyph(id, &mut area);
        area.sum
    }

    #[test]
    fn libertinus_becomes_truetype_with_the_same_glyphs() {
        let cff = libertinus_regular();
        assert!(is_cff(&cff));
        let ttf = from_cff(&cff).unwrap();
        assert!(!is_cff(&ttf));
        assert_eq!(checksum(&ttf), 0xB1B0_AFBA, "whole-font checksum");

        let before = ttf_parser::Face::parse(&cff, 0).unwrap();
        let after = ttf_parser::Face::parse(&ttf, 0).unwrap();
        assert!(after.tables().glyf.is_some());
        assert_eq!(after.number_of_glyphs(), before.number_of_glyphs());
        assert_eq!(after.units_per_em(), before.units_per_em());

        for c in ['g', 'o', 'Q', '&'] {
            let id = after.glyph_index(c).unwrap();
            assert_eq!(before.glyph_index(c), Some(id));
            assert_eq!(after.glyph_hor_advance(id), before.glyph_hor_advance(id));
            assert_eq!(
                after.glyph_hor_side_bearing(id),
                after.glyph_bounding_box(id).map(|b| b.x_min),
                "{c}: bearing at xMin"
            );
            let (b, a) = (
                before.glyph_bounding_box(id).unwrap(),
                after.glyph_bounding_box(id).unwrap(),
            );
            for (x, y) in [
                (b.x_min, a.x_min),
                (b.y_min, a.y_min),
                (b.x_max, a.x_max),
                (b.y_max, a.y_max),
            ] {
                assert!((x - y).abs() <= 2, "{c}: {b:?} vs {a:?}");
            }
            // Same shape, opposite direction (TrueType outer contours run
            // clockwise).
            let (sb, sa) = (signed_area(&before, id), signed_area(&after, id));
            assert!(sb > 0.0 && sa < 0.0, "{c}: areas {sb} {sa}");
            assert!((sb + sa).abs() / sb < 0.002, "{c}: areas {sb} {sa}");
        }
    }

    #[test]
    fn quadratics_stay_within_tolerance() {
        let cubic = [(0.0, 0.0), (0.0, 600.0), (600.0, 600.0), (600.0, 0.0)];
        let quads = quadratics(cubic);
        assert!(quads.len() > 1);
        let mut start = cubic[0];
        for (i, [c, end]) in quads.iter().enumerate() {
            for k in 0..=10 {
                let t = k as f32 / 10.0;
                let s = 1.0 - t;
                let q = (
                    s * s * start.0 + 2.0 * s * t * c.0 + t * t * end.0,
                    s * s * start.1 + 2.0 * s * t * c.1 + t * t * end.1,
                );
                let u = (i as f32 + t) / quads.len() as f32;
                let p = piece(cubic, 0.0, u)[3];
                assert!((q.0 - p.0).hypot(q.1 - p.1) <= TOLERANCE, "{q:?} vs {p:?}");
            }
            start = *end;
        }
    }
}
