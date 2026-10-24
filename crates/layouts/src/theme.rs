//! The deck's colour scheme and fonts (plan §7), from the paints and fonts it
//! uses: Touying keeps `config-colors` to itself, so the palette is read
//! back from the slides.
//!
//! Colours are counted by use (a run, a fill, a line, a background), in
//! slides and layouts. lt1 is the most common slide background, dk1/dk2 the
//! two most used dark text colours, lt2 the most used other light colour,
//! accent1–6 the most used saturated colours, hlink the colour of linked
//! text. Accents used as fills or lines come first: a theme's palette shows
//! in its shapes, while text-only colours are often syntax highlighting. The minor font is the family with the most characters, the major
//! font the same among heading-sized text.

use std::collections::HashMap;

use pptx_ir::{Color, Deck, Fill, Item, Link, Theme};

/// Chroma ((max − min) / 255) from which a colour is an accent.
const ACCENT_CHROMA: f64 = 0.25;
/// Relative luminance below which a colour is dark, above which light.
const DARK: f64 = 0.2;
const LIGHT: f64 = 0.8;
/// Text from this size (hundredths of a point) is heading-sized.
const HEADING_SIZE: i32 = 2400;

#[derive(Default)]
struct Uses {
    /// Colour → (uses, first use), so ties keep document order.
    text: HashMap<Color, (usize, usize)>,
    paint: HashMap<Color, (usize, usize)>,
    background: HashMap<Color, (usize, usize)>,
    link: HashMap<Color, (usize, usize)>,
    fonts: HashMap<String, (usize, usize)>,
    heading_fonts: HashMap<String, (usize, usize)>,
    order: usize,
}

impl Uses {
    fn add<K: std::hash::Hash + Eq>(
        map: &mut HashMap<K, (usize, usize)>,
        key: K,
        n: usize,
        order: &mut usize,
    ) {
        *order += 1;
        let entry = map.entry(key).or_insert((0, *order));
        entry.0 += n;
    }

    fn items(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::TextBox(text) => {
                    for paragraph in &text.paragraphs {
                        self.paragraph(paragraph, text.link.is_some());
                    }
                }
                Item::Shape(shape) => {
                    self.fill(shape.fill.as_ref());
                    if let Some(line) = &shape.line {
                        Self::add(&mut self.paint, opaque(line.color), 1, &mut self.order);
                    }
                }
                Item::Picture(_) => {}
                Item::Group(group) => self.items(&group.items),
                Item::Table(table) => {
                    for cell in table.cells.iter().flatten() {
                        self.fill(cell.fill.as_ref());
                        for paragraph in &cell.paragraphs {
                            self.paragraph(paragraph, false);
                        }
                    }
                }
            }
        }
    }

    fn fill(&mut self, fill: Option<&Fill>) {
        if let Some(Fill::Solid { color }) = fill {
            Self::add(&mut self.paint, opaque(*color), 1, &mut self.order);
        }
    }

    fn paragraph(&mut self, paragraph: &pptx_ir::Paragraph, linked: bool) {
        for run in paragraph.lines.iter().flatten() {
            let chars = run.text.chars().filter(|c| !c.is_whitespace()).count();
            if chars == 0 {
                continue;
            }
            let color = opaque(run.color);
            let link = linked || matches!(run.link, Some(Link::Url(_)));
            Self::add(
                if link { &mut self.link } else { &mut self.text },
                color,
                1,
                &mut self.order,
            );
            Self::add(
                &mut self.fonts,
                run.font.family.clone(),
                chars,
                &mut self.order,
            );
            if run.size >= HEADING_SIZE {
                Self::add(
                    &mut self.heading_fonts,
                    run.font.family.clone(),
                    chars,
                    &mut self.order,
                );
            }
        }
    }
}

fn opaque(color: Color) -> Color {
    Color { a: 255, ..color }
}

fn chroma(c: Color) -> f64 {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    f64::from(max - min) / 255.0
}

/// WCAG relative luminance.
fn luminance(c: Color) -> f64 {
    let channel = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// Keys by use, most used first, ties in document order.
fn ranked<K: Clone>(map: &HashMap<K, (usize, usize)>) -> Vec<K> {
    let mut entries: Vec<(&K, &(usize, usize))> = map.iter().collect();
    entries.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.1.1.cmp(&b.1.1)));
    entries.into_iter().map(|(k, _)| k.clone()).collect()
}

