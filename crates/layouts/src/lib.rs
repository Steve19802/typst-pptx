//! Detection of repeated content to build slide layouts (plan §7).
//!
//! Items drawn identically (same content key, frame included) on several
//! slides move to a slide layout shared by those slides: the theme's
//! backgrounds, decorations, footers. Text boxes with a `pptx-hints` role
//! stay on the slide as placeholders; the layout gets an empty placeholder
//! for each role its slides use. A layout is drawn behind the slide,
//! so an item only moves if no item that stays on the slide and lies below
//! it overlaps it, except text: themes place faint decorations over the
//! header (signal-wave's grid is placed in the body, above the title), and
//! PowerPoint draws a layout's decorations below the title anyway. Animated
//! items stay on their slide.
//!
//! Repetition is counted in logical slides: subslides exported as separate
//! slides (Morph splits, `--animations slides`) share their content, which
//! must stay editable on the slides.
//!
//! Slides are grouped by the set of items they would move; a set shared by
//! at least two slides becomes a layout. A slide whose set is unique uses
//! the largest layout whose items it also shows, or none.

pub mod theme;

use std::collections::{HashMap, HashSet};

use pptx_ir::{Align, Deck, Field, Item, Layout, Placeholder, Rect, Role, Slide, item_key};

/// `logical`: per slide, the logical slide it belongs to.
pub fn extract(deck: &mut Deck, logical: &[usize]) {
    let keys: Vec<Vec<String>> = deck
        .slides
        .iter()
        .map(|slide| slide.items.iter().map(|i| item_key(i, true)).collect())
        .collect();
    // Number of logical slides showing each key.
    let mut shown: HashMap<&str, HashSet<usize>> = HashMap::new();
    for (slide, keys) in keys.iter().enumerate() {
        for key in keys {
            shown.entry(key).or_default().insert(logical[slide]);
        }
    }
    let count: HashMap<&str, usize> = shown.into_iter().map(|(k, s)| (k, s.len())).collect();

    // Per slide, the items that could move and their sorted keys.
    let candidates: Vec<Vec<usize>> = deck
        .slides
        .iter()
        .zip(&keys)
        .map(|(slide, keys)| {
            let animated = animated(slide);
            liftable(&slide.items, |i| {
                count[keys[i].as_str()] >= 2
                    && !animated.contains(&i)
                    && role(&slide.items[i]).is_none()
            })
        })
        .collect();
    let sets: Vec<Vec<&str>> = candidates
        .iter()
        .zip(&keys)
        .map(|(lifted, keys)| sorted(lifted.iter().map(|&i| keys[i].as_str())))
        .collect();

    // Layouts: sets shared by two slides or more, in order of first use.
    let mut layouts: Vec<Vec<&str>> = Vec::new();
    for set in &sets {
        if !set.is_empty()
            && !layouts.contains(set)
            && sets.iter().filter(|other| *other == set).count() >= 2
        {
            layouts.push(set.clone());
        }
    }

    // Each slide's layout and the items it moves there.
    let mut chosen: Vec<Option<(usize, Vec<usize>)>> = Vec::new();
    for (index, slide) in deck.slides.iter().enumerate() {
        let set = &sets[index];
        let exact = layouts.iter().position(|l| l == set);
        let choice = match exact {
            Some(layout) => Some((layout, candidates[index].clone())),
            None => best_subset(&layouts, set, slide, &keys[index]),
        };
        chosen.push(choice);
    }

    let mut out: Vec<Layout> = Vec::new();
    let mut first_slide: Vec<usize> = Vec::new();
    for (layout, _) in layouts.iter().enumerate() {
        let users: Vec<usize> = (0..deck.slides.len())
            .filter(|&s| chosen[s].as_ref().is_some_and(|(l, _)| *l == layout))
            .collect();
        let first = users[0];
        let (_, lifted) = chosen[first].as_ref().unwrap();
        let background = deck.slides[first].background;
        let shared = users
            .iter()
            .all(|&s| deck.slides[s].background == background);
        // Each role at its first slide's place.
        let mut placeholders: Vec<Placeholder> = Vec::new();
        for &s in &users {
            for item in &deck.slides[s].items {
                if let (Some(role), Item::TextBox(text)) = (role(item), item)
                    && !placeholders.iter().any(|p| p.role == role)
                {
                    placeholders.push(Placeholder {
                        role,
                        frame: text.frame,
                    });
                }
            }
        }
        out.push(Layout {
            name: format!("Layout {}", layout + 1),
            background: if shared { background } else { None },
            items: lifted
                .iter()
                .map(|&i| deck.slides[first].items[i].clone())
                .collect(),
            placeholders,
        });
        first_slide.push(first);
        if shared {
            for &s in &users {
                deck.slides[s].background = None;
            }
        }
    }

    for (slide, choice) in deck.slides.iter_mut().zip(chosen) {
        if let Some((layout, lifted)) = choice {
            remove(slide, &lifted);
            slide.layout = Some(layout);
        }
    }
    deck.layouts = out;
}

