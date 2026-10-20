//! Typst World, compilation, frame walker and tag tracker producing IR per page.

mod download;
pub mod dump;
pub mod ir;
pub mod walk;
mod world;

use std::io::IsTerminal;

use typst::diag::{SourceDiagnostic, SourceResult, Warned};
use typst::foundations::Label;
use typst::introspection::{Introspector, MetadataElem};
use typst::model::Document;
use typst::utils::PicoStr;
pub use typst_kit::diagnostics::DiagnosticFormat;
use typst_kit::diagnostics::termcolor::{ColorChoice, StandardStream};
pub use typst_layout::PagedDocument;

pub use crate::world::{ExportWorld, WorldOptions};

/// Compiles the main file to pages.
pub fn compile(world: &ExportWorld) -> Warned<SourceResult<PagedDocument>> {
    typst::compile::<PagedDocument>(world)
}

/// The value of Touying's `<pdfpc-file>` metadata, if the deck has one.
pub fn pdfpc(doc: &PagedDocument) -> Option<serde_json::Value> {
    let label = Label::new(PicoStr::intern("pdfpc-file"))?;
    let content = doc.introspector().query_label(label).ok()?;
    let metadata = content.to_packed::<MetadataElem>()?;
    serde_json::to_value(&metadata.value).ok()
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
