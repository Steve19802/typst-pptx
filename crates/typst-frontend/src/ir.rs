//! Page frames → IR, one slide per page (plan §5).
//!
//! Text is grouped into one text box per paragraph with Typst's line breaks;
//! shapes, curves and pictures map to native objects. Equations are baked
//! into pictures (plan §5.3). Items that can't be mapped yet are counted in
//! [`Skipped`].

use std::collections::HashMap;

use pptx_ir::{
    Align, Color, Crop, Deck, Fidelity, FontRef, Geometry, Item, Line, Media, MediaFormat,
    Paragraph, PathCommand, Picture, Rect, Run, Shape, Slide, TextBox, emu, text,
};
use typst::foundations::Content;
use typst::introspection::Location;
use typst::layout::{Abs, Em, Frame, FrameItem, GroupItem, Point, Size, Transform};
use typst::syntax::Span;
use typst::text::{FontInstance, TextItem};
use typst::visualize::{
    Curve, CurveItem, ExchangeFormat, Geometry as TypstGeometry, Image, ImageKind, Paint,
    RasterFormat, Shape as TypstShape,
};
use typst_layout::PagedDocument;

use crate::walk::{self, Event};

/// Counts of frame items that couldn't be mapped, by reason.
pub type Skipped = HashMap<&'static str, usize>;

/// Resolution of PNG renderings (SVG fallbacks, converted rasters).
const PIXEL_PER_PT: f64 = 2.0;
/// Resolution of baked content, which is mostly small text.
const BAKED_PIXEL_PER_PT: f64 = 4.0;
/// Margin around baked content so antialiasing isn't cut off, in pt.
const BAKE_MARGIN: f64 = 1.0;
/// Elements exported as one baked picture.
const BAKED: [&str; 1] = ["equation"];

/// Tolerances in pt for grouping text into lines and paragraphs.
const SAME_BASELINE: f64 = 0.01;
const ADJACENT: f64 = 0.5;
const SAME_PITCH: f64 = 0.05;
const SAME_EDGE: f64 = 0.5;

pub fn deck(doc: &PagedDocument) -> (Deck, Skipped) {
    let mut ctx = DeckBuilder::default();
    let (width, height) = doc
        .pages()
        .first()
        .map(|page| (page.frame.width(), page.frame.height()))
        .unwrap_or_default();
    let slides = doc
        .pages()
        .iter()
        .map(|page| {
            let mut slide = ctx.page(&page.frame);
            slide.background = page.fill_or_transparent().as_ref().and_then(solid);
            slide
        })
        .collect();
    (
        Deck {
            width: emu(width.to_pt()),
            height: emu(height.to_pt()),
            slides,
            media: ctx.media,
        },
        ctx.skipped,
    )
}

#[derive(Default)]
struct DeckBuilder {
    fonts: FontNames,
    media: Vec<Media>,
    media_index: HashMap<u128, usize>,
    skipped: Skipped,
}

/// A text item reduced to what paragraph building needs.
struct Piece {
    baseline: Point,
    width: Abs,
    /// Advance of trailing whitespace; Typst keeps the space at a soft line
    /// break in the item, but it doesn't count for alignment.
    trailing: Abs,
    run: Run,
}

/// Paragraph grouping key: the enclosing `par` element, or for text outside
/// paragraphs, the innermost element within the same group.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TextKey {
    Par(Location),
    Loose(Option<Location>, usize),
}

/// A position in the slide's z-order: a finished item, or the paragraphs of
/// a text group, built once the page has been walked.
enum Slot {
    Item(Item),
    Text(TextKey),
    /// Reserved for the element being baked; filled when its end tag comes.
    Empty,
}

/// Leaf items of an element being baked, with their page transforms.
struct Bake<'a> {
    elem: &'a Content,
    slot: usize,
    leaves: Vec<(Transform, &'a FrameItem)>,
}

#[derive(Clone, Copy)]
enum Clip {
    None,
    Rect(Point, Point),
    Complex,
}