/// Extends an unwrapped box to the slide edges around its alignment point.
fn widen(text: &mut pptx_ir::TextBox, slide_width: i64) {
    let align = text.paragraphs.first().map_or(Align::Left, |p| p.align);
    if text.paragraphs.iter().any(|p| p.align != align) {
        return;
    }
    let frame = &mut text.frame;
    let (left, right) = (frame.x.max(0), (slide_width - frame.x - frame.w).max(0));
    match align {
        Align::Left => frame.w += right,
        Align::Right => {
            frame.x -= left;
            frame.w += left;
        }
        Align::Center => {
            let room = left.min(right);
            frame.x -= room;
            frame.w += 2 * room;
        }
    }
}

fn role(item: &Item) -> Option<Role> {
    match item {
        Item::TextBox(text) => text.role,
        _ => None,
    }
}

/// Role boxes as placeholders: one per role and slide (later ones become
/// plain text boxes), and slide numbers as fields where every slide-number
/// box starts with its slide's position in the deck. With Morph splits or
/// hidden slides, Touying's numbers differ and stay static text.
///
/// Placeholder frames are widened towards the slide edges, keeping the
/// text's alignment point: LibreOffice wraps placeholder text at the frame
/// whatever `wrap="none"` says, and frames fitted to Typst's line widths
/// leave it no room. Unwrapped text doesn't move in PowerPoint.
pub fn roles(deck: &mut Deck) {
    let width = deck.width;
    for slide in &mut deck.slides {
        let mut seen: Vec<Role> = Vec::new();
        for item in &mut slide.items {
            if let Item::TextBox(text) = item
                && let Some(role) = text.role
            {
                if seen.contains(&role) {
                    text.role = None;
                } else {
                    seen.push(role);
                    if !text.wrap && text.frame.rot == 0 {
                        widen(text, width);
                    }
                }
            }
        }
    }
    // The leading number of each slide-number box, if it is the position.
    let mut boxes: Vec<(usize, usize)> = Vec::new();
    for (index, slide) in deck.slides.iter().enumerate() {
        for (i, item) in slide.items.iter().enumerate() {
            if let Item::TextBox(text) = item
                && text.role == Some(Role::SlideNumber)
            {
                let first = text
                    .paragraphs
                    .first()
                    .and_then(|p| p.lines.first()?.first());
                let digits: String = first
                    .map(|run| run.text.chars().take_while(char::is_ascii_digit).collect())
                    .unwrap_or_default();
                if digits.parse::<usize>().ok() != Some(index + 1) {
                    return;
                }
                boxes.push((index, i));
            }
        }
    }
    for (index, i) in boxes {
        let Item::TextBox(text) = &mut deck.slides[index].items[i] else {
            unreachable!()
        };
        let line = &mut text.paragraphs[0].lines[0];
        let digits = (index + 1).to_string();
        let mut number = line[0].clone();
        let rest = number.text.split_off(digits.len());
        number.field = Some(Field::SlideNumber);
        if rest.is_empty() {
            line[0] = number;
        } else {
            line[0].text = rest;
            line.insert(0, number);
        }
    }
}

