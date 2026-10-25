//! Page frames → IR, one slide per page (plan §5).
//!
//! Text is grouped into one text box per paragraph with Typst's line breaks;
//! shapes, curves and pictures map to native objects. Equations are baked
//! into pictures (plan §5.3). The fonts of the text boxes are embedded where
//! their licence and format allow (plan §5.1). What couldn't be mapped or
//! embedded is listed in the [`Report`].

use std::collections::{BTreeMap, HashMap};

pub use pptx_ir::text::TextModel;
use pptx_ir::{
    Align, Color, Crop, Deck, EmbeddedFont, Fidelity, Fill, FontRef, Geometry, Group, Item, Line,
    LineCap, LineJoin, Link, Media, MediaFormat, Paragraph, PathCommand, Picture, Rect, Role, Run,
    Shape, Slide, Stop, TextBox, emu, text,
};
use typst::foundations::Content;
use typst::introspection::{Location, MetadataElem};
use typst::layout::Ratio;
use typst::layout::{Abs, Em, Frame, FrameItem, GroupItem, Point, Size, Transform};
use typst::model::Destination;
use typst::syntax::Span;
use typst::text::{Font, FontInstance, TextItem};
use typst::visualize::{
    Color as TypstColor, Curve, CurveItem, ExchangeFormat, FixedStroke, Geometry as TypstGeometry,
    Gradient, Image, ImageKind, LineCap as TypstLineCap, LineJoin as TypstLineJoin, Paint,
    RasterFormat, RatioOrAngle, RelativeTo, Shape as TypstShape, SvgImage,
};
use typst_layout::PagedDocument;

use crate::truetype;
use crate::walk::{self, Event};

mod list;
mod table;

pub use table::{TableFallback, TableMode};

/// What the mapping couldn't do as asked, with source spans for warnings.
#[derive(Debug, Default)]
pub struct Report {
    /// Items exported as pictures (plan §5.8).
    pub baked: Vec<Baked>,
    /// Paragraphs exported as one text box per line because PowerPoint
    /// would draw a line at another width.
    pub line_mode: Vec<Located>,
    /// Embedded faces, as "Family Bold Italic".
    pub fonts_embedded: Vec<String>,
    /// Tables exported as `a:tbl`.
    pub tables: usize,
    /// Lists exported with native bullets and numbers.
    pub lists: usize,
    /// Lists exported as plain text boxes (markers as text).
    pub list_fallbacks: Vec<Located>,
    /// Tables exported as shapes and text boxes instead, with the reason.
    pub table_fallbacks: Vec<TableFallback>,
    /// Faces left out, with the reason.
    pub fonts_not_embedded: Vec<(String, &'static str)>,
}

#[derive(Debug, Clone)]
pub struct Baked {
    pub at: Located,
    /// The baked element (e.g. "equation") or why the item couldn't map.
    pub reason: &'static str,
}

/// A source position on a page (0-based).
#[derive(Debug, Clone, Copy)]
pub struct Located {
    pub page: usize,
    pub span: Span,
}

impl Report {
    /// Baked items counted by reason.
    pub fn baked_counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for baked in &self.baked {
            *counts.entry(baked.reason).or_default() += 1;
        }
        counts
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub text_model: TextModel,
    pub embed_fonts: bool,
    /// `--text flow`: paragraphs wrap in PowerPoint instead of keeping
    /// Typst's line breaks.
    pub flow: bool,
    /// Convert simple SVG images to native shapes with at most this many
    /// nodes; 0 keeps every SVG a picture (plan §5.5).
    pub svg_max_nodes: usize,
    pub tables: TableMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            text_model: TextModel::default(),
            embed_fonts: true,
            flow: false,
            svg_max_nodes: 60,
            tables: TableMode::Auto,
        }
    }
}

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
/// Smallest tracking or justification worth a `spc` attribute, in pt.
const MIN_SPACING: f64 = 0.05;
/// Width drift that sends a paragraph to line mode: the larger of these.
const MAX_DRIFT: f64 = 1.0;
const MAX_DRIFT_RATIO: f64 = 0.005;

pub fn deck(doc: &PagedDocument, options: Options) -> (Deck, Report) {
    let mut ctx = DeckBuilder {
        model: options.text_model,
        svg_max_nodes: options.svg_max_nodes,
        flow: options.flow,
        tables: options.tables,
        ..DeckBuilder::default()
    };
    let (width, height) = doc
        .pages()
        .first()
        .map(|page| (page.frame.width(), page.frame.height()))
        .unwrap_or_default();
    let slides = doc
        .pages()
        .iter()
        .enumerate()
        .map(|(index, page)| {
            ctx.page_index = index;
            ctx.links = links(&page.frame, doc);
            let mut slide = ctx.page(&page.frame);
            slide.page = index;
            slide.background = page.fill_or_transparent().as_ref().and_then(solid);
            slide
        })
        .collect();
    let fonts = if options.embed_fonts {
        ctx.fonts.embeddable(&mut ctx.report)
    } else {
        Vec::new()
    };
    (
        Deck {
            width: emu(width.to_pt()),
            height: emu(height.to_pt()),
            slides,
            media: ctx.media,
            fonts,
            layouts: Vec::new(),
            theme: None,
        },
        ctx.report,
    )
}

#[derive(Default)]
struct DeckBuilder {
    model: TextModel,
    /// The page being mapped, for report entries.
    page_index: usize,
    svg_max_nodes: usize,
    flow: bool,
    tables: TableMode,
    /// Link areas of the page being mapped, in page coordinates.
    links: Vec<(Point, Point, Link)>,
    fonts: FontNames,
    media: Vec<Media>,
    media_index: HashMap<u128, usize>,
    report: Report,
}

/// A text item reduced to what paragraph building needs.
#[derive(Clone)]
struct Piece {
    baseline: Point,
    width: Abs,
    /// Advance of trailing whitespace; Typst keeps the space at a soft line
    /// break in the item, but it doesn't count for alignment.
    trailing: Abs,
    runs: Vec<Run>,
    /// Source of the first glyph.
    span: Span,
    /// Width PowerPoint won't reproduce (ligatures and other shaping that
    /// changes advances), estimated from the font's own advances.
    drift: Abs,
    /// How far the last glyph hangs past the aligned edge if this item
    /// ends its line (see [`overhang`]).
    hang: Abs,
    /// Advance of the font's space at this size.
    space: Option<Abs>,
    /// Paragraph key without lists, for mapping list text as plain text.
    key: TextKey,
    list: Option<list::ListRole>,
    /// The `pptx-hints` role open around the text.
    role: Option<Role>,
}