impl DeckBuilder {
    fn page(&mut self, frame: &Frame) -> Slide {
        let mut slots = Vec::new();
        let mut texts: HashMap<TextKey, Vec<Piece>> = HashMap::new();
        let mut clips = vec![Clip::None];
        let mut groups = vec![0_usize];
        let mut next_group = 1;
        let mut bake: Option<Bake> = None;

        walk::walk(frame, &mut |ctx, event| match event {
            Event::TagStart(elem) if bake.is_none() && BAKED.contains(&elem.elem().name()) => {
                bake = Some(Bake {
                    elem,
                    slot: slots.len(),
                    leaves: Vec::new(),
                });
                slots.push(Slot::Empty);
            }
            Event::TagEnd(elem)
                if bake
                    .as_ref()
                    .is_some_and(|b| b.elem.location() == elem.location()) =>
            {
                let done = bake.take().unwrap();
                if let Some(item) = self.baked(&done) {
                    slots[done.slot] = Slot::Item(item);
                }
            }
            Event::Item(item) if bake.is_some() => {
                bake.as_mut().unwrap().leaves.push((ctx.ts, item));
            }
            Event::GroupStart(group) => {
                let clip = clip(ctx.ts, group, *clips.last().unwrap());
                clips.push(clip);
                groups.push(next_group);
                next_group += 1;
            }
            Event::GroupEnd => {
                clips.pop();
                groups.pop();
            }
            Event::Item(item) => {
                let clip = *clips.last().unwrap();
                match item {
                    FrameItem::Text(text) => {
                        if !ctx.ts.is_only_translate() {
                            self.skip("text (transformed)");
                            return;
                        }
                        let Some(color) = solid(&text.fill) else {
                            self.skip("text (non-solid fill)");
                            return;
                        };
                        let key = text_key(ctx.tags, *groups.last().unwrap());
                        let pieces = texts.entry(key).or_default();
                        if pieces.is_empty() {
                            slots.push(Slot::Text(key));
                        }
                        pieces.push(self.piece(ctx.ts, text, color));
                    }
                    FrameItem::Shape(shape, _) => match self.shape(ctx.ts, shape, slots.len()) {
                        Some(item) => slots.push(Slot::Item(item)),
                        None => self.skip("shape (transformed or non-solid)"),
                    },
                    FrameItem::Image(image, size, _) => {
                        if let Some(item) = self.picture(ctx.ts, image, *size, clip, slots.len()) {
                            slots.push(Slot::Item(item));
                        }
                    }
                    FrameItem::Link(..) => {}
                    FrameItem::Group(_) | FrameItem::Tag(_) => unreachable!(),
                }
            }
            Event::TagStart(_) | Event::TagEnd(_) => {}
        });

        let mut items = Vec::new();
        for slot in slots {
            match slot {
                Slot::Item(item) => items.push(item),
                Slot::Empty => {}
                Slot::Text(key) => {
                    let pieces = texts.remove(&key).unwrap_or_default();
                    for paragraph in paragraphs(lines(pieces)) {
                        let name = format!("Text {}", items.len());
                        items.push(Item::TextBox(text_box(name, paragraph)));
                    }
                }
            }
        }
        Slide {
            background: None,
            items,
            notes: Vec::new(),
        }
    }

    fn skip(&mut self, reason: &'static str) {
        *self.skipped.entry(reason).or_default() += 1;
    }

    fn piece(&mut self, ts: Transform, text: &TextItem, color: Color) -> Piece {
        let trailing = text
            .glyphs
            .iter()
            .rev()
            .take_while(|glyph| text.text[glyph.range()].chars().all(char::is_whitespace))
            .map(|glyph| glyph.x_advance.at(text.size))
            .sum();
        Piece {
            baseline: walk::origin(ts),
            width: text.width(),
            trailing,
            run: Run {
                text: text.text.to_string(),
                font: self.fonts.get(&text.font),
                size: (text.size.to_pt() * 100.0).round() as i32,
                color,
                spacing: tracking(text),
            },
        }
    }

