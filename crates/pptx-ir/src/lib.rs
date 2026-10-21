//! Intermediate representation of a deck (slides, items, timeline), dumped as JSON with --emit-ir.
//!
//! Geometry is in EMU (plan §5: 1 pt = 12 700 EMU), already rounded, with the
//! origin at the slide's top left corner and y pointing down.

pub mod text;

use serde::{Serialize, Serializer};

pub const EMU_PER_PT: f64 = 12_700.0;

/// English Metric Units.
pub type Emu = i64;

pub fn emu(pt: f64) -> Emu {
    (pt * EMU_PER_PT).round() as Emu
}

#[derive(Debug, Clone, Serialize)]
pub struct Deck {
    pub width: Emu,
    pub height: Emu,
    pub slides: Vec<Slide>,
    /// Picture data, referenced by index from [`Picture::media`].
    pub media: Vec<Media>,
    /// Font faces embedded in the package (plan §5.1).
    pub fonts: Vec<EmbeddedFont>,
}

/// One face of a font family, addressed like the runs that use it.
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddedFont {
    pub font: FontRef,
    /// A single TrueType font (not a collection).
    #[serde(serialize_with = "digest")]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Slide {
    /// The Typst page this slide shows (0-based).
    pub page: usize,
    pub background: Option<Color>,
    pub items: Vec<Item>,
    /// Speaker notes, one string per paragraph.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub x: Emu,
    pub y: Emu,
    pub w: Emu,
    pub h: Emu,
    /// Clockwise rotation about the centre, in 60 000ths of a degree
    /// (0 ≤ rot < 21 600 000).
    #[serde(skip_serializing_if = "is_zero")]
    pub rot: i32,
}

impl Paragraph {
    /// A paragraph without text (empty table cells).
    pub fn empty() -> Self {
        Self {
            align: Align::Left,
            line_spacing: None,
            lines: Vec::new(),
            indents: None,
        }
    }
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

/// sRGB colour with alpha (255 = opaque).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// The ppt-master fidelity vocabulary (plan §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fidelity {
    Native,
    Normalized,
    Approximate,
    Baked,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    TextBox(TextBox),
    Shape(Shape),
    Picture(Picture),
    Group(Group),
    Table(Table),
}

