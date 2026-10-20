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
}

#[derive(Debug, Clone, Serialize)]
pub struct TextBox {
    /// Stable name, also used for Morph pairing.
    pub name: String,
    pub frame: Rect,
    pub paragraphs: Vec<Paragraph>,
    pub fidelity: Fidelity,
}

#[derive(Debug, Clone, Serialize)]
pub struct Paragraph {
    pub align: Align,
    /// Exact line spacing in hundredths of a point; `None` = single.
    pub line_spacing: Option<i32>,
    /// Lines separated by `a:br`.
    pub lines: Vec<Vec<Run>>,
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