    /// One picture of the baked element's leaves, SVG with a PNG fallback.
    fn baked(&mut self, bake: &Bake) -> Option<Item> {
        let mut bounds: Option<(Point, Point)> = None;
        for (ts, item) in &bake.leaves {
            let Some((min, max)) = local_bounds(item) else {
                continue;
            };
            let corners = [min, max, Point::new(min.x, max.y), Point::new(max.x, min.y)]
                .map(|p| p.transform(*ts));
            let (cmin, cmax) = self::bounds(&corners);
            bounds = Some(match bounds {
                None => (cmin, cmax),
                Some((a, b)) => (
                    Point::new(a.x.min(cmin.x), a.y.min(cmin.y)),
                    Point::new(b.x.max(cmax.x), b.y.max(cmax.y)),
                ),
            });
        }
        let (min, max) = bounds?;
        let margin = Point::splat(Abs::pt(BAKE_MARGIN));
        let (min, max) = (min - margin, max + margin);
        let mut frame = Frame::soft(Size::new(max.x - min.x, max.y - min.y));
        for (ts, item) in &bake.leaves {
            let mut leaf = Frame::soft(Size::zero());
            leaf.push(Point::zero(), (*item).clone());
            let mut group = GroupItem::new(leaf);
            group.transform = Transform::translate(-min.x, -min.y).pre_concat(*ts);
            frame.push(Point::zero(), FrameItem::Group(group));
        }

        let svg = fallback::svg(&frame).into_bytes();
        let key = typst::utils::hash128(&svg);
        let media = match self.media_index.get(&key) {
            Some(&index) => index,
            None => {
                self.media.push(Media {
                    format: MediaFormat::Png,
                    data: fallback::png(&frame, BAKED_PIXEL_PER_PT),
                    fallback: None,
                });
                self.media.push(Media {
                    format: MediaFormat::Svg,
                    data: svg,
                    fallback: Some(self.media.len() - 1),
                });
                let index = self.media.len() - 1;
                self.media_index.insert(key, index);
                index
            }
        };
        let source = bake.elem.plain_text();
        Some(Item::Picture(Picture {
            name: format!("baked:{}", bake.elem.elem().name()),
            frame: rect(min, max),
            media,
            descr: (!source.is_empty()).then(|| source.to_string()),
            crop: None,
            fidelity: Fidelity::Baked,
        }))
    }

    fn shape(&mut self, ts: Transform, shape: &TypstShape, n: usize) -> Option<Item> {
        if !ts.is_only_translate() {
            return None;
        }
        let origin = walk::origin(ts);
        let fill = match &shape.fill {
            Some(paint) => Some(solid(paint)?),
            None => None,
        };
        let line = match &shape.stroke {
            Some(stroke) => Some(Line {
                color: solid(&stroke.paint)?,
                width: emu(stroke.thickness.to_pt()),
            }),
            None => None,
        };
        let (frame, geometry) = match &shape.geometry {
            TypstGeometry::Rect(size) => (
                rect(origin, origin + Point::new(size.x, size.y)),
                Geometry::Rect,
            ),
            TypstGeometry::Line(to) => (
                rect(origin, origin + *to),
                Geometry::Line {
                    flip_h: to.x < Abs::zero(),
                    flip_v: to.y < Abs::zero(),
                },
            ),
            TypstGeometry::Curve(curve) => curve_geometry(origin, curve)?,
        };
        Some(Item::Shape(Shape {
            name: format!("Shape {n}"),
            frame,
            geometry,
            fill,
            line,
            fidelity: Fidelity::Native,
        }))
    }

    fn picture(
        &mut self,
        ts: Transform,
        image: &Image,
        size: Size,
        clip: Clip,
        n: usize,
    ) -> Option<Item> {
        if !ts.is_only_translate() {
            self.skip("image (transformed)");
            return None;
        }
        let min = walk::origin(ts);
        let max = min + Point::new(size.x, size.y);
        let (visible_min, visible_max, fidelity) = match clip {
            Clip::None => (min, max, Fidelity::Native),
            Clip::Complex => (min, max, Fidelity::Approximate),
            Clip::Rect(cmin, cmax) => {
                let vmin = Point::new(min.x.max(cmin.x), min.y.max(cmin.y));
                let vmax = Point::new(max.x.min(cmax.x), max.y.min(cmax.y));
                if vmin.x >= vmax.x || vmin.y >= vmax.y {
                    return None;
                }
                (vmin, vmax, Fidelity::Native)
            }
        };
        let crop = (visible_min != min || visible_max != max).then(|| {
            let part = |cut: Abs, of: Abs| (cut / of * 100_000.0).round() as i32;
            Crop {
                l: part(visible_min.x - min.x, size.x),
                t: part(visible_min.y - min.y, size.y),
                r: part(max.x - visible_max.x, size.x),
                b: part(max.y - visible_max.y, size.y),
            }
        });
        let media = self.media(image, size);
        Some(Item::Picture(Picture {
            name: format!("Picture {n}"),
            frame: rect(visible_min, visible_max),
            media,
            descr: image.alt().map(Into::into),
            crop,
            fidelity,
        }))
    }

