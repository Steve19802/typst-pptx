//! Subslide diff (plan §6.2): the pages of one logical slide become one or
//! more PowerPoint slides with click animations.
//!
//! Items are matched across pages by content and position: Touying's `hide`
//! cover keeps the layout, so content that stays doesn't move. A text box
//! whose paragraphs are a subset or superset of an earlier version (a list
//! revealed with `#pause`) is the same box, animated by paragraph; box
//! frames can't be compared there, since they are fitted to the visible
//! lines, so paragraphs are compared by content and [`Paragraph::anchor`].
//! A changed paint (alpha cover) is a different item: the old version
//! disappears and the new one appears on the same click.
//!
//! A step where the same content leaves one place and enters another is a
//! layout shift: the slide is split there and the next slide is entered
//! with Morph, persisting items sharing a `!!` name.

use std::collections::{HashMap, HashSet};

use pptx_ir::{Effect, EffectKind, Item, Paragraph, Slide, TextBox, Transition, item_key};

/// One exported slide: the pages it shows and the slide.
pub struct Segment {
    pub pages: Vec<usize>,
    pub slide: Slide,
}

/// The slides for the pages of one logical slide, in order.
pub fn animate(pages: Vec<Slide>) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut start = 0;
    for step in 0..pages.len().saturating_sub(1) {
        if Run::new(&pages[start..=step + 1]).shifts(step - start) {
            segments.push(start..step + 1);
            start = step + 1;
        }
    }
    segments.push(start..pages.len());

    let mut names = Names::default();
    let mut out: Vec<Segment> = Vec::new();
    let mut previous: Option<(Run, Vec<usize>)> = None;
    for (index, range) in segments.into_iter().enumerate() {
        let run = Run::new(&pages[range.clone()]);
        let (mut slide, order) = run.slide();
        if let Some((before, before_order)) = &previous {
            names.pair(before, before_order, &run, &order, index);
            slide.transition = Some(Transition::Morph);
        }
        out.push(Segment {
            pages: range.map(|k| pages[k].page).collect(),
            slide,
        });
        previous = Some((run, order));
    }
    names.apply(&mut out);
    out
}

/// Content keys of a paragraph: with its position (matching) and without
/// (finding moved content). Spacing and indents are left out: they depend
/// on which lines of a list are visible.
fn paragraph_keys(paragraph: &Paragraph) -> (String, String) {
    let content = serde_json::json!({
        "align": paragraph.align,
        "lines": paragraph.lines,
        "level": paragraph.list.as_ref().map(|l| l.level),
        "bullet": paragraph.list.as_ref().map(|l| &l.bullet),
    })
    .to_string();
    let placed = format!("{content}@{:?}", paragraph.anchor);
    (placed, content)
}

/// What an object is made of.
enum Kind {
    /// Any item but a text box, matched whole.
    Whole { key: String, content: String },
    /// A text box: the union of the paragraphs shown, by placed key.
    Text {
        paragraphs: Vec<String>,
        contents: HashMap<String, String>,
    },
}

/// Something shown on some pages of a run.
struct Object {
    kind: Kind,
    /// Per page: the item index, if shown.
    shown: Vec<Option<usize>>,
    /// Text boxes, per page: placed keys of the paragraphs shown.
    visible: Vec<HashSet<String>>,
}

/// Matched pages: consecutive subslides shown on one slide.
struct Run<'a> {
    pages: &'a [Slide],
    objects: Vec<Object>,
}

