//! Baked rendering of unsupported subframes to SVG and PNG.

use typst::foundations::{Content, Smart};
use typst::layout::{Frame, Sides};
use typst::utils::Scalar;
use typst_layout::Page;
use typst_render::RenderOptions;
use typst_svg::SvgOptions;

/// Renders a frame on a transparent background to PNG bytes.
pub fn png(frame: &Frame, pixel_per_pt: f64) -> Vec<u8> {
    let options = RenderOptions {
        pixel_per_pt: Scalar::new(pixel_per_pt),
        render_bleed: false,
    };
    typst_render::render(&page(frame), &options)
        .encode_png()
        .expect("encoding an in-memory pixmap cannot fail")
}

/// Renders a frame on a transparent background to an SVG document.
pub fn svg(frame: &Frame) -> String {
    typst_svg::svg(&page(frame), &SvgOptions::default())
}

fn page(frame: &Frame) -> Page {
    Page {
        frame: frame.clone(),
        bleed: Sides::default(),
        fill: Smart::Custom(None),
        numbering: None,
        supplement: Content::empty(),
        number: 1,
    }
}
