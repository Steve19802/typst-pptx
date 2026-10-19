//! Typst World, compilation, frame walker and tag tracker producing IR per page.

mod download;
pub mod dump;
pub mod ir;
pub mod walk;
mod world;

use std::io::IsTerminal;

use typst::diag::{SourceDiagnostic, SourceResult, Warned};
pub use typst_kit::diagnostics::DiagnosticFormat;
use typst_kit::diagnostics::termcolor::{ColorChoice, StandardStream};
pub use typst_layout::PagedDocument;

pub use crate::world::{ExportWorld, WorldOptions};

/// Compiles the main file to pages.
pub fn compile(world: &ExportWorld) -> Warned<SourceResult<PagedDocument>> {
    typst::compile::<PagedDocument>(world)
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