    /// Adds the image to the media list once; SVGs get a PNG fallback
    /// rendered at the size they are shown.
    fn media(&mut self, image: &Image, size: Size) -> usize {
        let key = typst::utils::hash128(image);
        if let Some(&index) = self.media_index.get(&key) {
            return index;
        }
        let render = || {
            let mut frame = Frame::soft(size);
            frame.push(
                Point::zero(),
                FrameItem::Image(image.clone(), size, Span::detached()),
            );
            fallback::png(&frame, PIXEL_PER_PT)
        };
        let media = match image.kind() {
            ImageKind::Raster(raster) => {
                let format = match raster.format() {
                    RasterFormat::Exchange(ExchangeFormat::Png) => Some(MediaFormat::Png),
                    RasterFormat::Exchange(ExchangeFormat::Jpg) => Some(MediaFormat::Jpeg),
                    RasterFormat::Exchange(ExchangeFormat::Gif) => Some(MediaFormat::Gif),
                    _ => None,
                };
                match format {
                    Some(format) => Media {
                        format,
                        data: raster.data().to_vec(),
                        fallback: None,
                    },
                    None => Media {
                        format: MediaFormat::Png,
                        data: render(),
                        fallback: None,
                    },
                }
            }
            ImageKind::Svg(svg) if uses_unsupported_svg(svg.data()) => Media {
                format: MediaFormat::Png,
                data: render(),
                fallback: None,
            },
            ImageKind::Svg(svg) => {
                self.media.push(Media {
                    format: MediaFormat::Png,
                    data: render(),
                    fallback: None,
                });
                Media {
                    format: MediaFormat::Svg,
                    data: svg.data().to_vec(),
                    fallback: Some(self.media.len() - 1),
                }
            }
            ImageKind::Pdf(_) => Media {
                format: MediaFormat::Png,
                data: render(),
                fallback: None,
            },
        };
        self.media.push(media);
        let index = self.media.len() - 1;
        self.media_index.insert(key, index);
        index
    }
}

/// Typst adds tracking to every glyph's advance. The median difference to
/// the font's own advance is the tracking; kerning only touches a few glyphs.
fn tracking(text: &TextItem) -> Option<i32> {
    let mut deltas: Vec<f64> = text
        .glyphs
        .iter()
        .filter_map(|glyph| {
            let natural = text.font.x_advance(glyph.id)?;
            Some((glyph.x_advance - natural).at(text.size).to_pt())
        })
        .collect();
    if deltas.len() < 2 {
        return None;
    }
    deltas.sort_by(f64::total_cmp);
    let median = deltas[deltas.len() / 2];
    (median.abs() >= 0.05).then(|| (median * 100.0).round() as i32)
}

/// Bounding box of a leaf item in its own coordinates.
fn local_bounds(item: &FrameItem) -> Option<(Point, Point)> {
    match item {
        FrameItem::Text(text) => {
            let ttf = text.font.ttf();
            let scale = |units: i16| Em::from_units(units, text.font.units_per_em()).at(text.size);
            let mut pen = Abs::zero();
            let mut min = Point::splat(Abs::inf());
            let mut max = Point::splat(-Abs::inf());
            for glyph in &text.glyphs {
                if let Some(bbox) = ttf.glyph_bounding_box(ttf_parser::GlyphId(glyph.id)) {
                    let x = pen + glyph.x_offset.at(text.size);
                    let y = -glyph.y_offset.at(text.size);
                    min.x = min.x.min(x + scale(bbox.x_min));
                    max.x = max.x.max(x + scale(bbox.x_max));
                    min.y = min.y.min(y - scale(bbox.y_max));
                    max.y = max.y.max(y - scale(bbox.y_min));
                }
                pen += glyph.x_advance.at(text.size);
            }
            (min.x <= max.x).then_some((min, max))
        }
        FrameItem::Shape(shape, _) => {
            let rect = shape.bbox(true);
            Some((rect.min, rect.max))
        }
        FrameItem::Image(_, size, _) => Some((Point::zero(), Point::new(size.x, size.y))),
        _ => None,
    }
}

/// SVG features Office's SVG renderer draws unreliably (and LibreOffice
/// too); such images are exported as PNG only.
fn uses_unsupported_svg(data: &[u8]) -> bool {
    let text = String::from_utf8_lossy(data);
    ["<pattern", "<filter", "<mask", "<foreignObject"]
        .iter()
        .any(|tag| text.contains(tag))
}