/// Items targeted by the slide's click animations.
fn animated(slide: &Slide) -> HashSet<usize> {
    slide.clicks.iter().flatten().map(|e| e.item).collect()
}

fn sorted<'a>(keys: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut keys: Vec<&str> = keys.collect();
    keys.sort_unstable();
    keys
}

/// Items, bottom to top, that can move behind the slide: `allowed`, and not
/// overlapped by a lower item that stays, unless that item is text.
fn liftable(items: &[Item], mut allowed: impl FnMut(usize) -> bool) -> Vec<usize> {
    let mut lifted: Vec<usize> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if !allowed(i) {
            continue;
        }
        let blocked = items[..i].iter().enumerate().any(|(j, below)| {
            !lifted.contains(&j)
                && !matches!(below, Item::TextBox(_))
                && overlaps(frame(item), frame(below))
        });
        if !blocked {
            lifted.push(i);
        }
    }
    lifted
}

/// The largest layout whose items the slide shows and can move.
fn best_subset(
    layouts: &[Vec<&str>],
    set: &[&str],
    slide: &Slide,
    keys: &[String],
) -> Option<(usize, Vec<usize>)> {
    let animated = animated(slide);
    let mut best: Option<(usize, Vec<usize>)> = None;
    for (index, layout) in layouts.iter().enumerate() {
        if layout.len() > set.len()
            || best
                .as_ref()
                .is_some_and(|(b, _)| layouts[*b].len() >= layout.len())
        {
            continue;
        }
        // Each layout item once, even if the slide shows it twice.
        let mut wanted: Vec<&str> = layout.clone();
        let lifted = liftable(&slide.items, |i| {
            !animated.contains(&i)
                && match wanted.iter().position(|k| *k == keys[i]) {
                    Some(at) => {
                        wanted.remove(at);
                        true
                    }
                    None => false,
                }
        });
        if sorted(lifted.iter().map(|&i| keys[i].as_str())) == *layout {
            best = Some((index, lifted));
        }
    }
    best
}

/// Removes the items at `indices` (ascending) and renumbers the clicks.
fn remove(slide: &mut Slide, indices: &[usize]) {
    let mut index = 0;
    slide.items.retain(|_| {
        let keep = !indices.contains(&index);
        index += 1;
        keep
    });
    for effect in slide.clicks.iter_mut().flatten() {
        effect.item -= indices.iter().filter(|&&i| i < effect.item).count();
    }
}

fn frame(item: &Item) -> Rect {
    match item {
        Item::TextBox(text) => text.frame,
        Item::Shape(shape) => shape.frame,
        Item::Picture(picture) => picture.frame,
        Item::Group(group) => group.frame,
        Item::Table(table) => table.frame,
    }
}

/// Whether two frames share area (rotated frames by their circumcircle's
/// box).
fn overlaps(a: Rect, b: Rect) -> bool {
    let bounds = |r: Rect| {
        let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
        let (hw, hh) = if r.rot == 0 {
            (r.w / 2, r.h / 2)
        } else {
            let half = ((r.w as f64).hypot(r.h as f64) / 2.0).ceil() as i64;
            (half, half)
        };
        (cx - hw, cy - hh, cx + hw, cy + hh)
    };
    let (ax0, ay0, ax1, ay1) = bounds(a);
    let (bx0, by0, bx1, by1) = bounds(b);
    ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1
}

#[cfg(test)]
mod tests {
    use pptx_ir::{Color, Effect, EffectKind, Fidelity, Geometry};

    use super::*;

