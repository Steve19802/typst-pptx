//! Subslide diff, click timeline and Morph splitting.
//!
//! Until the diff exists (Phase 4), `auto` behaves like `slides`.

use pptx_ir::Slide;
use touying_adapter::LogicalSlide;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Click animations where possible, Morph where the layout shifts.
    Auto,
    /// Only the last subslide of each logical slide.
    None,
    /// Every subslide as its own slide.
    Slides,
}

/// Turns per-page slides into the exported slide list. Notes go on the first
/// exported slide of each logical slide.
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(n: usize) -> Slide {
        Slide {
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
}
