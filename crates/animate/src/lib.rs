//! Subslide diff, click timeline and Morph splitting.
//!
//! Until the diff exists (Phase 4), `auto` behaves like `slides`.

use std::collections::HashMap;

use pptx_ir::{Item, Link, Slide};
use touying_adapter::LogicalSlide;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// Click animations where possible, Morph where the layout shifts.
    #[default]
    Auto,
    /// Only the last subslide of each logical slide.
    None,
    /// Every subslide as its own slide.
    Slides,
}

/// Turns per-page slides into the exported slide list. Notes go on the first
/// exported slide of each logical slide; links to pages become links to the
/// slide showing that page, or else the first slide of its logical slide.
pub fn assemble(pages: Vec<Slide>, logical: &[LogicalSlide], mode: Mode) -> Vec<Slide> {
    let mut pages: Vec<Option<Slide>> = pages.into_iter().map(Some).collect();
    let mut out = Vec::new();
    for slide in logical {
        let selected: Vec<usize> = match mode {
            Mode::None => slide.pages.last().copied().into_iter().collect(),
            Mode::Auto | Mode::Slides => slide.pages.clone(),
        };
        for (position, &page) in selected.iter().enumerate() {
            let Some(mut exported) = pages.get_mut(page).and_then(Option::take) else {
                continue;
            };
            exported.notes = if position == 0 {
                slide.notes.clone()
            } else {
                Vec::new()
            };
            out.push(exported);
        }
    }

    let mut targets: HashMap<usize, usize> = HashMap::new();
    for slide in logical {
        if let Some(first) = out.iter().position(|s| slide.pages.contains(&s.page)) {
            for &page in &slide.pages {
                targets.insert(page, first);
            }
        }
    }
    for (index, slide) in out.iter().enumerate() {
        targets.insert(slide.page, index);
    }
    for slide in &mut out {
        resolve_links(&mut slide.items, &targets);
    }
    out
}

fn resolve_links(items: &mut [Item], targets: &HashMap<usize, usize>) {
    let resolve = |link: &mut Option<Link>| {
        if let Some(Link::Page(page)) = *link {
            *link = targets.get(&page).map(|&slide| Link::Slide(slide));
        }
    };
    for item in items {
        match item {
            Item::TextBox(text) => {
                resolve(&mut text.link);
                for run in text
                    .paragraphs
                    .iter_mut()
                    .flat_map(|p| p.lines.iter_mut())
                    .flatten()
                {
                    resolve(&mut run.link);
                }
            }
            Item::Picture(picture) => resolve(&mut picture.link),
            Item::Group(group) => resolve_links(&mut group.items, targets),
            Item::Shape(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(n: usize) -> Slide {
        Slide {
            page: n,
            background: None,
            items: Vec::new(),
            notes: vec![format!("page {n}")],
        }
    }

    fn logical() -> Vec<LogicalSlide> {
        vec![
            LogicalSlide {
                label: "1".into(),
                pages: vec![0],
                notes: vec!["one".into()],
                hidden: false,
            },
            LogicalSlide {
                label: "2".into(),
                pages: vec![1, 2, 3],
                notes: vec!["two".into()],
                hidden: false,
            },
        ]
    }

    #[test]
    fn none_keeps_the_last_subslide_with_the_notes() {
        let out = assemble((0..4).map(page).collect(), &logical(), Mode::None);
        let notes: Vec<_> = out.iter().map(|s| s.notes.clone()).collect();
        assert_eq!(
            notes,
            vec![vec!["one".to_string()], vec!["two".to_string()]]
        );
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn slides_puts_notes_on_the_first_subslide_only() {
        let out = assemble((0..4).map(page).collect(), &logical(), Mode::Slides);
        let notes: Vec<_> = out.iter().map(|s| s.notes.len()).collect();
        assert_eq!(notes, vec![1, 1, 0, 0]);
    }

    /// Slide 0 links to pages 0–3; returns the resolved targets.
    fn resolved(mode: Mode) -> Vec<Option<Link>> {
        let mut pages: Vec<Slide> = (0..4).map(page).collect();
        pages[0].items = (0..4)
            .map(|target| {
                Item::Picture(pptx_ir::Picture {
                    name: String::new(),
                    frame: pptx_ir::Rect {
                        x: 0,
                        y: 0,
                        w: 1,
                        h: 1,
                        rot: 0,
                    },
                    media: 0,
                    descr: None,
                    crop: None,
                    link: Some(Link::Page(target)),
                    fidelity: pptx_ir::Fidelity::Native,
                })
            })
            .collect();
        let out = assemble(pages, &logical(), mode);
        out[0]
            .items
            .iter()
            .map(|item| match item {
                Item::Picture(picture) => picture.link.clone(),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn page_links_go_to_the_slide_showing_the_page() {
        let slide = |n| Some(Link::Slide(n));
        assert_eq!(
            resolved(Mode::Slides),
            vec![slide(0), slide(1), slide(2), slide(3)]
        );
        // Dropped subslides link to their logical slide.
        assert_eq!(
            resolved(Mode::None),
            vec![slide(0), slide(1), slide(1), slide(1)]
        );
    }
}