    fn rect(x: i64, y: i64, w: i64, h: i64) -> Item {
        Item::Shape(pptx_ir::Shape {
            name: format!("Shape {x}"),
            frame: Rect { x, y, w, h, rot: 0 },
            geometry: Geometry::Rect,
            fill: None,
            line: None,
            fidelity: Fidelity::Native,
        })
    }

    fn text(x: i64, y: i64) -> Item {
        Item::TextBox(pptx_ir::TextBox {
            name: format!("Text {x}"),
            frame: Rect {
                x,
                y,
                w: 10,
                h: 10,
                rot: 0,
            },
            paragraphs: Vec::new(),
            wrap: false,
            link: None,
            fidelity: Fidelity::Native,
            role: None,
        })
    }

    const BG: Color = Color {
        r: 1,
        g: 2,
        b: 3,
        a: 255,
    };

    fn deck_of(slides: Vec<Vec<Item>>) -> Deck {
        deck(slides)
    }

    fn deck(slides: Vec<Vec<Item>>) -> Deck {
        Deck {
            width: 1000,
            height: 1000,
            slides: slides
                .into_iter()
                .enumerate()
                .map(|(page, items)| Slide {
                    page,
                    pages: Vec::new(),
                    layout: None,
                    background: Some(BG),
                    items,
                    notes: Vec::new(),
                    clicks: Vec::new(),
                    transition: None,
                })
                .collect(),
            media: Vec::new(),
            fonts: Vec::new(),
            layouts: Vec::new(),
            theme: None,
        }
    }

    /// A full-page background, a title, content and a footer.
    fn page(content: i64) -> Vec<Item> {
        vec![
            text(content, 0),
            rect(0, 0, 1000, 1000),
            rect(content, 300, 100, 100),
            rect(0, 900, 1000, 50),
        ]
    }

    fn number(slide: i64, text: &str) -> Item {
        let mut item = text_box_with(text, slide * 100);
        if let Item::TextBox(t) = &mut item {
            t.role = Some(Role::SlideNumber);
        }
        item
    }