fn text_key(tags: &[&Content], group: usize) -> TextKey {
    let par = tags
        .iter()
        .rev()
        .find(|elem| elem.elem().name() == "par")
        .and_then(|elem| elem.location());
    match par {
        Some(location) => TextKey::Par(location),
        None => TextKey::Loose(tags.last().and_then(|elem| elem.location()), group),
    }
}

/// The group's clip intersected with the clip around it, in page coordinates.
fn clip(ts: Transform, group: &GroupItem, outer: Clip) -> Clip {
    let Some(curve) = &group.clip else {
        return outer;
    };
    let inner = match axis_rect(curve) {
        Some((min, max)) if ts.is_only_translate() => {
            let origin = walk::origin(ts);
            Clip::Rect(origin + min, origin + max)
        }
        _ => Clip::Complex,
    };
    match (outer, inner) {
        (Clip::None, inner) => inner,
        (Clip::Complex, _) | (_, Clip::Complex) => Clip::Complex,
        (Clip::Rect(amin, amax), Clip::Rect(bmin, bmax)) => Clip::Rect(
            Point::new(amin.x.max(bmin.x), amin.y.max(bmin.y)),
            Point::new(amax.x.min(bmax.x), amax.y.min(bmax.y)),
        ),
        (Clip::Rect(..), Clip::None) => outer,
    }
}

/// Corners of a curve that is an axis-aligned rectangle (move + 3 or 4
/// lines, optionally closed).
fn axis_rect(curve: &Curve) -> Option<(Point, Point)> {
    let mut points = Vec::new();
    for item in &curve.0 {
        match item {
            CurveItem::Move(p) if points.is_empty() => points.push(*p),
            CurveItem::Line(p) => points.push(*p),
            CurveItem::Close => {}
            _ => return None,
        }
    }
    if points.len() == 5 && points[4] == points[0] {
        points.pop();
    }
    if points.len() != 4 {
        return None;
    }
    let axis = (0..4).all(|i| {
        let (a, b) = (points[i], points[(i + 1) % 4]);
        a.x == b.x || a.y == b.y
    });
    axis.then(|| bounds(&points))
}

fn bounds(points: &[Point]) -> (Point, Point) {
    let mut min = points[0];
    let mut max = points[0];
    for p in points {
        min = Point::new(min.x.min(p.x), min.y.min(p.y));
        max = Point::new(max.x.max(p.x), max.y.max(p.y));
    }
    (min, max)
}

/// Recognises rectangles, ellipses and rounded rectangles; anything else
/// becomes a custom path.
fn curve_geometry(origin: Point, curve: &Curve) -> Option<(Rect, Geometry)> {
    let mut points = Vec::new();
    for item in &curve.0 {
        match item {
            CurveItem::Move(p) | CurveItem::Line(p) => points.push(*p),
            CurveItem::Cubic(a, b, c) => points.extend([*a, *b, *c]),
            CurveItem::Close => {}
        }
    }
    if points.is_empty() {
        return None;
    }
    let (min, max) = bounds(&points);
    let frame = rect(origin + min, origin + max);
    if axis_rect(curve).is_some() {
        return Some((frame, Geometry::Rect));
    }
    if is_ellipse(curve, min, max) {
        return Some((frame, Geometry::Ellipse));
    }
    if let Some(radius) = round_rect_radius(curve, min, max) {
        return Some((
            frame,
            Geometry::RoundRect {
                radius: emu(radius.to_pt()),
            },
        ));
    }
    let local = |p: &Point| (emu((p.x - min.x).to_pt()), emu((p.y - min.y).to_pt()));
    let commands = curve
        .0
        .iter()
        .map(|item| match item {
            CurveItem::Move(p) => {
                let (x, y) = local(p);
                PathCommand::Move { x, y }
            }
            CurveItem::Line(p) => {
                let (x, y) = local(p);
                PathCommand::Line { x, y }
            }
            CurveItem::Cubic(a, b, c) => PathCommand::Cubic {
                pts: [local(a), local(b), local(c)],
            },
            CurveItem::Close => PathCommand::Close,
        })
        .collect();
    Some((frame, Geometry::Custom { commands }))
}

fn near(a: Abs, b: Abs) -> bool {
    (a - b).to_pt().abs() < 0.01
}