impl<'a> Run<'a> {
    fn new(pages: &'a [Slide]) -> Self {
        let n = pages.len();
        let mut objects: Vec<Object> = Vec::new();
        for (k, page) in pages.iter().enumerate() {
            let mut taken: HashSet<usize> = HashSet::new();
            // Whole items and unchanged text boxes first, growing text after.
            let mut growing = Vec::new();
            for (index, item) in page.items.iter().enumerate() {
                if let Item::TextBox(text) = item {
                    let keys: Vec<(String, String)> =
                        text.paragraphs.iter().map(paragraph_keys).collect();
                    let set: HashSet<String> = keys.iter().map(|(p, _)| p.clone()).collect();
                    let same = objects.iter().position(|o| {
                        matches!(&o.kind, Kind::Text { paragraphs, .. }
                            if paragraphs.len() == set.len() && paragraphs.iter().all(|p| set.contains(p)))
                            && o.shown[k].is_none()
                    });
                    match same.filter(|o| !taken.contains(o)) {
                        Some(o) => {
                            taken.insert(o);
                            objects[o].shown[k] = Some(index);
                            objects[o].visible[k] = set;
                        }
                        None => growing.push((index, keys, set)),
                    }
                } else {
                    let key = item_key(item, true);
                    let found = objects.iter().enumerate().position(|(o, object)| {
                        !taken.contains(&o)
                            && object.shown[k].is_none()
                            && matches!(&object.kind, Kind::Whole { key: other, .. } if *other == key)
                    });
                    let o = found.unwrap_or_else(|| {
                        objects.push(Object {
                            kind: Kind::Whole {
                                key,
                                content: item_key(item, false),
                            },
                            shown: vec![None; n],
                            visible: vec![HashSet::new(); n],
                        });
                        objects.len() - 1
                    });
                    taken.insert(o);
                    objects[o].shown[k] = Some(index);
                }
            }
            for (index, keys, set) in growing {
                let found = objects.iter().enumerate().position(|(o, object)| {
                    !taken.contains(&o)
                        && object.shown[k].is_none()
                        && match &object.kind {
                            Kind::Text { paragraphs, .. } => {
                                let union: HashSet<&String> = paragraphs.iter().collect();
                                let overlap = set.iter().any(|p| union.contains(p));
                                overlap
                                    && (set.iter().all(|p| union.contains(p))
                                        || union.iter().all(|p| set.contains(*p)))
                            }
                            Kind::Whole { .. } => false,
                        }
                });
                let o = found.unwrap_or_else(|| {
                    objects.push(Object {
                        kind: Kind::Text {
                            paragraphs: Vec::new(),
                            contents: HashMap::new(),
                        },
                        shown: vec![None; n],
                        visible: vec![HashSet::new(); n],
                    });
                    objects.len() - 1
                });
                taken.insert(o);
                let object = &mut objects[o];
                if let Kind::Text {
                    paragraphs,
                    contents,
                } = &mut object.kind
                {
                    if !set.iter().all(|p| paragraphs.contains(p)) {
                        // A superset: its order is the union's.
                        *paragraphs = keys.iter().map(|(p, _)| p.clone()).collect();
                    }
                    for (placed, content) in keys {
                        contents.insert(placed, content);
                    }
                }
                object.shown[k] = Some(index);
                object.visible[k] = set;
            }
        }
        Self { pages, objects }
    }

    /// Whether step `k` → `k + 1` moves content: the same content leaves
    /// one place and enters another.
    fn shifts(&self, k: usize) -> bool {
        let mut left: HashSet<&str> = HashSet::new();
        let mut entered: HashSet<&str> = HashSet::new();
        for object in &self.objects {
            match &object.kind {
                Kind::Whole { content, .. } => match (object.shown[k], object.shown[k + 1]) {
                    (Some(_), None) => {
                        left.insert(content);
                    }
                    (None, Some(_)) => {
                        entered.insert(content);
                    }
                    _ => {}
                },
                Kind::Text { contents, .. } => {
                    for p in object.visible[k].difference(&object.visible[k + 1]) {
                        left.insert(&contents[p]);
                    }
                    for p in object.visible[k + 1].difference(&object.visible[k]) {
                        entered.insert(&contents[p]);
                    }
                }
            }
        }
        !left.is_disjoint(&entered)
    }

    /// The version of an object shown on the slide: the first page's for
    /// whole items, the one with every paragraph for text boxes.
    fn display(&self, object: &Object) -> &Item {
        let page = match &object.kind {
            Kind::Whole { .. } => object.shown.iter().position(Option::is_some),
            Kind::Text { paragraphs, .. } => object
                .visible
                .iter()
                .position(|v| v.len() == paragraphs.len()),
        }
        .expect("an object is shown on some page");
        &self.pages[page].items[object.shown[page].unwrap()]
    }

