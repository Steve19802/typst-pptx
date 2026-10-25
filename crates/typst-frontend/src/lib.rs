//! Typst World, compilation, frame walker and tag tracker producing IR per page.

mod download;
pub mod dump;
pub mod ir;
mod svg;
mod truetype;
pub mod walk;
mod world;

use std::io::IsTerminal;

pub use typst::diag::SourceDiagnostic;
use typst::diag::{SourceResult, Warned};
use typst::foundations::{Label, Selector};
use typst::introspection::{Introspector, MetadataElem};
use typst::model::Document;
pub use typst::syntax::Span;
use typst::utils::PicoStr;
use typst::{World, WorldExt};
pub use typst_kit::diagnostics::DiagnosticFormat;
use typst_kit::diagnostics::DiagnosticWorld;
use typst_kit::diagnostics::termcolor::{ColorChoice, StandardStream};
pub use typst_layout::PagedDocument;

pub use crate::world::{ExportWorld, WorldOptions};

/// Compiles the main file to pages.
pub fn compile(world: &ExportWorld) -> Warned<SourceResult<PagedDocument>> {
    typst::compile::<PagedDocument>(world)
}

/// Touying versions the exporter supports (plan §6.1).
const TOUYING_MIN: (u32, u32) = (0, 8);
const TOUYING_TESTED: (u32, u32) = (0, 8);

pub enum TouyingCheck {
    /// No Touying, or a supported version.
    Ok,
    /// Newer than tested: export continues with a warning.
    Untested(String),
    /// Too old: export stops.
    Unsupported(String),
}

/// Checks the Touying version the deck imported (`@preview/touying` only;
/// a deck importing several versions is judged by the oldest).
pub fn check_touying(world: &ExportWorld) -> TouyingCheck {
    let Some(version) = world
        .packages()
        .into_iter()
        .filter(|spec| spec.namespace == "preview" && spec.name == "touying")
        .map(|spec| spec.version)
        .min_by_key(|v| (v.major, v.minor, v.patch))
    else {
        return TouyingCheck::Ok;
    };
    let found = (version.major, version.minor);
    if found < TOUYING_MIN {
        TouyingCheck::Unsupported(format!(
            "typst-pptx needs Touying {}.{} or newer (found {version})",
            TOUYING_MIN.0, TOUYING_MIN.1
        ))
    } else if found > TOUYING_TESTED {
        TouyingCheck::Untested(format!(
            "Touying {version} is newer than the tested {}.{}; slide grouping may be wrong",
            TOUYING_TESTED.0, TOUYING_TESTED.1
        ))
    } else {
        TouyingCheck::Ok
    }
}

/// The value of Touying's `<pdfpc-file>` metadata, if the deck has one.
pub fn pdfpc(doc: &PagedDocument) -> Option<serde_json::Value> {
    let label = Label::new(PicoStr::intern("pdfpc-file"))?;
    let content = doc.introspector().query_label(label).ok()?;
    let metadata = content.to_packed::<MetadataElem>()?;
    serde_json::to_value(&metadata.value).ok()
}

/// The colours declared with `pptx-hints`' `palette` (`<pptx-palette>`
/// metadata, the first one if several), in [`pptx_ir::Theme::SLOTS`] order.
pub fn palette(doc: &PagedDocument) -> [Option<pptx_ir::Color>; 12] {
    let mut slots = [None; 12];
    let Some(label) = Label::new(PicoStr::intern("pptx-palette")) else {
        return slots;
    };
    let found = doc.introspector().query(&Selector::Label(label));
    let Some(value) = found
        .first()
        .and_then(|content| content.to_packed::<MetadataElem>())
        .and_then(|metadata| serde_json::to_value(&metadata.value).ok())
    else {
        return slots;
    };
    let color = |value: Option<&serde_json::Value>| value.and_then(|v| hex_color(v.as_str()?));
    for (slot, key) in [
        (0, "dark"),
        (1, "light"),
        (2, "dark2"),
        (3, "light2"),
        (10, "link"),
        (11, "visited-link"),
    ] {
        slots[slot] = color(value.get(key));
    }
    if let Some(accents) = value.get("accents").and_then(|a| a.as_array()) {
        for (index, accent) in accents.iter().take(6).enumerate() {
            slots[4 + index] = color(Some(accent));
        }
    }
    slots
}

/// `#rrggbb` or `#rrggbbaa` (alpha ignored: scheme colours are opaque).
fn hex_color(text: &str) -> Option<pptx_ir::Color> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
    Some(pptx_ir::Color {
        r: channel(0)?,
        g: channel(2)?,
        b: channel(4)?,
        a: 255,
    })
}

/// A span's file and position, numbered like the typst CLI's short
/// diagnostics (`file:line:column`): 1-based line, column = characters
/// before the position on its line.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

pub fn locate(world: &ExportWorld, span: Span) -> Option<SourceLocation> {
    let id = span.id()?;
    let range = world.range(span)?;
    let source = world.source(id).ok()?;
    let (line, column) = source.lines().byte_to_line_column(range.start)?;
    Some(SourceLocation {
        file: DiagnosticWorld::name(world, id),
        line: line + 1,
        column,
    })
}

/// Title and authors from `set document(..)`.
pub fn properties(doc: &PagedDocument) -> (Option<String>, Option<String>) {
    let info = doc.info();
    let author = (!info.author.is_empty()).then(|| info.author.join(", "));
    (info.title.as_ref().map(|title| title.to_string()), author)
}

/// Prints diagnostics to stderr exactly like the typst CLI (errors first),
/// so tino's `parse_diagnostics` reads them unchanged.
pub fn emit_diagnostics(
    world: &ExportWorld,
    errors: &[SourceDiagnostic],
    warnings: &[SourceDiagnostic],
    format: DiagnosticFormat,
) {
    let color = if std::io::stderr().is_terminal() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    let mut stderr = StandardStream::stderr(color);
    if let Err(err) =
        typst_kit::diagnostics::emit(&mut stderr, world, errors.iter().chain(warnings), format)
    {
        eprintln!("failed to print diagnostics ({err})");
    }
}