/// A `pptx-hints` marker (plan §7): `#metadata((kind: ..)) <pptx-role-start>`
/// before a role's content, `#metadata(..) <pptx-role-end>` after it. Plain
/// metadata leaves the layout alone, unlike a labelled box or block.
enum RoleMarker {
    /// Opens a role; `None` for a kind the exporter doesn't know.
    Start(Option<Role>),
    End,
}

fn role_marker(elem: &Content) -> Option<RoleMarker> {
    let label = elem.label()?.resolve();
    let metadata = elem.to_packed::<MetadataElem>()?;
    match label.as_str() {
        "pptx-role-start" => {
            let value = serde_json::to_value(&metadata.value).ok()?;
            let kind = value.get("kind").and_then(|k| k.as_str());
            Some(RoleMarker::Start(kind.and_then(Role::from_name)))
        }
        "pptx-role-end" => Some(RoleMarker::End),
        _ => None,
    }
}

/// Paragraph grouping key: the enclosing `par` element, or for text outside
/// paragraphs, the innermost element that isn't [`INLINE`] (or else the
/// group).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TextKey {
    Par(Location),
    Loose(Option<Location>, usize),
    /// All text of an outermost list or enum.
    List(Location),
    /// All text of a `pptx-hints` role on the page: Touying's "n / N" is
    /// two elements, one placeholder.
    Role(Role),
}

/// A position in the slide's z-order: a finished item, or the paragraphs of
/// a text group, built once the page has been walked.
enum Slot {
    Item(Item),
    /// Text of one paragraph key at one rotation (see [`angle_key`]).
    Text((TextKey, i64)),
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
        let mut texts: HashMap<(TextKey, i64), Vec<Piece>> = HashMap::new();
        let mut clips = vec![Clip::None];
        let mut groups = vec![0_usize];
        let mut next_group = 1;
        let mut bake: Option<Bake> = None;
        let mut native_tables = self.tables(frame);
        let mut list_items = list::ListItems::default();
        // The native table whose own items are being skipped.
        let mut in_table: Option<Location> = None;
        let mut role: Option<Role> = None;