    /// The slide: every object once, in a stacking order that keeps each
    /// page's, and a click per step. Also returns the object of each item.
    fn slide(&self) -> (Slide, Vec<usize>) {
        let mut order: Vec<usize> = Vec::new();
        for (k, page) in self.pages.iter().enumerate() {
            let mut below: Option<usize> = None;
            for index in 0..page.items.len() {
                let o = self
                    .objects
                    .iter()
                    .position(|object| object.shown[k] == Some(index))
                    .unwrap();
                match order.iter().position(|&x| x == o) {
                    Some(at) => below = Some(at),
                    None => {
                        let at = below.map_or(0, |b| b + 1);
                        order.insert(at, o);
                        below = Some(at);
                    }
                }
            }
        }

        let mut items: Vec<Item> = order
            .iter()
            .map(|&o| self.display(&self.objects[o]).clone())
            .collect();
        if self.pages.len() > 1 {
            renumber(&mut items);
        }
        let mut clicks = Vec::new();
        for k in 0..self.pages.len() - 1 {
            let mut effects = Vec::new();
            for (item, &o) in order.iter().enumerate() {
                self.effects(o, k, item, &items[item], &mut effects);
            }
            if !effects.is_empty() {
                clicks.push(effects);
            }
        }
        let first = &self.pages[0];
        let slide = Slide {
            page: first.page,
            pages: Vec::new(),
            layout: None,
            background: first.background,
            items,
            notes: Vec::new(),
            clicks,
            transition: None,
        };
        (slide, order)
    }

    /// The effects of step `k` → `k + 1` on object `o`, shown as `item`.
    fn effects(&self, o: usize, k: usize, item: usize, shown: &Item, out: &mut Vec<Effect>) {
        let object = &self.objects[o];
        let whole = |kind| Effect {
            item,
            paragraphs: None,
            kind,
        };
        match &object.kind {
            Kind::Whole { .. } => match (object.shown[k], object.shown[k + 1]) {
                (None, Some(_)) => out.push(whole(EffectKind::Appear)),
                (Some(_), None) => out.push(whole(EffectKind::Disappear)),
                _ => {}
            },
            Kind::Text { paragraphs, .. } => {
                let Item::TextBox(text) = shown else {
                    unreachable!()
                };
                // The box's paragraphs are always shown together: animate
                // the whole box.
                let constant = object
                    .visible
                    .iter()
                    .all(|v| v.is_empty() || v.len() == paragraphs.len());
                let (before, after) = (&object.visible[k], &object.visible[k + 1]);
                if constant {
                    match (before.is_empty(), after.is_empty()) {
                        (true, false) => out.push(whole(EffectKind::Appear)),
                        (false, true) => out.push(whole(EffectKind::Disappear)),
                        _ => {}
                    }
                    return;
                }
                let keys = placed_keys(text);
                for (kind, set) in [
                    (EffectKind::Disappear, before.difference(after).collect()),
                    (EffectKind::Appear, after.difference(before).collect()),
                ] {
                    let set: HashSet<&String> = set;
                    for (first, last) in ranges(&keys, &set) {
                        out.push(Effect {
                            item,
                            paragraphs: Some((first, last)),
                            kind,
                        });
                    }
                }
            }
        }
    }
}

/// Gives positional names ("Text 3", numbered per page by the frontend)
/// the item's index on the merged slide, so names stay unique.
fn renumber(items: &mut [Item]) {
    for (index, item) in items.iter_mut().enumerate() {
        let name = match item {
            Item::TextBox(text) => &mut text.name,
            Item::Shape(shape) => &mut shape.name,
            Item::Picture(picture) => &mut picture.name,
            Item::Group(group) => &mut group.name,
            Item::Table(table) => &mut table.name,
        };
        let positional = name.split_once(' ').is_some_and(|(kind, number)| {
            matches!(kind, "Text" | "Shape" | "Picture")
                && !number.is_empty()
                && number.bytes().all(|b| b.is_ascii_digit())
        });
        if positional {
            let kind = name.split_once(' ').unwrap().0.to_string();
            *name = format!("{kind} {index}");
        }
    }
}

fn placed_keys(text: &TextBox) -> Vec<String> {
    text.paragraphs
        .iter()
        .map(|p| paragraph_keys(p).0)
        .collect()
}

/// Contiguous runs of the paragraphs in `set`, as inclusive index ranges.
fn ranges(keys: &[String], set: &HashSet<&String>) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        if !set.contains(key) {
            continue;
        }
        let index = index as u32;
        match out.last_mut() {
            Some((_, last)) if *last + 1 == index => *last = index,
            _ => out.push((index, index)),
        }
    }
    out
}

/// `!!` names pairing items across a Morph transition.
#[derive(Default)]
struct Names {
    /// (segment, item) → name.
    names: HashMap<(usize, usize), String>,
    count: usize,
}

