//! Freezes the shape of Touying 0.8's `<pdfpc-file>` metadata (plan §6.1).
//! `touying-basic.json` is the value for `fixtures/decks/touying-basic.typ`:
//!
//!     typst eval 'query(<pdfpc-file>).first().value' --in fixtures/decks/touying-basic.typ

use touying_adapter::{Error, LogicalSlide, logical_slides};

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("touying-basic.json")).unwrap()
}

#[test]
fn groups_subslides_and_collects_notes() {
    let slides = logical_slides(Some(&fixture()), 6).unwrap();
    let slide = |label: &str, pages: Vec<usize>, notes: &[&str]| LogicalSlide {
        label: label.into(),
        pages,
        notes: notes.iter().map(|n| n.to_string()).collect(),
        hidden: false,
    };
    assert_eq!(
        slides,
        vec![
            slide("1", vec![0], &["Notes on the title slide."]),
            slide(
                "2",
                vec![1, 2, 3],
                &[
                    "First paragraph of the notes.",
                    "Second paragraph of the notes."
                ]
            ),
            slide("3", vec![4, 5], &[]),
        ]
    );
}

#[test]
fn unknown_page_fields_are_rejected() {
    let mut value = fixture();
    value["pages"][0]["subslide"] = 1.into();
    assert!(matches!(
        logical_slides(Some(&value), 6),
        Err(Error::Shape(_))
    ));
}

#[test]
fn page_count_must_match() {
    assert_eq!(
        logical_slides(Some(&fixture()), 5),
        Err(Error::PageCount {
            listed: 6,
            pages: 5
        })
    );
}

#[test]
fn without_touying_every_page_is_a_slide() {
    let slides = logical_slides(None, 2).unwrap();
    assert_eq!(slides.len(), 2);
    assert_eq!(slides[1].pages, vec![1]);
}
