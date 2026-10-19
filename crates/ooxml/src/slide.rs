//! Slide parts: `p:sld` with its shape tree.

use std::fmt::Write;

use pptx_ir::{Color, Geometry, Item, Paragraph, Rect, Run, Shape, Slide, TextBox};

use crate::xml::{DECLARATION, escape, pml_namespaces};

pub fn slide(slide: &Slide) -> String {
    let mut out = format!("{DECLARATION}<p:sld {}><p:cSld>", pml_namespaces());
    if let Some(color) = slide.background {
        write!(
            out,
            "<p:bg><p:bgPr>{}<a:effectLst/></p:bgPr></p:bg>",
            solid_fill(color)
        )
        .unwrap();
    }
    out.push_str("<p:spTree>");
    out.push_str(EMPTY_GROUP_PROPS);
    // Shape ids are unique per slide; 1 is the shape tree itself.
    for (index, item) in slide.items.iter().enumerate() {
        let id = index + 2;
        match item {
            Item::TextBox(text) => text_box(&mut out, id, text),
            Item::Shape(shape) => match shape.geometry {
                Geometry::Line { flip_h, flip_v } => connector(&mut out, id, shape, flip_h, flip_v),
                Geometry::Rect => preset_shape(&mut out, id, shape, "rect"),
            },
        }
    }
    out.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>");
    out
}

/// `p:nvGrpSpPr` + `p:grpSpPr` of a shape tree that isn't transformed.
pub const EMPTY_GROUP_PROPS: &str = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#;

fn text_box(out: &mut String, id: usize, text: &TextBox) {
    write!(
        out,
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>"#,
        escape(&text.name),
        xfrm(text.frame, ""),
    )
    .unwrap();
    // Exact mode (plan §5.2): no insets, no wrapping, no autofit.
    out.push_str(r#"<p:txBody><a:bodyPr wrap="none" lIns="0" tIns="0" rIns="0" bIns="0" rtlCol="0" anchor="t"><a:noAutofit/></a:bodyPr><a:lstStyle/>"#);
    for paragraph in &text.paragraphs {
        para(out, paragraph);
    }
    out.push_str("</p:txBody></p:sp>");
}

fn para(out: &mut String, paragraph: &Paragraph) {
    out.push_str("<a:p><a:pPr>");
    if let Some(spacing) = paragraph.line_spacing {
        write!(out, r#"<a:lnSpc><a:spcPts val="{spacing}"/></a:lnSpc>"#).unwrap();
    }
    out.push_str(r#"<a:spcBef><a:spcPts val="0"/></a:spcBef><a:spcAft><a:spcPts val="0"/></a:spcAft></a:pPr>"#);
    let mut last = None;
    for (index, line) in paragraph.lines.iter().enumerate() {
        if index > 0 {
            write!(
                out,
                "<a:br>{}</a:br>",
                run_props("a:rPr", last.or(line.first()))
            )
            .unwrap();
        }
        for run in line {
            write!(
                out,
                "<a:r>{}<a:t>{}</a:t></a:r>",
                run_props("a:rPr", Some(run)),
                escape(&run.text)
            )
            .unwrap();
            last = Some(run);
        }
    }
    out.push_str(&run_props("a:endParaRPr", last));
    out.push_str("</a:p>");
}

fn run_props(tag: &str, run: Option<&Run>) -> String {
    let Some(run) = run else {
        return format!(r#"<{tag} lang="en-US" dirty="0"/>"#);
    };
    let family = escape(&run.font.family);
    format!(
        r#"<{tag} lang="en-US" sz="{}" b="{}" i="{}" kern="100" dirty="0">{}<a:latin typeface="{family}"/><a:ea typeface="{family}"/><a:cs typeface="{family}"/></{tag}>"#,
        run.size.clamp(100, 400_000),
        u8::from(run.font.bold),
        u8::from(run.font.italic),
        solid_fill(run.color),
    )
}

fn preset_shape(out: &mut String, id: usize, shape: &Shape, preset: &str) {
    write!(
        out,
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="{preset}"><a:avLst/></a:prstGeom>{}{}</p:spPr></p:sp>"#,
        escape(&shape.name),
        xfrm(shape.frame, ""),
        shape.fill.map(solid_fill).unwrap_or_else(|| "<a:noFill/>".into()),
        line(shape),
    )
    .unwrap();
}

fn connector(out: &mut String, id: usize, shape: &Shape, flip_h: bool, flip_v: bool) {
    let mut flips = String::new();
    if flip_h {
        flips.push_str(r#" flipH="1""#);
    }
    if flip_v {
        flips.push_str(r#" flipV="1""#);
    }
    write!(
        out,
        r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr>{}<a:prstGeom prst="line"><a:avLst/></a:prstGeom>{}</p:spPr></p:cxnSp>"#,
        escape(&shape.name),
        xfrm(shape.frame, &flips),
        line(shape),
    )
    .unwrap();
}

fn line(shape: &Shape) -> String {
    match &shape.line {
        Some(line) => format!(
            r#"<a:ln w="{}">{}</a:ln>"#,
            line.width,
            solid_fill(line.color)
        ),
        None => "<a:ln><a:noFill/></a:ln>".into(),
    }
}

fn xfrm(rect: Rect, attrs: &str) -> String {
    format!(
        r#"<a:xfrm{attrs}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        rect.x,
        rect.y,
        rect.w.max(0),
        rect.h.max(0)
    )
}

pub fn solid_fill(color: Color) -> String {
    let hex = format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    if color.a == 255 {
        format!(r#"<a:solidFill><a:srgbClr val="{hex}"/></a:solidFill>"#)
    } else {
        // Alpha in thousandths of a percent.
        let alpha = (u32::from(color.a) * 100_000 + 127) / 255;
        format!(
            r#"<a:solidFill><a:srgbClr val="{hex}"><a:alpha val="{alpha}"/></a:srgbClr></a:solidFill>"#
        )
    }
}