        walk::walk(frame, &mut |ctx, event| match event {
            Event::TagStart(elem)
                if in_table.is_none()
                    && bake.is_none()
                    && elem
                        .location()
                        .is_some_and(|loc| native_tables.contains_key(&loc)) =>
            {
                let location = elem.location().unwrap();
                slots.push(Slot::Item(native_tables.remove(&location).unwrap()));
                in_table = Some(location);
            }
            Event::TagEnd(elem) if in_table.is_some() && elem.location() == in_table => {
                in_table = None;
            }
            Event::Item(_) | Event::TagStart(_) | Event::TagEnd(_) if in_table.is_some() => {}
            Event::TagStart(elem) if role_marker(elem).is_some() => {
                role = match role_marker(elem) {
                    Some(RoleMarker::Start(kind)) => kind,
                    _ => None,
                };
            }
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
                let name = done.elem.elem().name();
                self.report.baked.push(Baked {
                    at: self.at(done.elem.span()),
                    reason: name,
                });
                let source = done.elem.plain_text();
                let descr = (!source.is_empty()).then(|| source.to_string());
                if let Some(item) = self.bake(&done.leaves, format!("baked:{name}"), descr) {
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
                        let color = solid(&text.fill);
                        let rigid = rigid(ctx.ts);
                        if rigid.is_none() || color.is_none() {
                            let reason = if color.is_none() {
                                "text with gradient or tiling fill"
                            } else {
                                "scaled, skewed or mirrored text"
                            };
                            slots.extend(self.bake_leaf(ctx.ts, item, reason).map(Slot::Item));
                            return;
                        }
                        let color = color.unwrap();
                        let (angle, local) = rigid.unwrap();
                        let plain = text_key(ctx.tags, *groups.last().unwrap());
                        let mut piece = self.piece(ctx.ts, local, text, color);
                        piece.key = plain;
                        piece.role = role;
                        let mut key = (plain, angle_key(angle));
                        if let Some(role) = role {
                            key.0 = TextKey::Role(role);
                        }
                        if let Some((root, role)) = list_items.role(ctx.tags) {
                            key.0 = TextKey::List(root);
                            piece.list = role;
                        }
                        let pieces = texts.entry(key).or_default();
                        if pieces.is_empty() {
                            slots.push(Slot::Text(key));
                        }
                        pieces.push(piece);
                    }
                    FrameItem::Shape(shape, _) => match self.shape(ctx.ts, shape, slots.len()) {
                        Some(mapped) => slots.push(Slot::Item(mapped)),
                        None => {
                            let reason = "transformed shape or unsupported paint";
                            slots.extend(self.bake_leaf(ctx.ts, item, reason).map(Slot::Item));
                        }
                    },
                    FrameItem::Image(_, size, _) if !rotatable(ctx.ts, *size, clip) => {
                        let reason = "scaled, skewed, mirrored or clipped rotated image";
                        slots.extend(self.bake_leaf(ctx.ts, item, reason).map(Slot::Item));
                    }
                    FrameItem::Image(image, size, _) => {
                        if let Some(item) = self.picture(ctx.ts, image, *size, clip, slots.len()) {
                            slots.push(Slot::Item(item));
                        }
                    }
                    FrameItem::Link(..) => {}
                    FrameItem::Group(_) | FrameItem::Tag(_) => unreachable!(),
                }
            }
            Event::TagStart(elem) => list_items.tag_start(ctx.tags, elem),
            Event::TagEnd(_) => {}
        });

        let mut items = Vec::new();
        for slot in slots {
            match slot {
                Slot::Item(item) => items.push(item),
                Slot::Empty => {}
                Slot::Text(key) => {
                    let pieces = texts.remove(&key).unwrap_or_default();
                    let angle = key.1 as f64 / ANGLE_STEPS;
                    if !matches!(key.0, TextKey::List(_)) {
                        self.text_items(pieces, angle, &mut items);
                        continue;
                    }
                    let name = format!("Text {}", items.len());
                    if !self.flow
                        && let Some(mut text_box) = list::list_box(name, pieces.clone(), self.model)
                    {
                        text_box.frame = rotate(text_box.frame, angle);
                        items.push(Item::TextBox(text_box));
                        self.report.lists += 1;
                        continue;
                    }
                    // Not a native list: the pieces as plain text.
                    if let Some(first) = pieces.first() {
                        let at = self.at(first.span);
                        self.report.list_fallbacks.push(at);
                    }
                    let mut groups: Vec<(TextKey, Vec<Piece>)> = Vec::new();
                    for piece in pieces {
                        match groups.iter_mut().find(|(k, _)| *k == piece.key) {
                            Some((_, group)) => group.push(piece),
                            None => groups.push((piece.key, vec![piece])),
                        }
                    }
                    for (_, group) in groups {
                        self.text_items(group, angle, &mut items);
                    }
                }
            }
        }
        Slide {
            page: 0,
            pages: Vec::new(),
            layout: None,
            background: None,
            items,
            notes: Vec::new(),
            clicks: Vec::new(),
            transition: None,
        }
    }

    /// Text boxes for the pieces of one paragraph key.
    fn text_items(&mut self, pieces: Vec<Piece>, angle: f64, items: &mut Vec<Item>) {
        // Text with different roles never shares a box (a title-slide stack
        // holds title, subtitle and author under one key).
        if pieces.iter().any(|p| p.role != pieces[0].role) {
            let mut groups: Vec<(Option<Role>, Vec<Piece>)> = Vec::new();
            for piece in pieces {
                match groups.iter_mut().find(|(role, _)| *role == piece.role) {
                    Some((_, group)) => group.push(piece),
                    None => groups.push((piece.role, vec![piece])),
                }
            }
            for (_, group) in groups {
                self.text_items(group, angle, items);
            }
            return;
        }
        let role = pieces.first().and_then(|p| p.role);
        let (boxes, split) = line_mode(paragraphs(lines(pieces), self.model));
        for span in split {
            let at = self.at(span);
            self.report.line_mode.push(at);
        }
        for paragraph in boxes {
            let name = format!("Text {}", items.len());
            let mut text_box = text_box(name, paragraph, self.model, self.flow, false);
            text_box.frame = rotate(text_box.frame, angle);
            text_box.role = role;
            items.push(Item::TextBox(text_box));
        }
    }

    fn link_at(&self, point: Point) -> Option<Link> {
        self.links
            .iter()
            .rev()
            .find(|(min, max, _)| {
                point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
            })
            .map(|(_, _, link)| link.clone())
    }

    fn at(&self, span: Span) -> Located {
        Located {
            page: self.page_index,
            span,
        }
    }

    /// Bakes one leaf item the mapping can't express (plan §5.8).
    fn bake_leaf(&mut self, ts: Transform, item: &FrameItem, reason: &'static str) -> Option<Item> {
        let at = self.at(item_span(item));
        self.report.baked.push(Baked { at, reason });
        self.bake(&[(ts, item)], "baked:item".into(), None)
    }

    /// Splits a text item into runs: tracking applies to every character,
    /// justification only to spaces, which then get their own runs (plan
    /// §5.2). Advances are compared per cluster (glyphs sharing a text
    /// range) with the font's own.
    /// `ts` places the text on the page, `local` in its unrotated frame.
    fn piece(&mut self, ts: Transform, local: Point, text: &TextItem, color: Color) -> Piece {
        let trailing = text
            .glyphs
            .iter()
            .rev()
            .take_while(|glyph| text.text[glyph.range()].chars().all(char::is_whitespace))
            .map(|glyph| glyph.x_advance.at(text.size))
            .sum();
        // The link whose area holds the middle of the text, a bit above
        // the baseline.
        let probe = Point::new(text.width() / 2.0, -text.size * 0.3).transform(ts);
        let base = Run {
            text: String::new(),
            font: self.fonts.get(&text.font),
            size: (text.size.to_pt() * 100.0).round() as i32,
            color,
            spacing: None,
            link: self.link_at(probe),
            field: None,
        };
        let clusters = clusters(text);
        let tracking = tracking(&clusters);
        let hundredths = |pt: f64| (pt.abs() >= MIN_SPACING).then(|| (pt * 100.0).round() as i32);

        let mut runs: Vec<Run> = Vec::new();
        let mut drift = 0.0;
        for cluster in &clusters {
            let extra = if cluster.space {
                (cluster.delta - tracking).max(0.0)
            } else {
                0.0
            };
            let spacing = hundredths(tracking + if extra >= MIN_SPACING { extra } else { 0.0 });
            drift += cluster.chars_natural - cluster.natural;
            match runs.last_mut() {
                Some(last) if last.spacing == spacing => last.text.push_str(cluster.text),
                _ => runs.push(Run {
                    text: cluster.text.to_string(),
                    spacing,
                    ..base.clone()
                }),
            }
        }
        let hang = text.glyphs.last().map_or(Abs::zero(), |glyph| {
            let c = text.text[glyph.range()].chars().next().unwrap_or(' ');
            glyph.x_advance.at(text.size) * overhang(c)
        });
        let space = text
            .font
            .ttf()
            .glyph_index(' ')
            .and_then(|id| text.font.x_advance(id.0).map(|em| em.at(text.size)));
        Piece {
            baseline: local,
            hang,
            space,
            key: TextKey::Loose(None, 0),
            list: None,
            role: None,
            width: text.width(),
            trailing,
            runs,
            span: item_span_text(text),
            drift: Abs::pt(drift),
        }
    }

    /// One picture of leaf items at their page transforms, SVG with a PNG
    /// fallback (plan §5.8).
    fn bake(
        &mut self,
        leaves: &[(Transform, &FrameItem)],
        name: String,
        descr: Option<String>,
    ) -> Option<Item> {
        let mut bounds: Option<(Point, Point)> = None;
        for (ts, item) in leaves {
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
        for (ts, item) in leaves {
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
                // Same rule as for SVG images: tilings and conic gradients
                // come out as <pattern>, which Office draws unreliably.
                if !uses_unsupported_svg(&svg) {
                    self.media.push(Media {
                        format: MediaFormat::Svg,
                        data: svg,
                        fallback: Some(self.media.len() - 1),
                    });
                }
                let index = self.media.len() - 1;
                self.media_index.insert(key, index);
                index
            }
        };
        Some(Item::Picture(Picture {
            name,
            frame: rect(min, max),
            media,
            descr,
            crop: None,
            link: None,
            fidelity: Fidelity::Baked,
        }))
    }

    /// Mapped in the shape's unrotated frame, then rotated as a whole
    /// (DrawingML flips before rotating and rotates gradients with the
    /// shape, like Typst).
    fn shape(&mut self, ts: Transform, shape: &TypstShape, n: usize) -> Option<Item> {
        let (angle, origin) = rigid(ts)?;
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
        let fill = match &shape.fill {
            Some(paint) => Some(fill(paint, frame)?),
            None => None,
        };
        let (line, fidelity) = match &shape.stroke {
            Some(stroke) => {
                let (line, exact) = line(stroke)?;
                let fidelity = if exact {
                    Fidelity::Native
                } else {
                    Fidelity::Normalized
                };
                (Some(line), fidelity)
            }
            None => (None, Fidelity::Native),
        };
        Some(Item::Shape(Shape {
            name: format!("Shape {n}"),
            frame: rotate(frame, angle),
            geometry,
            fill,
            line,
            fidelity,
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
        // Rotated pictures are unclipped (see `rotatable`).
        let (angle, min) = rigid(ts)?;
        let clip = if angle == 0.0 { clip } else { Clip::None };
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
        if fidelity == Fidelity::Native
            && angle == 0.0
            && let ImageKind::Svg(svg) = image.kind()
            && let Some(group) =
                self.native_svg(svg, min, size, (visible_min, visible_max), image.alt(), n)
        {
            return Some(group);
        }
        let media = self.media(image, size);
        let centre = Point::new(size.x / 2.0, size.y / 2.0).transform(ts);
        let link = self.link_at(centre);
        Some(Item::Picture(Picture {
            name: format!("Picture {n}"),
            frame: rotate(rect(visible_min, visible_max), angle),
            media,
            descr: image.alt().map(Into::into),
            crop,
            link,
            fidelity,
        }))
    }

    /// A simple SVG as a group of native shapes (plan §5.5). `visible` is
    /// the part left by a rectangular clip (Typst's `fit: "cover"`): shapes
    /// outside it are dropped, filled rectangles crossing it are cut; any
    /// other shape crossing it keeps the picture.
    fn native_svg(
        &mut self,
        svg: &SvgImage,
        origin: Point,
        size: Size,
        visible: (Point, Point),
        alt: Option<&str>,
        n: usize,
    ) -> Option<Item> {
        if self.svg_max_nodes == 0 || uses_unsupported_svg(svg.data()) {
            return None;
        }
        let paths = crate::svg::paths(svg, origin, size, self.svg_max_nodes)?;
        let clip = rect(visible.0, visible.1);
        let mut items = Vec::new();
        for (index, path) in paths.into_iter().enumerate() {
            let (mut frame, geometry) = curve_geometry(Point::zero(), &path.curve)?;
            let inside = frame.x >= clip.x
                && frame.y >= clip.y
                && frame.x + frame.w <= clip.x + clip.w
                && frame.y + frame.h <= clip.y + clip.h;
            if !inside {
                let x0 = frame.x.max(clip.x);
                let y0 = frame.y.max(clip.y);
                let x1 = (frame.x + frame.w).min(clip.x + clip.w);
                let y1 = (frame.y + frame.h).min(clip.y + clip.h);
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }
                if !matches!(geometry, Geometry::Rect) || path.line.is_some() {
                    return None;
                }
                frame = Rect {
                    x: x0,
                    y: y0,
                    w: x1 - x0,
                    h: y1 - y0,
                    rot: 0,
                };
            }
            items.push(Item::Shape(Shape {
                name: format!("Shape {n}.{index}"),
                frame,
                geometry,
                fill: path.fill,
                line: path.line,
                fidelity: path.fidelity,
            }));
        }
        if items.is_empty() {
            return None;
        }
        Some(Item::Group(Group {
            name: alt.map_or_else(|| format!("SVG {n}"), str::to_string),
            frame: clip,
            items,
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

/// Glyphs that share a text range, with their advances in pt.
struct Cluster<'a> {
    text: &'a str,
    space: bool,
    /// Typst's advance minus the font's own advance of these glyphs.
    delta: f64,
    /// The font's advance of these glyphs.
    natural: f64,
    /// The font's advance of the characters looked up one by one, which is
    /// what an application without this shaping draws.
    chars_natural: f64,
}

fn clusters(text: &TextItem) -> Vec<Cluster<'_>> {
    let ttf = text.font.ttf();
    let advance = |id: u16| {
        text.font
            .x_advance(id)
            .map_or(0.0, |em| em.at(text.size).to_pt())
    };
    let mut out: Vec<Cluster> = Vec::new();
    let mut last_range = None;
    for glyph in &text.glyphs {
        let actual = glyph.x_advance.at(text.size).to_pt();
        let natural = advance(glyph.id);
        if last_range == Some(glyph.range.clone()) {
            let cluster = out.last_mut().unwrap();
            cluster.delta += actual - natural;
            cluster.natural += natural;
            continue;
        }
        last_range = Some(glyph.range.clone());
        let chunk = &text.text[glyph.range()];
        let chars_natural = chunk
            .chars()
            .map(|c| ttf.glyph_index(c).map_or(natural, |id| advance(id.0)))
            .sum();
        out.push(Cluster {
            text: chunk,
            space: !chunk.is_empty() && chunk.chars().all(char::is_whitespace),
            delta: actual - natural,
            natural,
            chars_natural,
        });
    }
    out
}

/// Typst adds tracking to every glyph's advance. The median difference to
/// the font's own advance (spaces excluded, they also carry justification)
/// is the tracking; kerning only touches a few glyphs.
fn tracking(clusters: &[Cluster]) -> f64 {
    let mut deltas: Vec<f64> = clusters
        .iter()
        .filter(|c| !c.space)
        .map(|c| c.delta)
        .collect();
    if deltas.len() < 2 {
        return 0.0;
    }
    deltas.sort_by(f64::total_cmp);
    let median = deltas[deltas.len() / 2];
    if median.abs() >= MIN_SPACING {
        median
    } else {
        0.0
    }
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

/// The link areas of a page with their resolved targets.
fn links(frame: &Frame, doc: &PagedDocument) -> Vec<(Point, Point, Link)> {
    let mut out = Vec::new();
    walk::walk(frame, &mut |ctx, event| {
        let Event::Item(FrameItem::Link(destination, size)) = event else {
            return;
        };
        let link = match destination {
            Destination::Url(url) => Link::Url(url.to_string()),
            Destination::Position(position) => Link::Page(position.page.get() - 1),
            Destination::Location(location) => match doc.introspector().position(*location) {
                Some(position) => Link::Page(position.page.get() - 1),
                None => return,
            },
        };
        let corners = [
            Point::zero(),
            Point::new(size.x, Abs::zero()),
            Point::new(Abs::zero(), size.y),
            Point::new(size.x, size.y),
        ]
        .map(|p| p.transform(ctx.ts));
        let (min, max) = bounds(&corners);
        out.push((min, max, link));
    });
    out
}

fn item_span_text(text: &TextItem) -> Span {
    text.glyphs
        .first()
        .map_or_else(Span::detached, |glyph| glyph.span.0)
}

fn item_span(item: &FrameItem) -> Span {
    match item {
        FrameItem::Text(text) => item_span_text(text),
        FrameItem::Shape(_, span) | FrameItem::Image(_, _, span) => *span,
        _ => Span::detached(),
    }
}

/// Elements that sit inside a line; text keyed by them belongs with the
/// text around them.
const INLINE: &[&str] = &[
    "box",
    "link",
    "repeat",
    "strong",
    "emph",
    "text",
    "highlight",
    "underline",
    "overline",
    "strike",
    "sub",
    "super",
    "smallcaps",
    "h",
    "hide",
];

fn text_key(tags: &[&Content], group: usize) -> TextKey {
    let par = tags
        .iter()
        .rev()
        .find(|elem| elem.elem().name() == "par")
        .and_then(|elem| elem.location());
    if let Some(location) = par {
        return TextKey::Par(location);
    }
    let block = tags
        .iter()
        .rev()
        .find(|elem| !INLINE.contains(&elem.elem().name()))
        .and_then(|elem| elem.location());
    match block {
        Some(location) => TextKey::Loose(Some(location), 0),
        None => TextKey::Loose(None, group),
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
#[derive(Clone)]
struct LineSeg {
    y: Abs,
    x0: Abs,
    /// Right edge without trailing whitespace.
    x1: Abs,
    /// Right edge including it, where the next adjacent piece starts.
    end: Abs,
    /// Sum of the pieces' drift.
    drift: Abs,
    /// Hanging punctuation at the end (Typst's `overhang`, on by default).
    hang: Abs,
    /// The last piece's space advance, for bridging a gap after it.
    space: Option<Abs>,
    /// Rounding of bridged gaps so far, in pt (see [`bridge`]).
    carry: f64,
    /// Source of the first piece.
    span: Span,
    runs: Vec<Run>,
}

impl LineSeg {
    /// Whether PowerPoint would draw this line noticeably wider or narrower.
    fn drifts(&self) -> bool {
        let limit = MAX_DRIFT.max(MAX_DRIFT_RATIO * (self.x1 - self.x0).to_pt());
        self.drift.to_pt().abs() > limit
    }

    /// Right edge for alignment: Typst lets line-final punctuation hang
    /// past it.
    fn aligned_x1(&self) -> Abs {
        self.x1 - self.hang
    }

    fn max_size(&self) -> f64 {
        self.runs.iter().map(|run| run.size).max().unwrap_or(0) as f64 / 100.0
    }
}

/// Appends a run, merged into the last one if they look the same.
fn push_run(runs: &mut Vec<Run>, run: Run) {
    match runs.last_mut() {
        Some(last)
            if last.font == run.font
                && last.size == run.size
                && last.color == run.color
                && last.spacing == run.spacing
                && last.link == run.link =>
        {
            last.text.push_str(&run.text)
        }
        _ => runs.push(run),
    }
}

/// Bridges a horizontal gap after a line's text (Typst's `repeat` leaders,
/// `h()` spacing): a space whose spacing makes up the rest if the gap is at
/// least a space wide, else extra spacing after the last character.
/// Spacing is written in hundredths of a point; `carry` holds the rounding
/// so far (written minus exact), which the next gap makes up for.
fn bridge(runs: &mut Vec<Run>, gap: Abs, space: Option<Abs>, carry: &mut f64) {
    let Some(mut last) = runs.pop() else {
        return;
    };
    let mut written = |pt: f64| {
        let hundredths = ((pt - *carry) * 100.0).round() as i32;
        *carry += f64::from(hundredths) / 100.0 - pt;
        hundredths
    };
    match space {
        Some(space) if gap >= space => {
            let filler = Run {
                text: " ".into(),
                spacing: Some(written((gap - space).to_pt())).filter(|&s| s != 0),
                ..last.clone()
            };
            push_run(runs, last);
            push_run(runs, filler);
        }
        _ => {
            let at = last.text.char_indices().last().map_or(0, |(at, _)| at);
            let mut tail = last.clone();
            tail.text = last.text.split_off(at);
            tail.spacing = Some(tail.spacing.unwrap_or(0) + written(gap.to_pt()));
            if !last.text.is_empty() {
                push_run(runs, last);
            }
            push_run(runs, tail);
        }
    }
}

/// Splits pieces (in walk order) into line segments: a new segment starts
/// on another baseline or when text goes back left. Gaps within a line
/// (pieces share their paragraph key) are bridged with spacing.
fn lines(pieces: Vec<Piece>) -> Vec<LineSeg> {
    let mut lines: Vec<LineSeg> = Vec::new();
    for piece in pieces {
        let x = piece.baseline.x;
        if let Some(line) = lines.last_mut()
            && (line.y - piece.baseline.y).to_pt().abs() < SAME_BASELINE
            && (x - line.end).to_pt() > -ADJACENT
        {
            let gap = x - line.end;
            if gap.to_pt() >= ADJACENT {
                bridge(&mut line.runs, gap, line.space, &mut line.carry);
            }
            line.end = x + piece.width;
            line.x1 = line.end - piece.trailing;
            line.drift += piece.drift;
            line.hang = piece.hang;
            line.space = piece.space;
            for run in piece.runs {
                push_run(&mut line.runs, run);
            }
            continue;
        }
        lines.push(LineSeg {
            y: piece.baseline.y,
            x0: x,
            x1: x + piece.width - piece.trailing,
            end: x + piece.width,
            drift: piece.drift,
            hang: piece.hang,
            space: piece.space,
            carry: 0.0,
            span: piece.span,
            runs: piece.runs,
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

#[derive(Clone)]
struct ParagraphBox {
    lines: Vec<LineSeg>,
    align: Align,
    pitch: Option<Abs>,
    /// The lines align only without Typst's hanging punctuation, which
    /// PowerPoint doesn't do: each line becomes a paragraph placed by its
    /// indents (see [`text_box`]).
    hung: bool,
}

/// Typst's hanging punctuation (`typst-layout`, `inline/line.rs`): the part
/// of a line-final glyph's advance that may extend past the right edge.
fn overhang(c: char) -> f64 {
    match c {
        '–' | '—' => 0.2,
        '-' | '\u{ad}' => 0.55,
        '.' | ',' => 0.8,
        ':' | ';' => 0.3,
        '\u{60C}' | '\u{6D4}' => 0.4,
        _ => 0.0,
    }
}

/// Chains consecutive lines into paragraphs while the baseline distance and
/// a common alignment edge hold (and, if the model needs it, the size).
fn paragraphs(lines: Vec<LineSeg>, model: TextModel) -> Vec<ParagraphBox> {
    let mut out: Vec<ParagraphBox> = Vec::new();
    // Alignments still possible for the open paragraph: left, center,
    // right, then center and right with hanging punctuation.
    let mut possible = [true; 5];
    for line in lines {
        if let Some(open) = out.last_mut() {
            let prev = open.lines.last().unwrap();
            let pitch = line.y - prev.y;
            let first = &open.lines[0];
            let size_ok = model.mixed_line_sizes() || line.max_size() == first.max_size();
            let pitch_ok = size_ok
                && pitch.to_pt() > 0.0
                && open
                    .pitch
                    .is_none_or(|p| (p - pitch).to_pt().abs() < SAME_PITCH);
            let same = |a: Abs, b: Abs| (a - b).to_pt().abs() < SAME_EDGE;
            let (hung, first_hung) = (line.aligned_x1(), first.aligned_x1());
            let edges = [
                same(line.x0, first.x0),
                same((line.x0 + line.x1) / 2.0, (first.x0 + first.x1) / 2.0),
                same(line.x1, first.x1),
                same((line.x0 + hung) / 2.0, (first.x0 + first_hung) / 2.0),
                same(hung, first_hung),
            ];
            let next: [bool; 5] = std::array::from_fn(|i| possible[i] && edges[i]);
            if pitch_ok && next.iter().any(|&p| p) {
                possible = next;
                open.pitch = Some(pitch);
                open.align = [
                    Align::Left,
                    Align::Center,
                    Align::Right,
                    Align::Center,
                    Align::Right,
                ][next.iter().position(|&p| p).unwrap()];
                open.lines.push(line);
                continue;
            }
        }
        if let Some(open) = out.last_mut() {
            open.hung = open.lines.len() > 1 && !possible[..3].iter().any(|&p| p);
        }
        possible = [true; 5];
        out.push(ParagraphBox {
            lines: vec![line],
            align: Align::Left,
            pitch: None,
            hung: false,
        });
    }
    if let Some(open) = out.last_mut() {
        open.hung = open.lines.len() > 1 && !possible[..3].iter().any(|&p| p);
    }
    out
}

/// Splits paragraphs with a line PowerPoint would draw at another width
/// into one box per line, each at Typst's exact position (plan §5.2).
/// Returns the boxes and the source of each split paragraph.
fn line_mode(paragraphs: Vec<ParagraphBox>) -> (Vec<ParagraphBox>, Vec<Span>) {
    let mut out = Vec::new();
    let mut split = Vec::new();
    for paragraph in paragraphs {
        if paragraph.lines.len() < 2 || !paragraph.lines.iter().any(LineSeg::drifts) {
            out.push(paragraph);
            continue;
        }
        split.push(paragraph.lines[0].span);
        out.extend(paragraph.lines.into_iter().map(|line| ParagraphBox {
            lines: vec![line],
            align: Align::Left,
            pitch: None,
            hung: false,
        }));
    }
    (out, split)
}

/// Slack added to flowing text boxes so PowerPoint's wrapping, with its
/// slightly different line widths, breaks where Typst did.
const FLOW_SLACK: f64 = 2.0;

/// `cell`: the text goes into a table cell (see
/// [`TextModel::cell_line_spacing`]).
fn text_box(
    name: String,
    paragraph: ParagraphBox,
    model: TextModel,
    flow: bool,
    cell: bool,
) -> TextBox {
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
    // The application may draw another pitch than Typst's (PowerPoint
    // rounds it): the middle line is placed exactly, the error is split
    // between the first and last lines.
    let drawn = spacing.map(|s| {
        if cell {
            model.cell_line_spacing(s)
        } else {
            model.line_spacing(s)
        }
    });
    let middle = (paragraph.lines.len() - 1) as f64 / 2.0;
    let balance = middle * (drawn.unwrap_or(0.0) - spacing.unwrap_or(0.0));
    let top = first.y.to_pt() - model.first_baseline(size, drawn) - balance;
    let height = text::box_height(size, drawn, paragraph.lines.len());
    let x = emu(x0.to_pt());
    let y = emu(top);
    let wrap = flow && paragraph.lines.len() > 1;
    let slack = if wrap { FLOW_SLACK } else { 0.0 };
    let line_spacing = drawn.map(|s| (s * 100.0).round() as i32);
    let mut paragraphs = if paragraph.hung && !wrap {
        // One paragraph per line, indented so PowerPoint's alignment puts
        // the line where Typst did, punctuation and all.
        paragraph
            .lines
            .into_iter()
            .map(|line| {
                let (left, right) = match paragraph.align {
                    Align::Right => (Abs::zero(), x1 - line.x1),
                    Align::Center | Align::Left => {
                        let shift = (line.x0 + line.x1) - (x0 + x1);
                        (shift.max(Abs::zero()), (-shift).max(Abs::zero()))
                    }
                };
                Paragraph {
                    align: paragraph.align,
                    line_spacing,
                    anchor: Some((emu(line.x0.to_pt()), emu(line.y.to_pt()))),
                    lines: vec![line.runs],
                    indents: Some((emu(left.to_pt()), emu(right.to_pt()))),
                    list: None,
                }
            })
            .collect()
    } else {
        let anchor = Some((emu(first.x0.to_pt()), emu(first.y.to_pt())));
        let lines: Vec<Vec<Run>> = paragraph.lines.into_iter().map(|line| line.runs).collect();
        vec![Paragraph {
            align: paragraph.align,
            line_spacing,
            lines: if wrap { vec![join(lines)] } else { lines },
            indents: None,
            list: None,
            anchor,
        }]
    };
    TextBox {
        name,
        frame: Rect {
            x,
            y,
            w: emu(x1.to_pt() + slack) - x,
            h: emu(top + height) - y,
            rot: 0,
        },
        link: hoist_link(&mut paragraphs),
        paragraphs,
        wrap,
        fidelity: Fidelity::Native,
        role: None,
    }
}

/// The link all of a box's visible text shares, taken off the runs.
fn hoist_link(paragraphs: &mut [Paragraph]) -> Option<Link> {
    let mut visible = paragraphs
        .iter()
        .flat_map(|p| p.lines.iter().flatten())
        .filter(|run| !run.text.trim().is_empty());
    let link = visible.next()?.link.clone()?;
    if !visible.all(|run| run.link.as_ref() == Some(&link)) {
        return None;
    }
    for line in paragraphs.iter_mut().flat_map(|p| p.lines.iter_mut()) {
        for run in std::mem::take(line) {
            push_run(line, Run { link: None, ..run });
        }
    }
    Some(link)
}

/// Joins Typst's lines into one for `--text flow`: with a space, or
/// directly after a hyphen (a real one stays intact; a hyphenation one
/// stays visible).
fn join(lines: Vec<Vec<Run>>) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for line in lines {
        if let Some(last) = out.last_mut()
            && !last.text.ends_with(['-', '\u{2010}', '\u{00AD}'])
        {
            last.text.push(' ');
        }
        for run in line {
            match out.last_mut() {
                Some(last)
                    if last.font == run.font
                        && last.size == run.size
                        && last.color == run.color
                        && last.spacing == run.spacing
                        && last.link == run.link =>
                {
                    last.text.push_str(&run.text)
                }
                _ => out.push(run),
            }
        }
    }
    out
}

/// Stops sampled from a Typst gradient: Typst interpolates in its own colour
/// space (Oklab by default), DrawingML in RGB, so the gradient is sampled
/// densely instead of copying the stops.
const GRADIENT_SAMPLES: usize = 16;

/// A shape's paint as a DrawingML fill. Gradients relative to the parent,
/// conic and off-centre radial ones return `None` (baked).
fn fill(paint: &Paint, frame: Rect) -> Option<Fill> {
    let gradient = match paint {
        Paint::Solid(_) => {
            return Some(Fill::Solid {
                color: solid(paint)?,
            });
        }
        Paint::Gradient(gradient) => gradient,
        Paint::Tiling(_) => return None,
    };
    if gradient.unwrap_relative(false) != RelativeTo::Self_ {
        return None;
    }
    let sample = |t: f64| color(&gradient.sample(RatioOrAngle::Ratio(Ratio::new(t))));
    let stops = |scale: f64| -> Option<Vec<Stop>> {
        (0..=GRADIENT_SAMPLES)
            .map(|i| {
                let t = i as f64 / GRADIENT_SAMPLES as f64;
                Some(Stop {
                    pos: ((t * scale).min(1.0) * 100_000.0).round() as i32,
                    color: sample(t)?,
                })
            })
            .collect()
    };
    match gradient {
        Gradient::Linear(linear) => {
            let aspect = frame.w as f64 / frame.h.max(1) as f64;
            let angle = Gradient::correct_aspect_ratio(linear.angle, Ratio::new(aspect));
            Some(Fill::Linear {
                angle: angle.to_deg(),
                stops: stops(1.0)?,
            })
        }
        Gradient::Radial(radial) => {
            let centred = |axes: &typst::layout::Axes<Ratio>| {
                (axes.x.get() - 0.5).abs() < 1e-6 && (axes.y.get() - 0.5).abs() < 1e-6
            };
            if !centred(&radial.center)
                || !centred(&radial.focal_center)
                || radial.focal_radius.get() > 1e-6
            {
                return None;
            }
            // DrawingML's circle reaches the corners (√0.5 of the unit
            // square from the centre), Typst's radius is on the unit square.
            let mut stops = stops(radial.radius.get() / std::f64::consts::FRAC_1_SQRT_2)?;
            if let Some(last) = stops.last().copied()
                && last.pos < 100_000
            {
                stops.push(Stop {
                    pos: 100_000,
                    ..last
                });
            }
            Some(Fill::Radial { stops })
        }
        Gradient::Conic(_) => None,
    }
}

/// A stroke as a DrawingML line; `false` if the dash phase was dropped.
fn line(stroke: &FixedStroke) -> Option<(Line, bool)> {
    let width = stroke.thickness.to_pt();
    let mut exact = true;
    let dash = match &stroke.dash {
        Some(pattern) if !pattern.array.is_empty() && width > 0.0 => {
            exact = pattern.phase == Abs::zero();
            let mut lengths: Vec<i32> = pattern
                .array
                .iter()
                .map(|len| (len.to_pt() / width * 100_000.0).round().max(1.0) as i32)
                .collect();
            if lengths.len() % 2 == 1 {
                lengths.extend_from_within(..);
            }
            lengths
        }
        _ => Vec::new(),
    };
    Some((
        Line {
            color: solid(&stroke.paint)?,
            width: emu(width),
            cap: match stroke.cap {
                TypstLineCap::Butt => LineCap::Flat,
                TypstLineCap::Round => LineCap::Round,
                TypstLineCap::Square => LineCap::Square,
            },
            join: match stroke.join {
                TypstLineJoin::Miter => LineJoin::Miter {
                    limit: stroke.miter_limit.get(),
                },
                TypstLineJoin::Round => LineJoin::Round,
                TypstLineJoin::Bevel => LineJoin::Bevel,
            },
            dash,
        },
        exact,
    ))
}

/// The box spanned by two corners, in any order.
/// Rotations are compared in steps of 1/1000 degree.
const ANGLE_STEPS: f64 = 1000.0;

fn angle_key(degrees: f64) -> i64 {
    (degrees * ANGLE_STEPS).round() as i64
}

/// For a transform that only rotates and translates: the clockwise
/// rotation in degrees, and where the transform puts the origin in the
/// frame rotated back by that angle. `None` if it scales, skews or
/// mirrors.
fn rigid(ts: Transform) -> Option<(f64, Point)> {
    const EPS: f64 = 1e-6;
    let (sx, ky, kx, sy) = (ts.sx.get(), ts.ky.get(), ts.kx.get(), ts.sy.get());
    if (sx - sy).abs() > EPS || (kx + ky).abs() > EPS || (sx * sx + ky * ky - 1.0).abs() > EPS {
        return None;
    }
    if ky.abs() < EPS && sx > 0.0 {
        return Some((0.0, walk::origin(ts)));
    }
    let angle = ky.atan2(sx);
    // ts(p) = R(angle)·p + t, so in the frame rotated back: p + R(-angle)·t.
    let (sin, cos) = (-angle).sin_cos();
    let local = Point::new(ts.tx * cos - ts.ty * sin, ts.tx * sin + ts.ty * cos);
    Some((angle.to_degrees(), local))
}

/// Whether an image can be a rotated picture: rigid, and if rotated, not
/// cut by a clip (a crop can't follow the rotation).
fn rotatable(ts: Transform, size: Size, clip: Clip) -> bool {
    match rigid(ts) {
        None => false,
        Some((0.0, _)) => true,
        Some(_) => {
            let corners = [
                Point::zero(),
                Point::new(size.x, Abs::zero()),
                Point::new(Abs::zero(), size.y),
                Point::new(size.x, size.y),
            ]
            .map(|p| p.transform(ts));
            let (min, max) = bounds(&corners);
            match clip {
                Clip::None => true,
                Clip::Rect(cmin, cmax) => {
                    min.x >= cmin.x && min.y >= cmin.y && max.x <= cmax.x && max.y <= cmax.y
                }
                Clip::Complex => false,
            }
        }
    }
}

/// A frame laid out in a frame rotated by `degrees`, placed on the page:
/// its centre turns about the origin, its size stays.
fn rotate(frame: Rect, degrees: f64) -> Rect {
    if degrees == 0.0 {
        return frame;
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    let cx = frame.x as f64 + frame.w as f64 / 2.0;
    let cy = frame.y as f64 + frame.h as f64 / 2.0;
    let (px, py) = (cx * cos - cy * sin, cx * sin + cy * cos);
    Rect {
        x: (px - frame.w as f64 / 2.0).round() as i64,
        y: (py - frame.h as f64 / 2.0).round() as i64,
        rot: ((degrees * 60_000.0).round() as i32).rem_euclid(21_600_000),
        ..frame
    }
}

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
        rot: 0,
    }
}

fn solid(paint: &Paint) -> Option<Color> {
    let Paint::Solid(c) = paint else {
        return None;
    };
    color(c)
}

fn color(color: &TypstColor) -> Option<Color> {
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
/// flags from the subfamily (name ID 2), plan §5.1. Also remembers which
/// font backs each face, for embedding.
#[derive(Default)]
struct FontNames {
    refs: HashMap<(Option<String>, u32), FontRef>,
    /// Faces in first-use order.
    faces: Vec<(FontRef, Font)>,
}

impl FontNames {
    fn get(&mut self, font: &FontInstance) -> FontRef {
        let key = (font.font().post_script_name(), font.font().index());
        if let Some(found) = self.refs.get(&key) {
            return found.clone();
        }
        let ttf = font.ttf();
        let name = |id: u16| {
            ttf.names()
                .into_iter()
                .filter(|name| name.name_id == id)
                .find_map(|name| name.to_string())
        };
        let family =
            name(ttf_parser::name_id::FAMILY).unwrap_or_else(|| font.font().info().family.clone());
        let subfamily = name(ttf_parser::name_id::SUBFAMILY).unwrap_or_default();
        let face = FontRef {
            family,
            bold: subfamily.contains("Bold"),
            italic: subfamily.contains("Italic") || subfamily.contains("Oblique"),
        };
        if !self.faces.iter().any(|(known, _)| *known == face) {
            self.faces.push((face.clone(), font.font().clone()));
        }
        self.refs.insert(key, face.clone());
        face
    }

    /// The faces PowerPoint can embed: a single font (not a collection)
    /// with an `fsType` that allows embedding. PowerPoint takes TrueType
    /// outlines only, so CFF fonts are converted. The others are reported;
    /// PowerPoint substitutes them where they aren't installed.
    fn embeddable(&self, report: &mut Report) -> Vec<EmbeddedFont> {
        let mut out = Vec::new();
        for (face, font) in &self.faces {
            let name = format!(
                "{}{}{}",
                face.family,
                if face.bold { " Bold" } else { "" },
                if face.italic { " Italic" } else { "" }
            );
            if let Some(reason) = embedding_problem(font) {
                report.fonts_not_embedded.push((name, reason));
                continue;
            }
            let data = if truetype::is_cff(font.data()) {
                match truetype::from_cff(font.data()) {
                    Some(data) => data,
                    None => {
                        let reason = "its CFF outlines couldn't be converted to TrueType";
                        report.fonts_not_embedded.push((name, reason));
                        continue;
                    }
                }
            } else {
                font.data().to_vec()
            };
            out.push(EmbeddedFont {
                font: face.clone(),
                data,
            });
            report.fonts_embedded.push(name);
        }
        out
    }
}

fn embedding_problem(font: &Font) -> Option<&'static str> {
    let data = font.data().as_slice();
    if data.starts_with(b"ttcf") || font.index() != 0 {
        return Some("font collections can't be embedded");
    }
    let ttf = ttf_parser::Face::parse(data, 0).ok()?;
    let os2 = ttf.tables().os2?;
    if os2.permissions() == Some(ttf_parser::Permissions::Restricted) {
        return Some("its licence forbids embedding (fsType restricted)");
    }
    if !os2.is_outline_embedding_allowed() {
        return Some("its licence allows embedding bitmaps only (fsType)");
    }
    None
}
