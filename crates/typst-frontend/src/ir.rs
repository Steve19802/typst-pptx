//! Page frames → IR. Phase 0 spike: one text box per Typst text item (line
//! mode), rectangles, straight lines and the page fill. Paragraph grouping,
//! curves, images and clips follow in Phase 1.

use std::collections::HashMap;

use pptx_ir::{
    Color, Deck, Fidelity, FontRef, Geometry, Item, Line, Paragraph, Rect, Run, Shape, Slide,
    TextBox, emu,
};
use typst::layout::{Abs, FrameItem, Point, Transform};
use typst::text::{FontInstance, TextItem};
use typst::visualize::{Geometry as TypstGeometry, Paint, Shape as TypstShape};
use typst_layout::PagedDocument;

use crate::walk::{self, Event};

/// Counts of frame items the spike can't map yet, by kind.
pub type Skipped = HashMap<&'static str, usize>;

pub fn deck(doc: &PagedDocument) -> (Deck, Skipped) {
    let mut skipped = Skipped::new();
    let mut fonts = FontNames::default();
    let (width, height) = doc
        .pages()
        .first()
        .map(|page| (page.frame.width(), page.frame.height()))
        .unwrap_or_default();

    let slides = doc
        .pages()
        .iter()
        .map(|page| {
            let mut items = Vec::new();
            walk::walk(&page.frame, &mut |ctx, event| {
                let Event::Item(item) = event else { return };
                if let FrameItem::Text(text) = item
                    && text.text.trim().is_empty()
                {
                    return;
                }
                let mapped = match item {
                    FrameItem::Text(text) => text_box(ctx.ts, text, &mut fonts, items.len()),
                    FrameItem::Shape(shape, _) => self::shape(ctx.ts, shape, items.len()),
                    FrameItem::Image(..) => None,
                    FrameItem::Link(..) => return,
                    FrameItem::Group(_) | FrameItem::Tag(_) => unreachable!(),
                };
                match mapped {
                    Some(item) => items.push(item),
                    None => *skipped.entry(kind(item)).or_default() += 1,
                }
            });
            Slide {
                background: page.fill_or_transparent().as_ref().and_then(solid),
                items,
                notes: Vec::new(),
            }
        })
        .collect();

    (
        Deck {
            width: emu(width.to_pt()),
            height: emu(height.to_pt()),
            slides,
        },
        skipped,
    )
}

fn kind(item: &FrameItem) -> &'static str {
    match item {
        FrameItem::Text(_) => "text (transformed or non-solid)",
        FrameItem::Shape(..) => "shape (curve, transformed or non-solid)",
        FrameItem::Image(..) => "image",
        _ => "other",
    }
}

fn text_box(ts: Transform, text: &TextItem, fonts: &mut FontNames, n: usize) -> Option<Item> {
    if !ts.is_only_translate() {
        return None;
    }
    let color = solid(&text.fill)?;
    let metrics = text.font.metrics();
    // Typst places text at its baseline. The box top is one ascender above,
    // which is where PowerPoint puts the first baseline with single spacing
    // and zero insets (to be calibrated, plan §5.2).
    let baseline = walk::origin(ts);
    let ascent = metrics.ascender.at(text.size);
    let descent = -metrics.descender.at(text.size);
    let frame = rect(
        Point::new(baseline.x, baseline.y - ascent),
        Point::new(baseline.x + text.width(), baseline.y + descent),
    );
    Some(Item::TextBox(TextBox {
        name: format!("Text {n}"),
        frame,
        paragraphs: vec![Paragraph {
            line_spacing: None,
            lines: vec![vec![Run {
                text: text.text.to_string(),
                font: fonts.get(&text.font),
                size: (text.size.to_pt() * 100.0).round() as i32,
                color,
            }]],
        }],
        fidelity: Fidelity::Native,
    }))
}

fn shape(ts: Transform, shape: &TypstShape, n: usize) -> Option<Item> {
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
        TypstGeometry::Curve(_) => return None,
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
