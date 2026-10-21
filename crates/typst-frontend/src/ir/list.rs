//! Lists as one text box with native bullets and numbers (plan Phase 3).
//!
//! Typst tags each list item's marker (`ListItemLabel`) and body
//! (`ListItemBody`). All text of an outermost `list`/`enum` is collected
//! with its item and nesting level; every item becomes a paragraph whose
//! left margin is the body's x and whose hanging indent puts PowerPoint's
//! bullet where Typst drew the marker. Gaps between items become space
//! before, which also absorbs the application's line-spacing rounding.

use std::collections::HashMap;

use pptx_ir::{Align, Bullet, BulletKind, Fidelity, ListProps, Paragraph, Rect, TextBox, emu};
use typst::foundations::Content;
use typst::introspection::Location;
use typst::layout::Abs;
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};

use super::{LineSeg, Piece, SAME_PITCH, TextModel, hoist_link, lines};

/// Where a text piece sits in a list.
#[derive(Debug, Clone, Copy)]
pub(super) struct ListRole {
    /// Item number within the outermost list, in tag order.
    pub item: usize,
    /// 1 for the outermost list.
    pub level: usize,
    /// Part of the marker rather than the body.
    pub label: bool,
    pub numbered: bool,
}

/// Item numbering while walking a page: the current item of each list
/// level, keyed by the outermost list.
#[derive(Default)]
pub(super) struct ListItems {
    current: HashMap<(Location, usize), usize>,
    next: HashMap<Location, usize>,
}

fn is_list(elem: &Content) -> bool {
    matches!(elem.elem().name(), "list" | "enum")
}

fn marker_kind(elem: &Content) -> Option<bool> {
    let tag = elem.to_packed::<PdfMarkerTag>()?;
    match tag.kind {
        PdfMarkerTagKind::ListItemLabel => Some(true),
        PdfMarkerTagKind::ListItemBody => Some(false),
        _ => None,
    }
}

impl ListItems {
    /// Counts a new item when its marker starts.
    pub fn tag_start(&mut self, tags: &[&Content], elem: &Content) {
        if marker_kind(elem) != Some(true) {
            return;
        }
        let Some(root) = tags.iter().find(|e| is_list(e)).and_then(|e| e.location()) else {
            return;
        };
        let level = tags.iter().filter(|e| is_list(e)).count();
        let next = self.next.entry(root).or_default();
        self.current.insert((root, level), *next);
        *next += 1;
    }

    /// The outermost list and the role of text with these open elements.
    pub fn role(&self, tags: &[&Content]) -> Option<(Location, Option<ListRole>)> {
        let root = tags.iter().find(|e| is_list(e))?.location()?;
        let level = tags.iter().filter(|e| is_list(e)).count();
        let numbered = tags
            .iter()
            .rev()
            .find(|e| is_list(e))
            .is_some_and(|e| e.elem().name() == "enum");
        let label = tags.iter().rev().find_map(|e| marker_kind(e));
        let role = match (label, self.current.get(&(root, level))) {
            (Some(label), Some(&item)) => Some(ListRole {
                item,
                level,
                label,
                numbered,
            }),
            _ => None,
        };
        Some((root, role))
    }
}

