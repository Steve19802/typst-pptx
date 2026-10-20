//! The export pipeline behind `typst-pptx compile`, shared with the fixture
//! tests.

use pptx_ir::Deck;
use typst_frontend::PagedDocument;
use typst_frontend::ir::{Report, TextModel};

pub struct Export {
    pub deck: Deck,
    pub properties: ooxml::Properties,
    pub report: Report,
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub animations: animate::Mode,
    /// Which application's text layout the boxes are placed for.
    pub text_model: TextModel,
    pub embed_fonts: bool,
    pub svg_max_nodes: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            animations: animate::Mode::default(),
            text_model: TextModel::default(),
            embed_fonts: true,
            svg_max_nodes: 60,
        }
    }
}

/// Turns a compiled document into the deck that is written to the .pptx.
pub fn export(doc: &PagedDocument, options: Options) -> Result<Export, String> {
    let logical =
        touying_adapter::logical_slides(typst_frontend::pdfpc(doc).as_ref(), doc.pages().len())
            .map_err(|err| err.to_string())?;
    let ir_options = typst_frontend::ir::Options {
        text_model: options.text_model,
        embed_fonts: options.embed_fonts,
        svg_max_nodes: options.svg_max_nodes,
    };
    let (mut deck, report) = typst_frontend::ir::deck(doc, ir_options);
    deck.slides = animate::assemble(
        std::mem::take(&mut deck.slides),
        &logical,
        options.animations,
    );
    let (title, author) = typst_frontend::properties(doc);
    Ok(Export {
        deck,
        properties: ooxml::Properties { title, author },
        report,
    })
}
