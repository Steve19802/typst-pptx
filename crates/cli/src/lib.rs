//! The export pipeline behind `typst-pptx compile`, shared with the fixture
//! tests.

use pptx_ir::Deck;
use typst_frontend::PagedDocument;
use typst_frontend::ir::{Skipped, TextModel};

pub struct Export {
    pub deck: Deck,
    pub properties: ooxml::Properties,
    pub skipped: Skipped,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub animations: animate::Mode,
    /// Which application's text layout the boxes are placed for.
    pub text_model: TextModel,
}

/// Turns a compiled document into the deck that is written to the .pptx.
pub fn export(doc: &PagedDocument, options: Options) -> Result<Export, String> {
    let logical =
        touying_adapter::logical_slides(typst_frontend::pdfpc(doc).as_ref(), doc.pages().len())
            .map_err(|err| err.to_string())?;
    let (mut deck, skipped) = typst_frontend::ir::deck(doc, options.text_model);
    deck.slides = animate::assemble(
        std::mem::take(&mut deck.slides),
        &logical,
        options.animations,
    );
    let (title, author) = typst_frontend::properties(doc);
    Ok(Export {
        deck,
        properties: ooxml::Properties { title, author },
        skipped,
    })
}