    fn text_box_with(content: &str, x: i64) -> Item {
        Item::TextBox(pptx_ir::TextBox {
            name: String::new(),
            frame: Rect {
                x,
                y: 900,
                w: 50,
                h: 20,
                rot: 0,
            },
            paragraphs: vec![pptx_ir::Paragraph {
                align: Align::Left,
                line_spacing: None,
                lines: vec![vec![pptx_ir::Run {
                    text: content.into(),
                    font: pptx_ir::FontRef {
                        family: "F".into(),
                        bold: false,
                        italic: false,
                    },
                    size: 1000,
                    color: BG,
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

    fn runs(item: &Item) -> Vec<(String, Option<Field>)> {
        let Item::TextBox(t) = item else { panic!() };
        t.paragraphs[0].lines[0]
            .iter()
            .map(|r| (r.text.clone(), r.field))
            .collect()
    }

    #[test]
    fn slide_numbers_become_fields_where_they_match() {
        let mut deck = deck(vec![vec![number(1, "1 / 2")], vec![number(2, "2")]]);
        roles(&mut deck);
        assert_eq!(
            runs(&deck.slides[0].items[0]),
            [
                ("1".into(), Some(Field::SlideNumber)),
                (" / 2".into(), None)
            ]
        );
        assert_eq!(
            runs(&deck.slides[1].items[0]),
            [("2".into(), Some(Field::SlideNumber))]
        );
        // A frame widened to the right edge: left-aligned text stays.
        let Item::TextBox(t) = &deck.slides[0].items[0] else {
            panic!()
        };
        assert_eq!((t.frame.x, t.frame.w), (100, 900));

        // One number off (a Morph split): all stay static.
        let mut deck = deck_of(vec![vec![number(1, "1")], vec![number(2, "1")]]);
        roles(&mut deck);
        assert_eq!(runs(&deck.slides[1].items[0]), [("1".into(), None)]);
    }

    #[test]
    fn a_role_is_a_placeholder_once_per_slide() {
        let mut deck = deck(vec![vec![number(1, "1"), number(2, "x")]]);
        roles(&mut deck);
        let roles: Vec<_> = deck.slides[0]
            .items
            .iter()
            .map(|i| match i {
                Item::TextBox(t) => t.role,
                _ => None,
            })
            .collect();
        assert_eq!(roles, [Some(Role::SlideNumber), None]);
    }

    #[test]
    fn role_boxes_stay_on_the_slide_and_the_layout_gets_placeholders() {
        let footer = |slide| {
            let mut item = text_box_with("Footer", 10);
            if let Item::TextBox(t) = &mut item {
                t.role = Some(Role::Footer);
            }
            let mut items = page(slide);
            items.push(item);
            items
        };
        let mut deck = deck(vec![footer(100), footer(200)]);
        extract(&mut deck, &[0, 1]);
        assert_eq!(deck.layouts[0].items.len(), 2);
        assert_eq!(deck.slides[0].items.len(), 3);
        assert_eq!(deck.layouts[0].placeholders[0].role, Role::Footer);
    }

    #[test]
    fn repeated_items_move_to_a_shared_layout() {
        let mut deck = deck(vec![page(100), page(200), vec![text(5, 5)]]);
        extract(&mut deck, &[0, 1, 2]);
        assert_eq!(deck.layouts.len(), 1);
        let layout = &deck.layouts[0];
        assert_eq!(layout.background, Some(BG));
        // Background over the title moves (title is text); footer moves.
        assert_eq!(layout.items.len(), 2);
        assert_eq!(deck.slides[0].layout, Some(0));
        assert_eq!(deck.slides[0].items.len(), 2);
        assert_eq!(deck.slides[0].background, None);
        assert_eq!(deck.slides[2].layout, None);
        assert_eq!(deck.slides[2].background, Some(BG));
    }

    #[test]
    fn subslides_of_one_logical_slide_keep_their_content() {
        let mut deck = deck(vec![page(100), page(100)]);
        extract(&mut deck, &[0, 0]);
        assert!(deck.layouts.is_empty());
        assert_eq!(deck.slides[0].items.len(), 4);
    }

    #[test]
    fn items_over_a_shape_that_stays_keep_their_place() {
        // The repeated rect overlaps a content rect drawn below it.
        let slide = |x| vec![rect(x, 0, 500, 500), rect(400, 400, 200, 200)];
        let mut deck = deck(vec![slide(0), slide(100)]);
        extract(&mut deck, &[0, 1]);
        assert!(deck.layouts.is_empty());
    }

    #[test]
    fn animated_items_stay_and_clicks_are_renumbered() {
        let mut deck = deck(vec![page(100), page(200)]);
        for slide in &mut deck.slides {
            slide.clicks = vec![vec![Effect {
                item: 3,
                paragraphs: None,
                kind: EffectKind::Appear,
            }]];
        }
        extract(&mut deck, &[0, 1]);
        // The footer (item 3) is animated: only the background moves.
        assert_eq!(deck.layouts[0].items.len(), 1);
        assert_eq!(deck.slides[0].clicks[0][0].item, 2);
    }

    #[test]
    fn a_slide_with_extra_repeats_uses_the_largest_layout_it_shows() {
        let mut odd = page(300);
        odd.push(rect(900, 0, 50, 50));
        let mut deck = deck(vec![page(100), page(200), odd, vec![rect(900, 0, 50, 50)]]);
        extract(&mut deck, &[0, 1, 2, 3]);
        assert_eq!(deck.layouts.len(), 1);
        assert_eq!(deck.slides[2].layout, Some(0));
        // The extra repeat stays on the slide.
        assert_eq!(deck.slides[2].items.len(), 3);
    }
}
