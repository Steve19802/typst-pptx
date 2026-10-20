//! Snapshot tests over the fixture decks (plan §11): the IR as JSON and the
//! XML parts of the written package. Runs offline: fonts and packages come
//! from `fixtures/` only.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use typst_frontend::{ExportWorld, PagedDocument, WorldOptions};
use typst_pptx::Export;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn compile(deck: &str) -> PagedDocument {
    let root = workspace();
    let options = WorldOptions {
        input: root.join("fixtures/decks").join(deck),
        root: Some(root.clone()),
        font_paths: vec![root.join("fixtures/fonts")],
        ignore_system_fonts: true,
        package_path: Some(root.join("fixtures/packages")),
        // Never reached: everything is in `package_path`.
        package_cache_path: Some(root.join("target/no-package-cache")),
        creation_timestamp: Some(1_767_225_600),
        ..WorldOptions::default()
    };
    let world = ExportWorld::new(&options).unwrap();
    let warned = typst_frontend::compile(&world);
    match warned.output {
        Ok(doc) => doc,
        Err(errors) => panic!("{deck} failed to compile: {errors:?}"),
    }
}

fn export(deck: &str, mode: animate::Mode) -> Export {
    let export = typst_pptx::export(&compile(deck), mode).unwrap();
    assert!(
        export.skipped.is_empty(),
        "{deck}: unmapped items {:?}",
        export.skipped
    );
    export
}

fn write(export: &Export) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    ooxml::write_pptx(&export.deck, &export.properties, &mut out).unwrap();
    out.into_inner()
}

/// Every XML part of the package, one tag per line, in archive order.
fn xml_parts(pptx: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(pptx)).unwrap();
    let mut out = String::new();
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).unwrap();
        let name = file.name().to_string();
        if !name.ends_with(".xml") && !name.ends_with(".rels") {
            out.push_str(&format!("=== {name} ({} bytes)\n", file.size()));
            continue;
        }
        let mut xml = String::new();
        file.read_to_string(&mut xml).unwrap();
        out.push_str(&format!("=== {name}\n{}\n", xml.replace("><", ">\n<")));
    }
    out
}

#[test]
fn features() {
    let export = export("features.typ", animate::Mode::Auto);
    insta::assert_json_snapshot!("features_ir", export.deck);
    let pptx = write(&export);
    assert_eq!(pptx, write(&export), "output is not deterministic");
    insta::assert_snapshot!("features_parts", xml_parts(&pptx));
}

#[test]
fn touying_slides_mode_keeps_every_subslide() {
    let export = export("touying-basic.typ", animate::Mode::Slides);
    let notes: Vec<_> = export.deck.slides.iter().map(|s| s.notes.clone()).collect();
    insta::assert_json_snapshot!("touying_slides_notes", notes);
    insta::assert_json_snapshot!("touying_slides_ir", export.deck);
    insta::assert_snapshot!("touying_slides_parts", xml_parts(&write(&export)));
}

#[test]
fn touying_none_mode_keeps_the_last_subslide() {
    let export = export("touying-basic.typ", animate::Mode::None);
    let notes: Vec<_> = export.deck.slides.iter().map(|s| s.notes.clone()).collect();
    insta::assert_json_snapshot!("touying_none_notes", notes);
}
