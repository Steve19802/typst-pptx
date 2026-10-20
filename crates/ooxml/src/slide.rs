//! Slide parts: `p:sld` with its shape tree.

use std::fmt::Write;

use pptx_ir::{
    Align, Color, Fill, Geometry, Item, LineCap, LineJoin, Media, MediaFormat, Paragraph,
    PathCommand, Picture, Rect, Run, Shape, Slide, Stop, TextBox,
};

use crate::xml::{DECLARATION, escape, pml_namespaces};

/// Picture relationships of one slide: media index → relationship id.
pub struct Rels {
    first: usize,
    media: Vec<usize>,
}

impl Rels {
    pub fn new(first: usize) -> Self {
        Self {
            first,
            media: Vec::new(),
        }
    }

    fn id(&mut self, media: usize) -> usize {
        let index = match self.media.iter().position(|&m| m == media) {
            Some(index) => index,
            None => {
                self.media.push(media);
                self.media.len() - 1
            }
        };
        self.first + index
    }

    /// (relationship id, media index) pairs in id order.
    pub fn media(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.media
            .iter()
            .enumerate()
            .map(|(index, &media)| (self.first + index, media))
    }

    pub fn next_id(&self) -> usize {
        self.first + self.media.len()
    }
}

pub fn slide(slide: &Slide, media: &[Media], rels: &mut Rels) -> String {
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
    let mut next_id = 2;
    for item in &slide.items {
        item_xml(&mut out, item, &mut next_id, media, rels);
    }
    out.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>");
    out
}

