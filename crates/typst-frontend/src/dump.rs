//! Human-readable dump of the page walk (`typst-pptx dump`). Phase 0 uses it
//! to check which elements carry tags and what the frames contain; it is not
//! a stable format.

use std::collections::BTreeMap;
use std::fmt::Write;

use typst::foundations::Content;
use typst::layout::{Abs, FrameItem, Transform};
use typst::text::TextItem;
use typst::visualize::{Geometry, ImageKind, Paint};
use typst_layout::PagedDocument;

use crate::walk::{self, Context, Event};

#[derive(Debug, Clone, Copy, Default)]
pub struct DumpOptions {
    /// Print open element names next to every leaf item.
    pub tag_paths: bool,
    /// Print tag start/end lines.
    pub tags: bool,
}

/// Dumps the selected pages (1-based; empty = all) and a tag summary.
pub fn dump(doc: &PagedDocument, pages: &[usize], options: DumpOptions) -> String {
    let mut out = String::new();
    let mut tag_counts: BTreeMap<&'static str, usize> = BTreeMap::new();

    for (index, page) in doc.pages().iter().enumerate() {
        let number = index + 1;
        if !pages.is_empty() && !pages.contains(&number) {
            continue;
        }
        let size = page.frame.size();
        let fill = page
            .fill_or_transparent()
            .as_ref()
            .map(paint)
            .unwrap_or("none".into());
        writeln!(
            out,
            "page {number} {} x {} fill={fill}",
            pt(size.x),
            pt(size.y)
        )
        .unwrap();

        let open = walk::walk(&page.frame, &mut |ctx, event| {
            if let Event::TagStart(elem) = &event {
                *tag_counts.entry(elem.elem().name()).or_default() += 1;
            }
            if let Some(line) = line(ctx, &event, options) {
                let indent = "  ".repeat(ctx.depth + 1);
                writeln!(out, "{indent}{line}").unwrap();
            }
        });
        if !open.is_empty() {
            writeln!(out, "  (still open at page end: {})", names(&open)).unwrap();
        }
    }

    writeln!(out, "tag starts by element:").unwrap();
    for (name, count) in tag_counts {
        writeln!(out, "  {name:<16} {count}").unwrap();
    }
    out
}

fn line(ctx: &Context, event: &Event, options: DumpOptions) -> Option<String> {
    let at = walk::origin(ctx.ts);
    let at = format!("@{},{}", pt(at.x), pt(at.y));
    let mut line = match event {
        Event::GroupStart(group) => {
            let size = group.frame.size();
            let mut s = format!("group {at} {} x {}", pt(size.x), pt(size.y));
            if !is_translation(ctx.ts) {
                write!(s, " ts={}", transform(ctx.ts)).unwrap();
            }
            if group.clip.is_some() {
                s.push_str(" clip");
            }
            if let Some(label) = group.label {
                write!(s, " <{}>", label.resolve()).unwrap();
            }
            if group.parent.is_some() {
                s.push_str(" parented");
            }
            s
        }
        Event::GroupEnd => return None,
        Event::TagStart(elem) if options.tags => format!("+{}", describe(elem)),
        Event::TagEnd(elem) if options.tags => format!("-{}", elem.elem().name()),
        Event::TagStart(_) | Event::TagEnd(_) => return None,
        Event::Item(FrameItem::Text(text)) => text_line(&at, text),
        Event::Item(FrameItem::Shape(shape, _)) => {
            let geometry = match &shape.geometry {
                Geometry::Line(to) => format!("line to {},{}", pt(to.x), pt(to.y)),
                Geometry::Rect(size) => format!("rect {} x {}", pt(size.x), pt(size.y)),
                Geometry::Curve(curve) => format!("curve {} items", curve.0.len()),
            };
            let mut s = format!("shape {at} {geometry}");
            if let Some(fill) = &shape.fill {
                write!(s, " fill={}", paint(fill)).unwrap();
            }
            if let Some(stroke) = &shape.stroke {
                write!(
                    s,
                    " stroke={}/{}",
                    paint(&stroke.paint),
                    pt(stroke.thickness)
                )
                .unwrap();
            }
            s
        }
        Event::Item(FrameItem::Image(image, size, _)) => {
            let kind = match image.kind() {
                ImageKind::Raster(_) => "raster",
                ImageKind::Svg(_) => "svg",
                ImageKind::Pdf(_) => "pdf",
            };
            format!(
                "image {at} {} x {} {kind} {}x{}",
                pt(size.x),
                pt(size.y),
                image.width(),
                image.height()
            )
        }
        Event::Item(FrameItem::Link(dest, size)) => {
            format!("link {at} {} x {} {dest:?}", pt(size.x), pt(size.y))
        }
        Event::Item(FrameItem::Group(_) | FrameItem::Tag(_)) => unreachable!(),
    };
    if options.tag_paths && matches!(event, Event::Item(_)) && !ctx.tags.is_empty() {
        write!(line, "  in {}", names(ctx.tags)).unwrap();
    }
    Some(line)
}

fn text_line(at: &str, text: &TextItem) -> String {
    let info = text.font.font().info();
    let ps = text.font.font().post_script_name().unwrap_or_default();
    format!(
        "text {at} w={} size={} font={:?}/{ps} {:?} {} fill={} {:?}",
        pt(text.width()),
        pt(text.size),
        info.family,
        info.variant.style,
        info.variant.weight.to_number(),
        paint(&text.fill),
        text.text.as_str(),
    )
}

fn describe(elem: &Content) -> String {
    let mut s = elem.elem().name().to_string();
    if let Some(label) = elem.label() {
        write!(s, " <{}>", label.resolve()).unwrap();
    }
    s
}

fn names(elems: &[&Content]) -> String {
    elems
        .iter()
        .map(|e| e.elem().name())
        .collect::<Vec<_>>()
        .join(">")
}

fn paint(paint: &Paint) -> String {
    match paint {
        Paint::Solid(color) => color.to_hex().to_string(),
        Paint::Gradient(_) => "gradient".into(),
        Paint::Tiling(_) => "tiling".into(),
    }
}

fn is_translation(ts: Transform) -> bool {
    ts.is_only_translate()
}

fn transform(ts: Transform) -> String {
    format!(
        "[{:.3} {:.3} {:.3} {:.3} {} {}]",
        ts.sx.get(),
        ts.ky.get(),
        ts.kx.get(),
        ts.sy.get(),
        pt(ts.tx),
        pt(ts.ty)
    )
}

/// Points with two decimals, trailing zeros trimmed.
fn pt(abs: Abs) -> String {
    let s = format!("{:.2}", abs.to_pt());
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}