impl Names {
    /// Pairs the items of segment `index - 1` (`before`) with those of
    /// segment `index` that show the same content at the boundary.
    fn pair(
        &mut self,
        before: &Run,
        before_order: &[usize],
        after: &Run,
        order: &[usize],
        index: usize,
    ) {
        let last = before.pages.len() - 1;
        let content = |run: &Run, o: usize, k: usize| -> Option<String> {
            let object = &run.objects[o];
            let item = &run.pages[k].items[object.shown[k]?];
            Some(item_key(item, false))
        };
        let mut used: HashSet<usize> = HashSet::new();
        for (from, &a) in before_order.iter().enumerate() {
            let Some(key) = content(before, a, last) else {
                continue;
            };
            let to = order.iter().enumerate().position(|(to, &b)| {
                !used.contains(&to) && content(after, b, 0).as_deref() == Some(key.as_str())
            });
            if let Some(to) = to {
                used.insert(to);
                let name = match self.names.get(&(index - 1, from)) {
                    Some(name) => name.clone(),
                    None => {
                        self.count += 1;
                        let name = format!("!!Morph {}", self.count);
                        self.names.insert((index - 1, from), name.clone());
                        name
                    }
                };
                self.names.insert((index, to), name);
            }
        }
    }

    fn apply(self, segments: &mut [Segment]) {
        for ((segment, item), name) in self.names {
            match &mut segments[segment].slide.items[item] {
                Item::TextBox(text) => text.name = name,
                Item::Shape(shape) => shape.name = name,
                Item::Picture(picture) => picture.name = name,
                Item::Group(group) => group.name = name,
                Item::Table(table) => table.name = name,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pptx_ir::{Align, Color, Fidelity, FontRef, Rect, Run};

    use super::*;

    const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    fn paragraph(text: &str, y: i64, color: Color) -> Paragraph {
        Paragraph {
            align: Align::Left,
            line_spacing: None,
            lines: vec![vec![Run {
                text: text.into(),
                font: FontRef {
                    family: "F".into(),
                    bold: false,
                    italic: false,
                },
                size: 2000,
                color,
                spacing: None,
                link: None,
            }]],
            indents: None,
            list: None,
            anchor: Some((0, y)),
        }
    }

    /// A text box with one paragraph per (text, baseline) pair; its frame
    /// is fitted to them, as the frontend does.
    fn text(paragraphs: &[(&str, i64)]) -> Item {
        colored(paragraphs, BLACK)
    }

    fn colored(paragraphs: &[(&str, i64)], color: Color) -> Item {
        let y = paragraphs[0].1 - 10;
        let h = paragraphs.last().unwrap().1 - y + 5;
        Item::TextBox(TextBox {
            name: String::new(),
            frame: Rect {
                x: 0,
                y,
                w: 100,
                h,
                rot: 0,
            },
            paragraphs: paragraphs
                .iter()
                .map(|&(t, y)| paragraph(t, y, color))
                .collect(),
            wrap: false,
            link: None,
            fidelity: Fidelity::Native,
        })
    }

    fn page(n: usize, items: Vec<Item>) -> Slide {
        Slide {
            page: n,
            pages: Vec::new(),
            layout: None,
            background: None,
            items,
            notes: Vec::new(),
            clicks: Vec::new(),
            transition: None,
        }
    }

    fn effect(item: usize, paragraphs: Option<(u32, u32)>, kind: EffectKind) -> Effect {
        Effect {
            item,
            paragraphs,
            kind,
        }
    }

    fn texts(slide: &Slide) -> Vec<String> {
        slide
            .items
            .iter()
            .map(|item| match item {
                Item::TextBox(t) => t
                    .paragraphs
                    .iter()
                    .map(|p| p.lines[0][0].text.clone())
                    .collect::<Vec<_>>()
                    .join("+"),
                _ => String::new(),
            })
            .collect()
    }

    use EffectKind::{Appear, Disappear};

    #[test]
    fn pause_reveals_items_on_clicks() {
        let out = animate(vec![
            page(0, vec![text(&[("A", 100)])]),
            page(1, vec![text(&[("A", 100)]), text(&[("B", 200)])]),
            page(
                2,
                vec![
                    text(&[("A", 100)]),
                    text(&[("B", 200)]),
                    text(&[("C", 300)]),
                ],
            ),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pages, [0, 1, 2]);
        let slide = &out[0].slide;
        assert_eq!(texts(slide), ["A", "B", "C"]);
        assert_eq!(
            slide.clicks,
            [vec![effect(1, None, Appear)], vec![effect(2, None, Appear)]]
        );
        assert_eq!(slide.transition, None);
    }

    #[test]
    fn a_growing_list_is_one_box_revealed_by_paragraph() {
        // Frames differ (fitted to the visible lines); baselines don't.
        let out = animate(vec![
            page(0, vec![text(&[("A", 100)])]),
            page(1, vec![text(&[("A", 100), ("B", 200)])]),
            page(
                2,
                vec![text(&[("A", 100), ("B", 200), ("C", 300), ("D", 400)])],
            ),
        ]);
        assert_eq!(out.len(), 1);
        let slide = &out[0].slide;
        assert_eq!(texts(slide), ["A+B+C+D"]);
        assert_eq!(
            slide.clicks,
            [
                vec![effect(0, Some((1, 1)), Appear)],
                vec![effect(0, Some((2, 3)), Appear)]
            ]
        );
    }

    #[test]
    fn alternatives_swap_on_the_same_click() {
        let out = animate(vec![
            page(0, vec![text(&[("First", 100)]), text(&[("Below", 200)])]),
            page(1, vec![text(&[("Second", 100)]), text(&[("Below", 200)])]),
        ]);
        assert_eq!(out.len(), 1);
        let slide = &out[0].slide;
        assert_eq!(texts(slide), ["Second", "First", "Below"]);
        assert_eq!(
            slide.clicks,
            [vec![effect(0, None, Appear), effect(1, None, Disappear)]]
        );
    }

    #[test]
    fn content_shown_again_keeps_its_item() {
        let out = animate(vec![
            page(0, vec![text(&[("T", 50)]), text(&[("A", 100)])]),
            page(1, vec![text(&[("T", 50)])]),
            page(2, vec![text(&[("T", 50)]), text(&[("A", 100)])]),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(texts(&out[0].slide), ["T", "A"]);
        assert_eq!(
            out[0].slide.clicks,
            [
                vec![effect(1, None, Disappear)],
                vec![effect(1, None, Appear)]
            ]
        );
    }

    #[test]
    fn a_changed_paint_swaps_versions() {
        let dim = Color { a: 64, ..BLACK };
        let out = animate(vec![
            page(0, vec![colored(&[("A", 100)], dim)]),
            page(1, vec![text(&[("A", 100)])]),
        ]);
        assert_eq!(out.len(), 1);
        // The new version is inserted below: the lowest item of its page.
        assert_eq!(out[0].slide.items.len(), 2);
        assert_eq!(
            out[0].slide.clicks,
            [vec![effect(0, None, Appear), effect(1, None, Disappear)]]
        );
    }

    #[test]
    fn moved_content_splits_the_slide_with_morph() {
        let out = animate(vec![
            page(0, vec![text(&[("Title", 50)]), text(&[("Moves", 100)])]),
            page(
                1,
                vec![
                    text(&[("Title", 50)]),
                    text(&[("New", 100)]),
                    text(&[("Moves", 200)]),
                ],
            ),
            page(
                2,
                vec![
                    text(&[("Title", 50)]),
                    text(&[("New", 100)]),
                    text(&[("Moves", 200)]),
                    text(&[("Last", 300)]),
                ],
            ),
        ]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].pages, [0]);
        assert_eq!(out[1].pages, [1, 2]);
        assert_eq!(out[1].slide.transition, Some(Transition::Morph));
        assert_eq!(out[1].slide.clicks, [vec![effect(3, None, Appear)]]);
        let names = |slide: &Slide| -> Vec<String> {
            slide
                .items
                .iter()
                .map(|item| match item {
                    Item::TextBox(t) => t.name.clone(),
                    _ => String::new(),
                })
                .collect()
        };
        // Title and the moved paragraph are paired by name.
        assert_eq!(names(&out[0].slide), ["!!Morph 1", "!!Morph 2"]);
        assert_eq!(names(&out[1].slide)[0], "!!Morph 1");
        assert_eq!(names(&out[1].slide)[2], "!!Morph 2");
    }

    #[test]
    fn a_list_item_pushed_down_is_a_shift() {
        let out = animate(vec![
            page(0, vec![text(&[("A", 100), ("C", 200)])]),
            page(1, vec![text(&[("A", 100), ("B", 200), ("C", 300)])]),
        ]);
        assert_eq!(out.len(), 2);
    }
}