/// Typst's `Curve::ellipse`: move + four cubics through the edge midpoints.
fn is_ellipse(curve: &Curve, min: Point, max: Point) -> bool {
    let items: Vec<_> = curve
        .0
        .iter()
        .filter(|item| !matches!(item, CurveItem::Close))
        .collect();
    if items.len() != 5 {
        return false;
    }
    let mid = Point::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0);
    let on_midpoint = |p: &Point| {
        (near(p.x, min.x) || near(p.x, max.x)) && near(p.y, mid.y)
            || (near(p.y, min.y) || near(p.y, max.y)) && near(p.x, mid.x)
    };
    match items[0] {
        CurveItem::Move(start) if on_midpoint(start) => items[1..]
            .iter()
            .all(|item| matches!(item, CurveItem::Cubic(_, _, end) if on_midpoint(end))),
        _ => false,
    }
}

/// Typst's rounded rectangles: axis-aligned lines joined by one cubic per
/// corner. Returns the radius when all four corners have the same one.
fn round_rect_radius(curve: &Curve, min: Point, max: Point) -> Option<Abs> {
    let mut current = None;
    let mut radii = Vec::new();
    for item in &curve.0 {
        match item {
            CurveItem::Move(p) => {
                if current.is_some() {
                    return None;
                }
                current = Some(*p);
            }
            CurveItem::Line(p) => {
                let from = current?;
                if !near(from.x, p.x) && !near(from.y, p.y) {
                    return None;
                }
                current = Some(*p);
            }
            CurveItem::Cubic(_, _, end) => {
                let from = current?;
                let dx = (end.x - from.x).abs();
                let dy = (end.y - from.y).abs();
                if !near(dx, dy) {
                    return None;
                }
                if dx.to_pt() > 0.01 {
                    radii.push(dx);
                }
                current = Some(*end);
            }
            CurveItem::Close => {}
        }
    }
    let radius = *radii.first()?;
    let fits = radius * 2.0 <= (max.x - min.x).min(max.y - min.y) + Abs::pt(0.01);
    (radii.len() == 4 && radii.iter().all(|&r| near(r, radius)) && fits).then_some(radius)
}

/// A run of adjacent text pieces on one baseline.
struct LineSeg {
    y: Abs,
    x0: Abs,
    /// Right edge without trailing whitespace.
    x1: Abs,
    /// Right edge including it, where the next adjacent piece starts.
    end: Abs,
    runs: Vec<Run>,
}

impl LineSeg {
    fn max_size(&self) -> f64 {
        self.runs.iter().map(|run| run.size).max().unwrap_or(0) as f64 / 100.0
    }
}

/// Splits pieces (in walk order) into line segments: a new segment starts
/// on another baseline or after a horizontal gap.
fn lines(pieces: Vec<Piece>) -> Vec<LineSeg> {
    let mut lines: Vec<LineSeg> = Vec::new();
    for piece in pieces {
        let x = piece.baseline.x;
        if let Some(line) = lines.last_mut()
            && (line.y - piece.baseline.y).to_pt().abs() < SAME_BASELINE
            && (x - line.end).to_pt().abs() < ADJACENT
        {
            line.end = x + piece.width;
            line.x1 = line.end - piece.trailing;
            match line.runs.last_mut() {
                Some(last)
                    if last.font == piece.run.font
                        && last.size == piece.run.size
                        && last.color == piece.run.color
                        && last.spacing == piece.run.spacing =>
                {
                    last.text.push_str(&piece.run.text)
                }
                _ => line.runs.push(piece.run),
            }
            continue;
        }
        lines.push(LineSeg {
            y: piece.baseline.y,
            x0: x,
            x1: x + piece.width - piece.trailing,
            end: x + piece.width,
            runs: vec![piece.run],
        });
    }
    lines.retain(|line| line.runs.iter().any(|run| !run.text.trim().is_empty()));
    for line in &mut lines {
        while let Some(last) = line.runs.last_mut() {
            let trimmed = last.text.trim_end().len();
            last.text.truncate(trimmed);
            if !last.text.is_empty() {
                break;
            }
            line.runs.pop();
        }
    }
    lines
}

struct ParagraphBox {
    lines: Vec<LineSeg>,
    align: Align,
    pitch: Option<Abs>,
}

