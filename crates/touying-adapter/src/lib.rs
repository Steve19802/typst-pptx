//! Touying 0.8 pdfpc metadata to logical slides, notes and subslide groups.
//!
//! Touying gathers its per-page markers into one `<pdfpc-file>` metadata
//! value (pdfpc format 2). Its shape is frozen in `tests/pdfpc_shape.rs`; an
//! unknown page field is an error, so a format change in a later Touying
//! fails clearly instead of grouping pages wrongly (plan §6.1).

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalSlide {
    /// Touying's logical slide number, as printed on the slide.
    pub label: String,
    /// 0-based page indices of the subslides, in order.
    pub pages: Vec<usize>,
    /// Speaker notes, one string per paragraph.
    pub notes: Vec<String>,
    pub hidden: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("unsupported Touying pdfpc metadata: {0}")]
    Shape(String),
    #[error("Touying pdfpc metadata lists {listed} pages, the document has {pages}")]
    PageCount { listed: usize, pages: usize },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PdfpcFile {
    pdfpc_format: u32,
    pages: Vec<PdfpcPage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PdfpcPage {
    idx: usize,
    label: Label,
    overlay: usize,
    #[allow(dead_code)]
    forced_overlay: bool,
    hidden: bool,
    note: Option<String>,
}

/// Touying writes the label as a string; its default is the integer 1.
#[derive(Deserialize)]
#[serde(untagged)]
enum Label {
    Text(String),
    Number(i64),
}

impl Label {
    fn into_string(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::Number(number) => number.to_string(),
        }
    }
}

/// Groups the pages of a Touying deck into logical slides. Without Touying
/// metadata every page is its own slide.
pub fn logical_slides(
    pdfpc: Option<&serde_json::Value>,
    pages: usize,
) -> Result<Vec<LogicalSlide>, Error> {
    let Some(value) = pdfpc else {
        return Ok((0..pages)
            .map(|page| LogicalSlide {
                label: (page + 1).to_string(),
                pages: vec![page],
                notes: Vec::new(),
                hidden: false,
            })
            .collect());
    };
    let file = PdfpcFile::deserialize(value).map_err(|err| Error::Shape(err.to_string()))?;
    if file.pdfpc_format != 2 {
        return Err(Error::Shape(format!(
            "pdfpcFormat {} (expected 2)",
            file.pdfpc_format
        )));
    }
    if file.pages.len() != pages {
        return Err(Error::PageCount {
            listed: file.pages.len(),
            pages,
        });
    }

    let mut slides: Vec<LogicalSlide> = Vec::new();
    for (index, page) in file.pages.into_iter().enumerate() {
        if page.idx != index {
            return Err(Error::Shape(format!("page {index} has idx {}", page.idx)));
        }
        let label = page.label.into_string();
        let continues = page.overlay > 0 && slides.last().is_some_and(|s| s.label == label);
        if !continues {
            slides.push(LogicalSlide {
                label,
                pages: Vec::new(),
                notes: Vec::new(),
                hidden: page.hidden,
            });
        }
        let slide = slides.last_mut().unwrap();
        slide.pages.push(index);
        if let Some(note) = page.note {
            for paragraph in note.split("\n\n").map(str::trim) {
                if !paragraph.is_empty() && !slide.notes.iter().any(|n| n == paragraph) {
                    slide.notes.push(paragraph.to_string());
                }
            }
        }
    }
    Ok(slides)
}