fn item_xml(out: &mut String, item: &Item, next_id: &mut usize, media: &[Media], rels: &mut Rels) {
    let id = *next_id;
    *next_id += 1;
    match item {
        Item::TextBox(text) => text_box(out, id, text),
        Item::Shape(shape) => match &shape.geometry {
            Geometry::Line { flip_h, flip_v } => connector(out, id, shape, *flip_h, *flip_v),
            Geometry::Rect => shape_xml(out, id, shape, &preset("rect", "")),
            Geometry::Ellipse => shape_xml(out, id, shape, &preset("ellipse", "")),
            Geometry::RoundRect { radius } => {
                let short = shape.frame.w.min(shape.frame.h).max(1);
                let adj = (radius * 100_000 / short).clamp(0, 50_000);
                let guide = format!(r#"<a:gd name="adj" fmla="val {adj}"/>"#);
                shape_xml(out, id, shape, &preset("roundRect", &guide))
            }
            Geometry::Custom { commands } => {
                shape_xml(out, id, shape, &custom(shape.frame, commands))
            }
        },
        Item::Picture(picture) => picture_xml(out, id, picture, media, rels),
        Item::Group(group) => {
            // Child offset and extent equal the group's own: no transform.
            let Rect { x, y, w, h } = group.frame;
            write!(
                out,
                r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/><a:chOff x="{x}" y="{y}"/><a:chExt cx="{w}" cy="{h}"/></a:xfrm></p:grpSpPr>"#,
                escape(&group.name)
            )
            .unwrap();
            for child in &group.items {
                item_xml(out, child, next_id, media, rels);
            }
            out.push_str("</p:grpSp>");
        }
    }
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
    let align = match paragraph.align {
        Align::Left => "l",
        Align::Center => "ctr",
        Align::Right => "r",
    };
    write!(out, r#"<a:p><a:pPr algn="{align}">"#).unwrap();
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
    let spacing = run
        .spacing
        .map(|spc| format!(r#" spc="{}""#, spc.clamp(-400_000, 400_000)))
        .unwrap_or_default();
    format!(
        r#"<{tag} lang="en-US" sz="{}" b="{}" i="{}" kern="100"{spacing} dirty="0">{}<a:latin typeface="{family}"/><a:ea typeface="{family}"/><a:cs typeface="{family}"/></{tag}>"#,
        run.size.clamp(100, 400_000),
        u8::from(run.font.bold),
        u8::from(run.font.italic),
        solid_fill(run.color),
    )
}

fn preset(name: &str, guides: &str) -> String {
    format!(r#"<a:prstGeom prst="{name}"><a:avLst>{guides}</a:avLst></a:prstGeom>"#)
}

/// `a:custGeom` with one path in the frame's coordinate space.
fn custom(frame: Rect, commands: &[PathCommand]) -> String {
    let (w, h) = (frame.w.max(1), frame.h.max(1));
    let mut out = format!(
        r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="{w}" h="{h}">"#
    );
    let pt = |(x, y): (i64, i64)| format!(r#"<a:pt x="{x}" y="{y}"/>"#);
    for command in commands {
        match *command {
            PathCommand::Move { x, y } => {
                write!(out, "<a:moveTo>{}</a:moveTo>", pt((x, y))).unwrap()
            }
            PathCommand::Line { x, y } => write!(out, "<a:lnTo>{}</a:lnTo>", pt((x, y))).unwrap(),
            PathCommand::Cubic { pts: [a, b, c] } => write!(
                out,
                "<a:cubicBezTo>{}{}{}</a:cubicBezTo>",
                pt(a),
                pt(b),
                pt(c)
            )
            .unwrap(),
            PathCommand::Close => out.push_str("<a:close/>"),
        }
    }
    out.push_str("</a:path></a:pathLst></a:custGeom>");
    out
}

/// Office 2016+ SVG extension of `a:blip`; older versions show the PNG.
const SVG_BLIP_EXT: &str = "{96DAC541-7B7A-43D3-8B79-37D633B846F1}";

fn picture_xml(out: &mut String, id: usize, picture: &Picture, media: &[Media], rels: &mut Rels) {
    let item = &media[picture.media];
    let blip = match (item.format, item.fallback) {
        (MediaFormat::Svg, Some(png)) => format!(
            r#"<a:blip r:embed="rId{}"><a:extLst><a:ext uri="{SVG_BLIP_EXT}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="rId{}"/></a:ext></a:extLst></a:blip>"#,
            rels.id(png),
            rels.id(picture.media)
        ),
        _ => format!(r#"<a:blip r:embed="rId{}"/>"#, rels.id(picture.media)),
    };
    let crop = picture
        .crop
        .map(|c| {
            format!(
                r#"<a:srcRect l="{}" t="{}" r="{}" b="{}"/>"#,
                c.l, c.t, c.r, c.b
            )
        })
        .unwrap_or_default();
    let descr = picture
        .descr
        .as_deref()
        .map(|d| format!(r#" descr="{}""#, escape(d)))
        .unwrap_or_default();
    write!(
        out,
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{}"{descr}/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill>{blip}{crop}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
        escape(&picture.name),
        xfrm(picture.frame, ""),
    )
    .unwrap();
}

fn shape_xml(out: &mut String, id: usize, shape: &Shape, geometry: &str) {
    write!(
        out,
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}{geometry}{}{}</p:spPr></p:sp>"#,
        escape(&shape.name),
        xfrm(shape.frame, ""),
        shape
            .fill
            .as_ref()
            .map(fill_xml)
            .unwrap_or_else(|| "<a:noFill/>".into()),
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
    let Some(line) = &shape.line else {
        return "<a:ln><a:noFill/></a:ln>".into();
    };
    let cap = match line.cap {
        LineCap::Flat => "flat",
        LineCap::Round => "rnd",
        LineCap::Square => "sq",
    };
    let mut out = format!(
        r#"<a:ln w="{}" cap="{cap}">{}"#,
        line.width,
        solid_fill(line.color)
    );
    if !line.dash.is_empty() {
        out.push_str("<a:custDash>");
        for pair in line.dash.chunks(2) {
            write!(out, r#"<a:ds d="{}" sp="{}"/>"#, pair[0], pair[1]).unwrap();
        }
        out.push_str("</a:custDash>");
    }
    match line.join {
        LineJoin::Miter { limit } => write!(
            out,
            r#"<a:miter lim="{}"/>"#,
            (limit * 100_000.0).round() as i64
        )
        .unwrap(),
        LineJoin::Round => out.push_str("<a:round/>"),
        LineJoin::Bevel => out.push_str("<a:bevel/>"),
    }
    out.push_str("</a:ln>");
    out
}

fn fill_xml(fill: &Fill) -> String {
    let stops = |stops: &[Stop]| {
        let mut out = String::from("<a:gsLst>");
        for stop in stops {
            write!(
                out,
                r#"<a:gs pos="{}">{}</a:gs>"#,
                stop.pos,
                color(stop.color)
            )
            .unwrap();
        }
        out.push_str("</a:gsLst>");
        out
    };
    match fill {
        Fill::Solid { color } => solid_fill(*color),
        Fill::Linear { angle, stops: s } => {
            let ang = ((angle.rem_euclid(360.0)) * 60_000.0).round() as i64 % 21_600_000;
            format!(
                r#"<a:gradFill rotWithShape="1">{}<a:lin ang="{ang}" scaled="1"/></a:gradFill>"#,
                stops(s)
            )
        }
        Fill::Radial { stops: s } => format!(
            r#"<a:gradFill rotWithShape="1">{}<a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path></a:gradFill>"#,
            stops(s)
        ),
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

pub fn solid_fill(c: Color) -> String {
    format!("<a:solidFill>{}</a:solidFill>", color(c))
}

fn color(color: Color) -> String {
    let hex = format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    if color.a == 255 {
        format!(r#"<a:srgbClr val="{hex}"/>"#)
    } else {
        // Alpha in thousandths of a percent.
        let alpha = (u32::from(color.a) * 100_000 + 127) / 255;
        format!(r#"<a:srgbClr val="{hex}"><a:alpha val="{alpha}"/></a:srgbClr>"#)
    }
}
