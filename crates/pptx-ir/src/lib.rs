//! Intermediate representation of a deck (slides, items, timeline), dumped as JSON with --emit-ir.
//!
//! Geometry is in EMU (plan §5: 1 pt = 12 700 EMU), already rounded, with the
//! origin at the slide's top left corner and y pointing down.

use serde::Serialize;

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
    /// Exact line spacing in hundredths of a point; `None` = single.
    pub line_spacing: Option<i32>,
    /// Lines separated by `a:br`.
    pub lines: Vec<Vec<Run>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub text: String,
    pub font: FontRef,
    /// Hundredths of a point.
    pub size: i32,
    pub color: Color,
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
    pub fill: Option<Color>,
    pub line: Option<Line>,
    pub fidelity: Fidelity,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub color: Color,
    pub width: Emu,
}