/// `buAutoNum` scheme and value of a Typst enum marker like "3." or "b)".
fn numbering(marker: &str) -> Option<(&'static str, u32)> {
    let (open, rest) = match marker.strip_prefix('(') {
        Some(rest) => (true, rest),
        None => (false, marker),
    };
    let (core, close) = rest.split_at(rest.len().checked_sub(1)?);
    let (kind, value) = if core.chars().all(|c| c.is_ascii_digit()) && !core.is_empty() {
        ("arabic", core.parse().ok()?)
    } else if core.len() == 1 && core.chars().all(|c| c.is_ascii_lowercase()) {
        ("alphaLc", u32::from(core.as_bytes()[0] - b'a') + 1)
    } else if core.len() == 1 && core.chars().all(|c| c.is_ascii_uppercase()) {
        ("alphaUc", u32::from(core.as_bytes()[0] - b'A') + 1)
    } else {
        return None;
    };
    let scheme = match (kind, open, close) {
        ("arabic", false, ".") => "arabicPeriod",
        ("arabic", false, ")") => "arabicParenR",
        ("arabic", true, ")") => "arabicParenBoth",
        ("alphaLc", false, ".") => "alphaLcPeriod",
        ("alphaLc", false, ")") => "alphaLcParenR",
        ("alphaLc", true, ")") => "alphaLcParenBoth",
        ("alphaUc", false, ".") => "alphaUcPeriod",
        ("alphaUc", false, ")") => "alphaUcParenR",
        ("alphaUc", true, ")") => "alphaUcParenBoth",
        _ => return None,
    };
    Some((scheme, value))
}

struct ListItem {
    level: usize,
    marker_x: Abs,
    bullet: Bullet,
    /// For numbers: the value shown.
    number: Option<u32>,
    body: Vec<LineSeg>,
}