/// The theme for the deck, or `None` if it has no text or paint.
pub fn theme(deck: &Deck) -> Option<Theme> {
    let mut uses = Uses::default();
    for slide in &deck.slides {
        if let Some(bg) = slide.background {
            Uses::add(&mut uses.background, opaque(bg), 1, &mut uses.order);
        }
        uses.items(&slide.items);
    }
    for (index, layout) in deck.layouts.iter().enumerate() {
        if let Some(bg) = layout.background {
            // A layout stands for its slides.
            let n = deck
                .slides
                .iter()
                .filter(|s| s.layout == Some(index))
                .count()
                .max(1);
            Uses::add(&mut uses.background, opaque(bg), n, &mut uses.order);
        }
        uses.items(&layout.items);
    }
    if uses.text.is_empty() && uses.paint.is_empty() {
        return None;
    }

    let mut colors: [Option<Color>; 12] = [None; 12];
    let mut taken: Vec<Color> = Vec::new();
    let mut set = |slot: usize, color: Option<Color>, taken: &mut Vec<Color>| {
        if let Some(color) = color.filter(|c| !taken.contains(c)) {
            colors[slot] = Some(color);
            taken.push(color);
        }
    };

    let light = |c: &Color| luminance(*c) > LIGHT;
    let dark = |c: &Color| luminance(*c) < DARK && chroma(*c) < ACCENT_CHROMA;
    set(
        1,
        ranked(&uses.background).into_iter().find(light),
        &mut taken,
    );
    let darks: Vec<Color> = ranked(&uses.text).into_iter().filter(dark).collect();
    set(0, darks.first().copied(), &mut taken);
    set(2, darks.get(1).copied(), &mut taken);
    let mut all = uses.paint.clone();
    for (color, (n, order)) in &uses.text {
        let entry = all.entry(*color).or_insert((0, *order));
        entry.0 += n;
        entry.1 = entry.1.min(*order);
    }
    let lights: Vec<Color> = ranked(&all)
        .into_iter()
        .filter(|c| light(c) && !taken.contains(c))
        .collect();
    set(3, lights.first().copied(), &mut taken);
    let painted = ranked(&uses.paint);
    let text_only = ranked(&uses.text)
        .into_iter()
        .filter(|c| !uses.paint.contains_key(c));
    let accents: Vec<Color> = painted
        .into_iter()
        .chain(text_only)
        .filter(|c| chroma(*c) >= ACCENT_CHROMA && !taken.contains(c))
        .take(6)
        .collect();
    for (index, color) in accents.into_iter().enumerate() {
        set(4 + index, Some(color), &mut taken);
    }
    set(10, ranked(&uses.link).into_iter().next(), &mut taken);

    let minor_font = ranked(&uses.fonts).into_iter().next();
    let major_font = ranked(&uses.heading_fonts)
        .into_iter()
        .next()
        .or_else(|| minor_font.clone());
    Some(Theme {
        colors,
        major_font,
        minor_font,
    })
}

#[cfg(test)]
mod tests {
    use pptx_ir::{
        Align, Fidelity, FontRef, Geometry, Paragraph, Rect, Run, Shape, Slide, TextBox,
    };

    use super::*;

    fn rgb(hex: u32) -> Color {
        Color {
            r: (hex >> 16) as u8,
            g: (hex >> 8) as u8,
            b: hex as u8,
            a: 255,
        }
    }

    const FRAME: Rect = Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        rot: 0,
    };

    fn text(family: &str, size: i32, color: u32) -> Item {
        Item::TextBox(TextBox {
            name: String::new(),
            frame: FRAME,
            paragraphs: vec![Paragraph {
                align: Align::Left,
                line_spacing: None,
                lines: vec![vec![Run {
                    text: "Some text".into(),
                    font: FontRef {
                        family: family.into(),
                        bold: false,
                        italic: false,
                    },
                    size,
                    color: rgb(color),
                    spacing: None,
                    link: None,
                    field: None,
                }]],
                indents: None,
                list: None,
                anchor: None,
            }],
            wrap: false,
            link: None,
            fidelity: Fidelity::Native,
            role: None,
        })
    }

    fn shape(color: u32) -> Item {
        Item::Shape(Shape {
            name: String::new(),
            frame: FRAME,
            geometry: Geometry::Rect,
            fill: Some(Fill::Solid { color: rgb(color) }),
            line: None,
            fidelity: Fidelity::Native,
        })
    }

    #[test]
    fn palette_and_fonts_come_from_use() {
        let items = vec![
            text("Heading", 3200, 0x1A2744),
            text("Body", 1800, 0x3D4F6B),
            text("Body", 1800, 0x3D4F6B),
            text("Mono", 1800, 0xD73948),
            text("Mono", 1800, 0xD73948),
            text("Mono", 1800, 0xD73948),
            shape(0x2E5090),
            shape(0x2E5090),
            shape(0xE85D3A),
            shape(0xFFFFFF),
        ];
        let deck = Deck {
            width: 100,
            height: 100,
            slides: vec![Slide {
                page: 0,
                pages: Vec::new(),
                layout: None,
                background: Some(rgb(0xF7F9FC)),
                items,
                notes: Vec::new(),
                clicks: Vec::new(),
                transition: None,
            }],
            media: Vec::new(),
            fonts: Vec::new(),
            layouts: Vec::new(),
            theme: None,
        };
        let theme = theme(&deck).unwrap();
        let c = theme.colors;
        assert_eq!(c[0], Some(rgb(0x3D4F6B)), "dk1: most used dark text");
        assert_eq!(c[1], Some(rgb(0xF7F9FC)), "lt1: background");
        assert_eq!(c[2], Some(rgb(0x1A2744)), "dk2");
        assert_eq!(c[3], Some(rgb(0xFFFFFF)), "lt2: other light paint");
        // Painted accents before the (more frequent) text-only colour.
        assert_eq!(
            &c[4..7],
            &[
                Some(rgb(0x2E5090)),
                Some(rgb(0xE85D3A)),
                Some(rgb(0xD73948))
            ]
        );
        assert_eq!(c[7], None);
        assert_eq!(theme.minor_font.as_deref(), Some("Mono"));
        assert_eq!(theme.major_font.as_deref(), Some("Heading"));
    }
}
