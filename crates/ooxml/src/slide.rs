//! Slide parts: `p:sld` with its shape tree.

use std::cell::RefCell;
use std::fmt::Write;

use pptx_ir::{
    Align, BulletKind, Color, Fill, Geometry, Item, Layout, Line, LineCap, LineJoin, Link, Media,
    MediaFormat, Merge, Paragraph, PathCommand, Picture, Rect, Run, Shape, Slide, Stop, Table,
    TextBox, Theme,
};

use crate::timing;
use crate::xml::{DECLARATION, escape, pml_namespaces};

/// Office 2019+ hyperlink colour extension of `a:hlinkClick`.
const HLINK_COLOR_EXT: &str = "{A12FA001-AC4F-418D-AE19-62706E023703}";

/// What a slide relationship points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A media index of the deck.
    Media(usize),
    Url(String),
    /// A slide of the deck (0-based).
    Slide(usize),
}

/// Relationships of one slide's content, numbered from `first`.
pub struct Rels {
    first: usize,
    targets: Vec<Target>,
}

impl Rels {
    pub fn new(first: usize) -> Self {
        Self {
            first,
            targets: Vec::new(),
        }
    }

    fn id(&mut self, target: Target) -> usize {
        let index = match self.targets.iter().position(|t| *t == target) {
            Some(index) => index,
            None => {
                self.targets.push(target);
                self.targets.len() - 1
            }
        };
        self.first + index
    }

    /// (relationship id, target) pairs in id order.
    pub fn targets(&self) -> impl Iterator<Item = (usize, &Target)> + '_ {
        self.targets
            .iter()
            .enumerate()
            .map(|(index, target)| (self.first + index, target))
    }

    pub fn next_id(&self) -> usize {
        self.first + self.targets.len()
    }

    /// `a:hlinkClick` for a link; on text, Office 2019+ is told to keep the
    /// run's colour instead of the theme's hyperlink colour.
    fn hyperlink(&mut self, link: &Link, text: bool) -> String {
        let (id, action) = match link {
            Link::Url(url) => (self.id(Target::Url(url.clone())), ""),
            Link::Slide(slide) => (
                self.id(Target::Slide(*slide)),
                r#" action="ppaction://hlinksldjump""#,
            ),
            // Pages without an exported slide: no link.
            Link::Page(_) => return String::new(),
        };
        if text {
            format!(
                r#"<a:hlinkClick r:id="rId{id}"{action}><a:extLst><a:ext uri="{HLINK_COLOR_EXT}"><ahyp:hlinkClr xmlns:ahyp="http://schemas.microsoft.com/office/drawing/2018/hyperlinkcolor" val="tx"/></a:ext></a:extLst></a:hlinkClick>"#
            )
        } else {
            format!(r#"<a:hlinkClick r:id="rId{id}"{action}/>"#)
        }
    }
}

pub fn slide(slide: &Slide, media: &[Media], rels: &mut Rels) -> String {
    let mut out = format!("{DECLARATION}<p:sld {}><p:cSld>", pml_namespaces());
    background(&mut out, slide.background);
    let ids = shape_tree(&mut out, &slide.items, media, rels);
    out.push_str("</p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>");
    out.push_str(timing::transition(slide));
    out.push_str(&timing::timing(slide, &ids));
    out.push_str("</p:sld>");
    out
}

/// A slide layout holding repeated content (plan §7).
pub fn layout(layout: &Layout, media: &[Media], rels: &mut Rels) -> String {
    let mut out = format!(
        r#"{DECLARATION}<p:sldLayout {} preserve="1"><p:cSld name="{}">"#,
        pml_namespaces(),
        escape(&layout.name)
    );
    background(&mut out, layout.background);
    shape_tree(&mut out, &layout.items, media, rels);
    out.push_str("</p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>");
    out
}