/// One text box for a whole list, or `None` if it doesn't fit the model
/// (the caller maps the pieces as plain text then).
pub(super) fn list_box(name: String, pieces: Vec<Piece>, model: TextModel) -> Option<TextBox> {
    let mut labels: Vec<(usize, ListRole, Vec<Piece>)> = Vec::new();
    let mut bodies: HashMap<usize, Vec<Piece>> = HashMap::new();
    for piece in pieces {
        let role = piece.list?;
        if role.label {
            match labels.iter_mut().find(|(item, ..)| *item == role.item) {
                Some((.., label)) => label.push(piece),
                None => labels.push((role.item, role, vec![piece])),
            }
        } else {
            bodies.entry(role.item).or_default().push(piece);
        }
    }

    let mut items = Vec::new();
    for (item, role, label) in labels {
        let marker = lines(label);
        let [marker] = marker.as_slice() else {
            return None;
        };
        let text: String = marker.runs.iter().map(|r| r.text.as_str()).collect();
        let text = text.trim();
        let run = marker.runs.first()?;
        let (kind, number) = if role.numbered {
            let (scheme, value) = numbering(text)?;
            (
                BulletKind::Number {
                    scheme: scheme.into(),
                    start: None,
                },
                Some(value),
            )
        } else {
            if text.chars().count() != 1 {
                return None;
            }
            (BulletKind::Char { char: text.into() }, None)
        };
        let body = lines(bodies.remove(&item)?);
        let first = body.first()?;
        // The marker sits on the body's first line, left of it.
        if (first.y - marker.y).to_pt().abs() > 0.05 || marker.x1 > first.x0 {
            return None;
        }
        items.push(ListItem {
            level: role.level,
            marker_x: marker.x0,
            bullet: Bullet {
                kind,
                font: run.font.family.clone(),
                size: run.size,
                color: run.color,
            },
            number,
            body,
        });
    }
    if items.is_empty() || !bodies.is_empty() {
        return None;
    }

    // Numbers: PowerPoint counts on through paragraphs of a level; each
    // run of numbered items starts at its first value and must count up.
    let mut expected: HashMap<usize, u32> = HashMap::new();
    for item in &mut items {
        let Some(value) = item.number else { continue };
        let BulletKind::Number { start, .. } = &mut item.bullet.kind else {
            continue;
        };
        match expected.get(&item.level) {
            Some(&next) if next == value => {}
            Some(_) => return None,
            None => *start = (value != 1).then_some(value),
        }
        expected.insert(item.level, value + 1);
    }

    // All lines top to bottom, with their item.
    let mut all: Vec<(usize, LineSeg, bool)> = Vec::new();
    for (index, item) in items.iter_mut().enumerate() {
        for (k, line) in std::mem::take(&mut item.body).into_iter().enumerate() {
            all.push((index, line, k == 0));
        }
    }
    all.sort_by(|a, b| a.1.y.partial_cmp(&b.1.y).unwrap());
    if all.windows(2).any(|w| w[1].1.y <= w[0].1.y) {
        return None;
    }
    let size = all[0].1.max_size();
    if !model.mixed_line_sizes() && all.iter().any(|(_, line, _)| line.max_size() != size) {
        return None;
    }
    // Line pitch: the smallest gap; larger gaps are space before.
    let gaps: Vec<f64> = all
        .windows(2)
        .map(|w| (w[1].1.y - w[0].1.y).to_pt())
        .collect();
    let pitch = gaps.iter().copied().fold(f64::INFINITY, f64::min);
    let spacing = pitch.is_finite().then(|| model.line_spacing(pitch));

    let x0 = items
        .iter()
        .map(|i| i.marker_x)
        .chain(all.iter().map(|(_, l, _)| l.x0))
        .fold(Abs::inf(), Abs::min);
    let x1 = all.iter().map(|(_, l, _)| l.x1).fold(-Abs::inf(), Abs::max);
    let first_y = all[0].1.y.to_pt();
    let top = first_y - model.first_baseline(size, spacing);

    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut predicted = first_y;
    // Drawn minus Typst baseline, per line.
    let mut errors: Vec<f64> = Vec::new();
    let mut height = spacing.unwrap_or(size * pptx_ir::text::SINGLE_LINE);
    for (k, (index, line, first_of_item)) in all.into_iter().enumerate() {
        let item = &items[index];
        let body_x = line.x0;
        let continues = k > 0 && !first_of_item && {
            let last = paragraphs.last().unwrap();
            let gap = gaps[k - 1];
            (gap - pitch).abs() < SAME_PITCH
                && last.indents.map(|(l, _)| l) == Some(emu((body_x - x0).to_pt()))
        };
        if continues {
            paragraphs.last_mut().unwrap().lines.push(line.runs);
            predicted += spacing.unwrap_or(0.0);
            height += spacing.unwrap_or(0.0);
            errors.push(predicted - line.y.to_pt());
            continue;
        }
        let before = if k == 0 {
            0.0
        } else {
            line.y.to_pt() - (predicted + spacing.unwrap_or(0.0))
        };
        // Rounded spacing can make a gap slightly negative; absorb it.
        if before < -0.5 {
            return None;
        }
        // The application's rounding is carried: the next gap makes up
        // for it, since `predicted` follows what it will draw.
        let drawn_before = model.space_before(before.max(0.0));
        let before = (drawn_before * 100.0).round() as i32;
        if k > 0 {
            predicted += spacing.unwrap_or(0.0) + drawn_before;
            height += spacing.unwrap_or(0.0) + drawn_before;
        }
        errors.push(predicted - line.y.to_pt());
        let (indent, bullet) = if first_of_item {
            (
                emu((item.marker_x - body_x).to_pt()),
                Some(item.bullet.clone()),
            )
        } else {
            (0, None)
        };
        paragraphs.push(Paragraph {
            align: Align::Left,
            line_spacing: spacing.map(|s| (s * 100.0).round() as i32),
            lines: vec![line.runs],
            indents: Some((emu((body_x - x0).to_pt()), 0)),
            list: Some(ListProps {
                level: (item.level - 1) as u32,
                indent,
                space_before: before,
                bullet,
            }),
        });
    }

    // Split what's left of the error between the first and last lines.
    let (low, high) = errors
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &e| {
            (lo.min(e), hi.max(e))
        });
    let top = top - (low + high) / 2.0;
    let x = emu(x0.to_pt());
    let y = emu(top);
    Some(TextBox {
        name,
        frame: Rect {
            x,
            y,
            w: emu(x1.to_pt()) - x,
            h: emu(top + height) - y,
            rot: 0,
        },
        link: hoist_link(&mut paragraphs),
        paragraphs,
        wrap: false,
        fidelity: Fidelity::Native,
    })
}