/// Chains consecutive lines into paragraphs while the baseline distance and
/// a common alignment edge hold.
fn paragraphs(lines: Vec<LineSeg>) -> Vec<ParagraphBox> {
    let mut out: Vec<ParagraphBox> = Vec::new();
    // Alignments still possible for the open paragraph: left, center, right.
    let mut possible = [true; 3];
    for line in lines {
        if let Some(open) = out.last_mut() {
            let prev = open.lines.last().unwrap();
            let pitch = line.y - prev.y;
            let first = &open.lines[0];
            let pitch_ok = pitch.to_pt() > 0.0
                && open
                    .pitch
                    .is_none_or(|p| (p - pitch).to_pt().abs() < SAME_PITCH);
            let edges = [
                (line.x0 - first.x0).to_pt().abs() < SAME_EDGE,
                ((line.x0 + line.x1) / 2.0 - (first.x0 + first.x1) / 2.0)
                    .to_pt()
                    .abs()
                    < SAME_EDGE,
                (line.x1 - first.x1).to_pt().abs() < SAME_EDGE,
            ];
            let next: [bool; 3] = std::array::from_fn(|i| possible[i] && edges[i]);
            if pitch_ok && next.iter().any(|&p| p) {
                possible = next;
                open.pitch = Some(pitch);
                open.align = [Align::Left, Align::Center, Align::Right]
                    [next.iter().position(|&p| p).unwrap()];
                open.lines.push(line);
                continue;
            }
        }
        possible = [true; 3];
        out.push(ParagraphBox {
            lines: vec![line],
            align: Align::Left,
            pitch: None,
        });
    }
    out
}

fn text_box(name: String, paragraph: ParagraphBox) -> TextBox {
    let first = &paragraph.lines[0];
    let size = first.max_size();
    let spacing = paragraph.pitch.map(|pitch| pitch.to_pt());
    let x0 = paragraph
        .lines
        .iter()
        .map(|l| l.x0)
        .fold(first.x0, Abs::min);
    let x1 = paragraph
        .lines
        .iter()
        .map(|l| l.x1)
        .fold(first.x1, Abs::max);
    let top = first.y.to_pt() - text::first_baseline(size, spacing);
    let height = text::box_height(size, spacing, paragraph.lines.len());
    let x = emu(x0.to_pt());
    let y = emu(top);
    TextBox {
        name,
        frame: Rect {
            x,
            y,
            w: emu(x1.to_pt()) - x,
            h: emu(top + height) - y,
        },
        paragraphs: vec![Paragraph {
            align: paragraph.align,
            line_spacing: spacing.map(|s| (s * 100.0).round() as i32),
            lines: paragraph.lines.into_iter().map(|line| line.runs).collect(),
        }],
        fidelity: Fidelity::Native,
    }
}

/// The box spanned by two corners, in any order.
fn rect(a: Point, b: Point) -> Rect {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    let x = emu(x0.to_pt());
    let y = emu(y0.to_pt());
    Rect {
        x,
        y,
        w: emu(x1.to_pt()) - x,
        h: emu(y1.to_pt()) - y,
    }
}

fn solid(paint: &Paint) -> Option<Color> {
    let Paint::Solid(color) = paint else {
        return None;
    };
    // `to_hex` converts any colour space to sRGB: #rrggbb or #rrggbbaa.
    let hex = color.to_hex();
    let byte = |i: usize| u8::from_str_radix(hex.get(1 + 2 * i..3 + 2 * i)?, 16).ok();
    Some(Color {
        r: byte(0)?,
        g: byte(1)?,
        b: byte(2)?,
        a: byte(3).unwrap_or(255),
    })
}

/// PowerPoint names a face by its legacy family (name ID 1) plus bold/italic
/// flags from the subfamily (name ID 2), plan §5.1.
#[derive(Default)]
struct FontNames(HashMap<(Option<String>, u32), FontRef>);

impl FontNames {
    fn get(&mut self, font: &FontInstance) -> FontRef {
        let key = (font.font().post_script_name(), font.font().index());
        self.0
            .entry(key)
            .or_insert_with(|| {
                let ttf = font.ttf();
                let name = |id: u16| {
                    ttf.names()
                        .into_iter()
                        .filter(|name| name.name_id == id)
                        .find_map(|name| name.to_string())
                };
                let family = name(ttf_parser::name_id::FAMILY)
                    .unwrap_or_else(|| font.font().info().family.clone());
                let subfamily = name(ttf_parser::name_id::SUBFAMILY).unwrap_or_default();
                FontRef {
                    family,
                    bold: subfamily.contains("Bold"),
                    italic: subfamily.contains("Italic") || subfamily.contains("Oblique"),
                }
            })
            .clone()
    }
}