fn background(out: &mut String, color: Option<Color>) {
    if let Some(color) = color {
        write!(
            out,
            "<p:bg><p:bgPr>{}<a:effectLst/></p:bgPr></p:bg>",
            solid_fill(color)
        )
        .unwrap();
    }
}

/// `p:spTree` with the items; returns each item's shape id.
fn shape_tree(out: &mut String, items: &[Item], media: &[Media], rels: &mut Rels) -> Vec<usize> {
    out.push_str("<p:spTree>");
    out.push_str(EMPTY_GROUP_PROPS);
    // Shape ids are unique per part; 1 is the shape tree itself.
    let mut next_id = 2;
    let mut ids = Vec::with_capacity(items.len());
    for item in items {
        ids.push(next_id);
        item_xml(out, item, &mut next_id, media, rels);
    }
    out.push_str("</p:spTree>");
    ids
}

fn item_xml(out: &mut String, item: &Item, next_id: &mut usize, media: &[Media], rels: &mut Rels) {
    let id = *next_id;
    *next_id += 1;
    match item {
        Item::TextBox(text) => text_box(out, id, text, rels),
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
        Item::Table(table) => table_xml(out, id, table, rels),
        Item::Group(group) => {
            // Child offset and extent equal the group's own: no transform.
            let Rect { x, y, w, h, .. } = group.frame;
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

fn text_box(out: &mut String, id: usize, text: &TextBox, rels: &mut Rels) {
    let link = text
        .link
        .as_ref()
        .map(|link| rels.hyperlink(link, false))
        .unwrap_or_default();
    write!(
        out,
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}">{link}</p:cNvPr><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>"#,
        escape(&text.name),
        xfrm(text.frame, ""),
    )
    .unwrap();
    // Exact mode (plan §5.2): no insets, no wrapping, no autofit.
    let wrap = if text.wrap { "square" } else { "none" };
    write!(out, r#"<p:txBody><a:bodyPr wrap="{wrap}" lIns="0" tIns="0" rIns="0" bIns="0" rtlCol="0" anchor="t"><a:noAutofit/></a:bodyPr><a:lstStyle/>"#).unwrap();
    for paragraph in &text.paragraphs {
        para(out, paragraph, rels);
    }
    out.push_str("</p:txBody></p:sp>");
}

fn para(out: &mut String, paragraph: &Paragraph, rels: &mut Rels) {
    let align = match paragraph.align {
        Align::Left => "l",
        Align::Center => "ctr",
        Align::Right => "r",
    };
    let mut margins = paragraph
        .indents
        .map(|(left, right)| format!(r#" marL="{left}" marR="{right}""#))
        .unwrap_or_default();
    if let Some(list) = &paragraph.list {
        write!(margins, r#" lvl="{}" indent="{}""#, list.level, list.indent).unwrap();
    }
    write!(out, r#"<a:p><a:pPr{margins} algn="{align}">"#).unwrap();
    if let Some(spacing) = paragraph.line_spacing {
        write!(out, r#"<a:lnSpc><a:spcPts val="{spacing}"/></a:lnSpc>"#).unwrap();
    }
    let before = paragraph.list.as_ref().map_or(0, |list| list.space_before);
    write!(
        out,
        r#"<a:spcBef><a:spcPts val="{before}"/></a:spcBef><a:spcAft><a:spcPts val="0"/></a:spcAft>"#
    )
    .unwrap();
    match paragraph.list.as_ref().map(|list| &list.bullet) {
        Some(Some(bullet)) => {
            write!(
                out,
                r#"<a:buClr>{}</a:buClr><a:buSzPts val="{}"/><a:buFont typeface="{}"/>"#,
                color(bullet.color),
                bullet.size.clamp(100, 400_000),
                escape(&bullet.font)
            )
            .unwrap();
            match &bullet.kind {
                BulletKind::Char { char } => {
                    write!(out, r#"<a:buChar char="{}"/>"#, escape(char)).unwrap()
                }
                BulletKind::Number { scheme, start } => {
                    let start = start
                        .map(|n| format!(r#" startAt="{n}""#))
                        .unwrap_or_default();
                    write!(out, r#"<a:buAutoNum type="{scheme}"{start}/>"#).unwrap()
                }
            }
        }
        Some(None) => out.push_str("<a:buNone/>"),
        None => {}
    }
    out.push_str("</a:pPr>");
    let mut last = None;
    for (index, line) in paragraph.lines.iter().enumerate() {
        if index > 0 {
            write!(
                out,
                "<a:br>{}</a:br>",
                run_props("a:rPr", last.or(line.first()), "")
            )
            .unwrap();
        }
        for run in line {
            let link = run
                .link
                .as_ref()
                .map(|link| rels.hyperlink(link, true))
                .unwrap_or_default();
            write!(
                out,
                "<a:r>{}<a:t>{}</a:t></a:r>",
                run_props("a:rPr", Some(run), &link),
                escape(&run.text)
            )
            .unwrap();
            last = Some(run);
        }
    }
    out.push_str(&run_props("a:endParaRPr", last, ""));
    out.push_str("</a:p>");
}

/// `link` is an `a:hlinkClick` or empty.
fn run_props(tag: &str, run: Option<&Run>, link: &str) -> String {
    let Some(run) = run else {
        // Only empty table cells: 1 pt, so the mark can't grow a low row.
        return format!(r#"<{tag} lang="en-US" sz="100" dirty="0"/>"#);
    };
    let family = escape(&run.font.family);
    let spacing = run
        .spacing
        .map(|spc| format!(r#" spc="{}""#, spc.clamp(-400_000, 400_000)))
        .unwrap_or_default();
    // Typst underlines are separate lines; applications would otherwise
    // underline hyperlinks themselves.
    let underline = if link.is_empty() { "" } else { r#" u="none""# };
    format!(
        r#"<{tag} lang="en-US" sz="{}" b="{}" i="{}"{underline} kern="100"{spacing} dirty="0">{}<a:latin typeface="{family}"/><a:ea typeface="{family}"/><a:cs typeface="{family}"/>{link}</{tag}>"#,
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
            rels.id(Target::Media(png)),
            rels.id(Target::Media(picture.media))
        ),
        _ => format!(
            r#"<a:blip r:embed="rId{}"/>"#,
            rels.id(Target::Media(picture.media))
        ),
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
    let link = picture
        .link
        .as_ref()
        .map(|link| rels.hyperlink(link, false))
        .unwrap_or_default();
    write!(
        out,
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{}"{descr}>{link}</p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill>{blip}{crop}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
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
    line_props("a:ln", shape.line.as_ref())
}

/// `a:ln`, or a table cell border (`a:lnL` …), which share the format.
fn line_props(tag: &str, line: Option<&Line>) -> String {
    let Some(line) = line else {
        return format!("<{tag}><a:noFill/></{tag}>");
    };
    let cap = match line.cap {
        LineCap::Flat => "flat",
        LineCap::Round => "rnd",
        LineCap::Square => "sq",
    };
    let mut out = format!(
        r#"<{tag} w="{}" cap="{cap}">{}"#,
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
    write!(out, "</{tag}>").unwrap();
    out
}

/// `p:graphicFrame` with an `a:tbl`: no table style, every cell's fill,
/// borders and margins explicit.
fn table_xml(out: &mut String, id: usize, table: &Table, rels: &mut Rels) {
    let Rect { x, y, w, h, .. } = table.frame;
    write!(
        out,
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="{}"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="0" bandRow="0"/><a:tblGrid>"#,
        escape(&table.name)
    )
    .unwrap();
    for column in &table.columns {
        write!(out, r#"<a:gridCol w="{column}"/>"#).unwrap();
    }
    out.push_str("</a:tblGrid>");
    for (row, cells) in table.rows.iter().zip(&table.cells) {
        write!(out, r#"<a:tr h="{row}">"#).unwrap();
        for cell in cells {
            let merge = match cell.merge {
                Merge::Origin { columns, rows } => {
                    let mut attrs = String::new();
                    if columns > 1 {
                        write!(attrs, r#" gridSpan="{columns}""#).unwrap();
                    }
                    if rows > 1 {
                        write!(attrs, r#" rowSpan="{rows}""#).unwrap();
                    }
                    attrs
                }
                Merge::Covered {
                    horizontal,
                    vertical,
                } => format!(
                    "{}{}",
                    if horizontal { r#" hMerge="1""# } else { "" },
                    if vertical { r#" vMerge="1""# } else { "" }
                ),
            };
            write!(out, r#"<a:tc{merge}><a:txBody><a:bodyPr/><a:lstStyle/>"#).unwrap();
            for paragraph in &cell.paragraphs {
                para(out, paragraph, rels);
            }
            let [l, r, t, b] = cell.margins;
            write!(
                out,
                r#"</a:txBody><a:tcPr marL="{l}" marR="{r}" marT="{t}" marB="{b}" anchor="t">"#
            )
            .unwrap();
            for (tag, line) in ["a:lnL", "a:lnR", "a:lnT", "a:lnB"]
                .iter()
                .zip(&cell.borders)
            {
                out.push_str(&line_props(tag, line.as_ref()));
            }
            match &cell.fill {
                Some(fill) => out.push_str(&fill_xml(fill)),
                None => out.push_str("<a:noFill/>"),
            }
            out.push_str("</a:tcPr></a:tc>");
        }
        out.push_str("</a:tr>");
    }
    out.push_str("</a:tbl></a:graphicData></a:graphic></p:graphicFrame>");
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
    let rot = if rect.rot == 0 {
        String::new()
    } else {
        format!(r#" rot="{}""#, rect.rot)
    };
    format!(
        r#"<a:xfrm{rot}{attrs}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        rect.x,
        rect.y,
        rect.w.max(0),
        rect.h.max(0)
    )
}

pub fn solid_fill(c: Color) -> String {
    format!("<a:solidFill>{}</a:solidFill>", color(c))
}

thread_local! {
    /// Theme colours (opaque) and the scheme names that refer to them.
    static SCHEME: RefCell<Vec<(Color, &'static str)>> = const { RefCell::new(Vec::new()) };
}

/// The theme colours `color` refers to while the guard lives (one package
/// write).
pub struct Scheme;

impl Scheme {
    pub fn set(theme: Option<&Theme>) -> Self {
        // dk1/lt1/dk2/lt2 are referred to through the master's colour map.
        const NAMES: [&str; 11] = [
            "tx1", "bg1", "tx2", "bg2", "accent1", "accent2", "accent3", "accent4", "accent5",
            "accent6", "hlink",
        ];
        let scheme = theme
            .map(|t| {
                NAMES
                    .iter()
                    .zip(t.colors)
                    .filter_map(|(name, c)| Some((c?, *name)))
                    .collect()
            })
            .unwrap_or_default();
        SCHEME.with(|s| *s.borrow_mut() = scheme);
        Self
    }
}

impl Drop for Scheme {
    fn drop(&mut self) {
        SCHEME.with(|s| s.borrow_mut().clear());
    }
}

fn color(color: Color) -> String {
    let opaque = Color { a: 255, ..color };
    let name = SCHEME.with(|s| {
        s.borrow()
            .iter()
            .find(|(c, _)| *c == opaque)
            .map(|(_, name)| *name)
    });
    let (tag, attr) = match name {
        Some(name) => ("schemeClr", name.to_string()),
        None => (
            "srgbClr",
            format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b),
        ),
    };
    if color.a == 255 {
        format!(r#"<a:{tag} val="{attr}"/>"#)
    } else {
        // Alpha in thousandths of a percent.
        let alpha = (u32::from(color.a) * 100_000 + 127) / 255;
        format!(r#"<a:{tag} val="{attr}"><a:alpha val="{alpha}"/></a:{tag}>"#)
    }
}
