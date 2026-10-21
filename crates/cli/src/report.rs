//! `report.json` (plan §8): per slide, how its items were exported; every
//! baked item and line-mode paragraph with its source position; font
//! embedding results.

use std::collections::BTreeMap;

use pptx_ir::{Fidelity, Item};
use serde::Serialize;
use typst_frontend::{ExportWorld, SourceLocation};

use crate::Export;

#[derive(Serialize)]
pub struct Report {
    pub typst_pptx: &'static str,
    pub animations: &'static str,
    pub text_model: &'static str,
    pub slides: Vec<SlideReport>,
    pub baked: Vec<Entry>,
    pub line_mode: Vec<Entry>,
    /// Tables exported as shapes and text boxes, with the reason.
    pub tables_as_shapes: Vec<Entry>,
    pub fonts: Fonts,
}

#[derive(Serialize)]
pub struct SlideReport {
    /// 1-based position in the .pptx.
    pub slide: usize,
    /// 1-based Typst page.
    pub page: usize,
    /// Touying's logical slide number.
    pub logical: String,
    /// Items by fidelity (text boxes, shapes, pictures; groups counted by
    /// their members).
    pub items: BTreeMap<&'static str, usize>,
    pub notes: bool,
}

#[derive(Serialize)]
pub struct Entry {
    pub slide: usize,
    pub reason: &'static str,
    #[serde(flatten)]
    pub source: Option<SourceLocation>,
}

#[derive(Serialize)]
pub struct Fonts {
    pub embedded: Vec<String>,
    pub not_embedded: Vec<NotEmbedded>,
}

#[derive(Serialize)]
pub struct NotEmbedded {
    pub font: String,
    pub reason: &'static str,
}

pub fn build(export: &Export, world: &ExportWorld) -> Report {
    let slide_of = |page: usize| {
        export
            .deck
            .slides
            .iter()
            .position(|slide| slide.page == page)
            .map(|index| index + 1)
    };
    let slides = export
        .deck
        .slides
        .iter()
        .enumerate()
        .map(|(index, slide)| {
            let mut items = BTreeMap::new();
            count(&slide.items, &mut items);
            SlideReport {
                slide: index + 1,
                page: slide.page + 1,
                logical: export
                    .logical
                    .iter()
                    .find(|logical| logical.pages.contains(&slide.page))
                    .map_or_else(|| (slide.page + 1).to_string(), |l| l.label.clone()),
                items,
                notes: !slide.notes.is_empty(),
            }
        })
        .collect();
    let entry = |page, span, reason| {
        Some(Entry {
            slide: slide_of(page)?,
            reason,
            source: typst_frontend::locate(world, span),
        })
    };
    let report = &export.report;
    Report {
        typst_pptx: env!("CARGO_PKG_VERSION"),
        animations: match export.options.animations {
            animate::Mode::Auto => "auto",
            animate::Mode::None => "none",
            animate::Mode::Slides => "slides",
        },
        text_model: match export.options.text_model {
            typst_frontend::ir::TextModel::PowerPoint => "powerpoint",
            typst_frontend::ir::TextModel::LibreOffice => "libreoffice",
        },
        slides,
        baked: report
            .baked
            .iter()
            .filter_map(|baked| entry(baked.at.page, baked.at.span, baked.reason))
            .collect(),
        line_mode: report
            .line_mode
            .iter()
            .filter_map(|at| entry(at.page, at.span, "line width differs in PowerPoint"))
            .collect(),
        tables_as_shapes: report
            .table_fallbacks
            .iter()
            .filter_map(|f| entry(f.at.page, f.at.span, f.reason))
            .collect(),
        fonts: Fonts {
            embedded: report.fonts_embedded.clone(),
            not_embedded: report
                .fonts_not_embedded
                .iter()
                .map(|(font, reason)| NotEmbedded {
                    font: font.clone(),
                    reason,
                })
                .collect(),
        },
    }
}

fn count(items: &[Item], out: &mut BTreeMap<&'static str, usize>) {
    for item in items {
        let fidelity = match item {
            Item::TextBox(text) => text.fidelity,
            Item::Shape(shape) => shape.fidelity,
            Item::Picture(picture) => picture.fidelity,
            Item::Table(table) => table.fidelity,
            Item::Group(group) => {
                count(&group.items, out);
                continue;
            }
        };
        let key = match fidelity {
            Fidelity::Native => "native",
            Fidelity::Normalized => "normalized",
            Fidelity::Approximate => "approximate",
            Fidelity::Baked => "baked",
        };
        *out.entry(key).or_default() += 1;
    }
}