/// A native table (`a:tbl`, plan §5.6): Typst's grid with exact column
/// widths and row heights.
#[derive(Debug, Clone, Serialize)]
pub struct Table {
    pub name: String,
    pub frame: Rect,
    pub columns: Vec<Emu>,
    pub rows: Vec<Emu>,
    /// Row by row, one entry per grid position.
    pub cells: Vec<Vec<TableCell>>,
    pub fidelity: Fidelity,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableCell {
    pub merge: Merge,
    pub fill: Option<Fill>,
    /// Left, right, top, bottom; `None` = no line.
    pub borders: [Option<Line>; 4],
    /// Left, right, top, bottom margins placing the text where Typst put it.
    pub margins: [Emu; 4],
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Merge {
    /// A cell spanning `columns` × `rows` grid positions from here.
    Origin { columns: u32, rows: u32 },
    /// A position covered by a spanning cell to the left and/or above.
    Covered { horizontal: bool, vertical: bool },
}

/// Items moved and selected together (`p:grpSp`, untransformed).
#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub name: String,
    pub frame: Rect,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextBox {
    /// Stable name, also used for Morph pairing.
    pub name: String,
    pub frame: Rect,
    pub paragraphs: Vec<Paragraph>,
    /// Let PowerPoint wrap lines (`--text flow`); otherwise Typst's breaks.
    pub wrap: bool,
    /// A link shared by all the box's text, set on the box instead of the
    /// runs: PowerPoint underlines linked runs whatever their `u`.
    pub link: Option<Link>,
    pub fidelity: Fidelity,
}

#[derive(Debug, Clone, Serialize)]
pub struct Paragraph {
    pub align: Align,
    /// Exact line spacing in hundredths of a point; `None` = single.
    pub line_spacing: Option<i32>,
    /// Lines separated by `a:br`.
    pub lines: Vec<Vec<Run>>,
    /// Left and right indent (`marL`, `marR`), written even when zero:
    /// LibreOffice carries omitted margins over from the paragraph before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indents: Option<(Emu, Emu)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub text: String,
    pub font: FontRef,
    /// Hundredths of a point.
    pub size: i32,
    pub color: Color,
    /// Extra space after each character (Typst tracking), hundredths of a
    /// point.
    pub spacing: Option<i32>,
    pub link: Option<Link>,
}

/// A hyperlink target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "to", content = "target", rename_all = "snake_case")]
pub enum Link {
    Url(String),
    /// A Typst page (0-based); resolved to a slide when slides are chosen.
    Page(usize),
    /// A slide of the deck (0-based).
    Slide(usize),
}

/// A font as PowerPoint addresses it: legacy family name plus style flags.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct FontRef {
    pub family: String,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Shape {
    pub name: String,
    pub frame: Rect,
    pub geometry: Geometry,
    pub fill: Option<Fill>,
    pub line: Option<Line>,
    pub fidelity: Fidelity,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Fill {
    Solid {
        color: Color,
    },
    /// `a:lin` with `scaled="1"`: the angle applies to the shape's unit
    /// square, clockwise from the x axis, in degrees.
    Linear {
        angle: f64,
        stops: Vec<Stop>,
    },
    /// `a:path path="circle"` centred in the shape; stop positions are on
    /// the radius to the corners.
    Radial {
        stops: Vec<Stop>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Stop {
    /// Position in thousandths of a percent (0–100 000).
    pub pos: i32,
    pub color: Color,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Geometry {
    Rect,
    /// Straight line from the frame's top-left to bottom-right corner,
    /// optionally flipped.
    Line {
        flip_h: bool,
        flip_v: bool,
    },
    Ellipse,
    /// Rectangle with the same radius on all four corners.
    RoundRect {
        radius: Emu,
    },
    /// Free-form path; coordinates are relative to the frame's top left.
    Custom {
        commands: Vec<PathCommand>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PathCommand {
    Move { x: Emu, y: Emu },
    Line { x: Emu, y: Emu },
    Cubic { pts: [(Emu, Emu); 3] },
    Close,
}

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub color: Color,
    pub width: Emu,
    pub cap: LineCap,
    pub join: LineJoin,
    /// Dash and gap lengths, alternating, in thousandths of a percent of the
    /// line width (`a:custDash`); empty = solid.
    pub dash: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineCap {
    Flat,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LineJoin {
    /// Miter limit as a ratio of the line width.
    Miter {
        limit: f64,
    },
    Round,
    Bevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct Picture {
    pub name: String,
    pub frame: Rect,
    /// Index into [`Deck::media`].
    pub media: usize,
    /// Alternative text.
    pub descr: Option<String>,
    pub crop: Option<Crop>,
    pub link: Option<Link>,
    pub fidelity: Fidelity,
}

/// Part of the image cut away on each side, in thousandths of a percent of
/// the image size (`a:srcRect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Crop {
    pub l: i32,
    pub t: i32,
    pub r: i32,
    pub b: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Media {
    pub format: MediaFormat,
    #[serde(serialize_with = "digest")]
    pub data: Vec<u8>,
    /// For SVG: index of the PNG rendering older PowerPoint versions show.
    pub fallback: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaFormat {
    Png,
    Jpeg,
    Gif,
    Svg,
}

impl MediaFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Svg => "svg",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Svg => "image/svg+xml",
        }
    }
}

/// Media bytes appear in the IR dump as length and FNV-1a hash only.
fn digest<S: Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    let hash = data.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    serializer.serialize_str(&format!("{} bytes, fnv1a {hash:016x}", data.len()))
}
