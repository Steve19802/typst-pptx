//! PresentationML/DrawingML writer and font embedding.

mod fonts;
mod notes;
mod slide;
mod theme;
mod xml;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::{self, Seek, Write};

use pptx_ir::Deck;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::xml::{
    DECLARATION, REL_APP_PROPS, REL_CORE_PROPS, REL_FONT, REL_IMAGE, REL_NOTES_MASTER,
    REL_NOTES_SLIDE, REL_OFFICE_DOCUMENT, REL_PRES_PROPS, REL_SLIDE, REL_SLIDE_LAYOUT,
    REL_SLIDE_MASTER, REL_TABLE_STYLES, REL_THEME, REL_VIEW_PROPS, Rel, escape, pml_namespaces,
    rels,
};

/// Document properties written to `docProps/core.xml`.
#[derive(Debug, Clone, Default)]
pub struct Properties {
    pub title: Option<String>,
    pub author: Option<String>,
}

const PML: &str = "application/vnd.openxmlformats-officedocument.presentationml";
const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";

/// Parts in write order plus the content-type overrides they need.
struct Package {
    parts: Vec<(String, Vec<u8>)>,
    overrides: Vec<(String, String)>,
    extensions: BTreeSet<(&'static str, &'static str)>,
}

impl Package {
    fn xml(&mut self, name: &str, content_type: Option<String>, data: String) {
        if let Some(content_type) = content_type {
            self.overrides.push((format!("/{name}"), content_type));
        }
        self.parts.push((name.into(), data.into_bytes()));
    }
}

/// Writes the deck as a `.pptx` package. Output is deterministic: parts in a
/// fixed order with fixed timestamps (plan §8).
pub fn write_pptx<W: Write + Seek>(deck: &Deck, props: &Properties, out: W) -> io::Result<()> {
    let mut pkg = Package {
        parts: Vec::new(),
        overrides: Vec::new(),
        extensions: BTreeSet::new(),
    };
    let n = deck.slides.len();
    let has_notes = deck.slides.iter().any(|s| !s.notes.is_empty());

    pkg.xml(
        "_rels/.rels",
        None,
        rels(&[
            rel(1, REL_OFFICE_DOCUMENT, "ppt/presentation.xml"),
            rel(2, REL_CORE_PROPS, "docProps/core.xml"),
            rel(3, REL_APP_PROPS, "docProps/app.xml"),
        ]),
    );
    pkg.xml(
        "docProps/core.xml",
        Some("application/vnd.openxmlformats-package.core-properties+xml".into()),
        core_props(props),
    );
    pkg.xml(
        "docProps/app.xml",
        Some("application/vnd.openxmlformats-officedocument.extended-properties+xml".into()),
        app_props(n),
    );
    let families = fonts::families(&deck.fonts);
    let pres_rels = PresentationRels::new(n, has_notes, deck.fonts.len());
    pkg.xml(
        "ppt/presentation.xml",
        Some(format!("{PML}.presentation.main+xml")),
        presentation(deck, &pres_rels, &families),
    );
    pkg.xml("ppt/_rels/presentation.xml.rels", None, pres_rels.xml());
    pkg.xml(
        "ppt/presProps.xml",
        Some(format!("{PML}.presProps+xml")),
        format!("{DECLARATION}<p:presentationPr {}/>", pml_namespaces()),
    );
    pkg.xml(
        "ppt/viewProps.xml",
        Some(format!("{PML}.viewProps+xml")),
        format!("{DECLARATION}<p:viewPr {}/>", pml_namespaces()),
    );
    pkg.xml(
        "ppt/tableStyles.xml",
        Some(format!("{PML}.tableStyles+xml")),
        format!(
            r#"{DECLARATION}<a:tblStyleLst xmlns:a="{}" def="{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}"/>"#,
            xml::NS_A
        ),
    );
    pkg.xml(
        "ppt/theme/theme1.xml",
        Some(CT_THEME.into()),
        theme::theme(),
    );
    pkg.xml(
        "ppt/slideMasters/slideMaster1.xml",
        Some(format!("{PML}.slideMaster+xml")),
        slide_master(),
    );
    pkg.xml(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        None,
        rels(&[
            rel(1, REL_SLIDE_LAYOUT, "../slideLayouts/slideLayout1.xml"),
            rel(2, REL_THEME, "../theme/theme1.xml"),
        ]),
    );
    pkg.xml(
        "ppt/slideLayouts/slideLayout1.xml",
        Some(format!("{PML}.slideLayout+xml")),
        slide_layout(),
    );
    pkg.xml(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        None,
        rels(&[rel(1, REL_SLIDE_MASTER, "../slideMasters/slideMaster1.xml")]),
    );
    if has_notes {
        pkg.xml(
            "ppt/theme/theme2.xml",
            Some(CT_THEME.into()),
            theme::theme(),
        );
        pkg.xml(
            "ppt/notesMasters/notesMaster1.xml",
            Some(format!("{PML}.notesMaster+xml")),
            notes::master(),
        );
        pkg.xml(
            "ppt/notesMasters/_rels/notesMaster1.xml.rels",
            None,
            rels(&[rel(1, REL_THEME, "../theme/theme2.xml")]),
        );
    }

    let mut notes_count = 0;
    for (index, s) in deck.slides.iter().enumerate() {
        let number = index + 1;
        // rId1 = layout; pictures from rId2; notes slide last.
        let mut slide_rels = slide::Rels::new(2);
        let xml = slide::slide(s, &deck.media, &mut slide_rels);
        let mut list = vec![rel(1, REL_SLIDE_LAYOUT, "../slideLayouts/slideLayout1.xml")];
        for (id, media) in slide_rels.media() {
            list.push(Rel {
                id: format!("rId{id}"),
                kind: REL_IMAGE,
                target: format!("../media/{}", media_name(deck, media)),
            });
        }
        if !s.notes.is_empty() {
            notes_count += 1;
            list.push(rel(
                slide_rels.next_id(),
                REL_NOTES_SLIDE,
                &format!("../notesSlides/notesSlide{notes_count}.xml"),
            ));
            pkg.xml(
                &format!("ppt/notesSlides/notesSlide{notes_count}.xml"),
                Some(format!("{PML}.notesSlide+xml")),
                notes::slide(&s.notes),
            );
            pkg.xml(
                &format!("ppt/notesSlides/_rels/notesSlide{notes_count}.xml.rels"),
                None,
                rels(&[
                    rel(1, REL_NOTES_MASTER, "../notesMasters/notesMaster1.xml"),
                    rel(2, REL_SLIDE, &format!("../slides/slide{number}.xml")),
                ]),
            );
        }
        pkg.xml(
            &format!("ppt/slides/slide{number}.xml"),
            Some(format!("{PML}.slide+xml")),
            xml,
        );
        pkg.xml(
            &format!("ppt/slides/_rels/slide{number}.xml.rels"),
            None,
            rels(&list),
        );
    }
    for (index, font) in deck.fonts.iter().enumerate() {
        let data = fonts::eot(&font.data, font.font.italic)
            .ok_or_else(|| io::Error::other(format!("cannot read font {}", font.font.family)))?;
        pkg.extensions
            .insert((fonts::EXTENSION, fonts::CONTENT_TYPE));
        pkg.parts.push((
            format!("ppt/fonts/font{}.{}", index + 1, fonts::EXTENSION),
            data,
        ));
    }
    for (index, media) in deck.media.iter().enumerate() {
        pkg.extensions
            .insert((media.format.extension(), media.format.content_type()));
        pkg.parts.push((
            format!("ppt/media/{}", media_name(deck, index)),
            media.data.clone(),
        ));
    }

    let mut zip = ZipWriter::new(out);
    let deflated = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default());
    let stored = deflated.compression_method(CompressionMethod::Stored);
    zip.start_file("[Content_Types].xml", deflated)?;
    zip.write_all(content_types(&pkg).as_bytes())?;
    for (name, data) in &pkg.parts {
        // Compressed image formats gain nothing from deflate.
        let options = if name.ends_with(".png") || name.ends_with(".jpeg") || name.ends_with(".gif")
        {
            stored
        } else {
            deflated
        };
        zip.start_file(name.as_str(), options)?;
        zip.write_all(data)?;
    }
    zip.finish()?;
    Ok(())
}

fn media_name(deck: &Deck, index: usize) -> String {
    format!(
        "image{}.{}",
        index + 1,
        deck.media[index].format.extension()
    )
}

fn rel(id: usize, kind: &'static str, target: &str) -> Rel {
    Rel {
        id: format!("rId{id}"),
        kind,
        target: target.into(),
    }
}

fn content_types(pkg: &Package) -> String {
    let mut out = format!(
        r#"{DECLARATION}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#
    );
    for (extension, content_type) in &pkg.extensions {
        write!(
            out,
            r#"<Default Extension="{extension}" ContentType="{content_type}"/>"#
        )
        .unwrap();
    }
    for (part, content_type) in &pkg.overrides {
        write!(
            out,
            r#"<Override PartName="{part}" ContentType="{content_type}"/>"#
        )
        .unwrap();
    }
    out.push_str("</Types>");
    out
}

/// Relationship ids of `presentation.xml`: rId1 = slide master, rId2.. =
/// slides, then the notes master (if any), theme and property parts, then
/// the embedded fonts.
struct PresentationRels {
    slides: usize,
    notes_master: Option<usize>,
    fonts: usize,
}

impl PresentationRels {
    fn new(slides: usize, has_notes: bool, fonts: usize) -> Self {
        Self {
            slides,
            notes_master: has_notes.then_some(slides + 2),
            fonts,
        }
    }

    fn font(&self, index: usize) -> usize {
        self.slides + 2 + usize::from(self.notes_master.is_some()) + 4 + index
    }

    fn slide(&self, index: usize) -> usize {
        index + 2
    }

    fn xml(&self) -> String {
        let mut list = vec![rel(1, REL_SLIDE_MASTER, "slideMasters/slideMaster1.xml")];
        for index in 0..self.slides {
            list.push(rel(
                self.slide(index),
                REL_SLIDE,
                &format!("slides/slide{}.xml", index + 1),
            ));
        }
        let mut next = self.slides + 2;
        if let Some(id) = self.notes_master {
            list.push(rel(id, REL_NOTES_MASTER, "notesMasters/notesMaster1.xml"));
            next += 1;
        }
        list.push(rel(next, REL_THEME, "theme/theme1.xml"));
        list.push(rel(next + 1, REL_PRES_PROPS, "presProps.xml"));
        list.push(rel(next + 2, REL_VIEW_PROPS, "viewProps.xml"));
        list.push(rel(next + 3, REL_TABLE_STYLES, "tableStyles.xml"));
        for index in 0..self.fonts {
            list.push(rel(
                self.font(index),
                REL_FONT,
                &format!("fonts/font{}.{}", index + 1, fonts::EXTENSION),
            ));
        }
        rels(&list)
    }
}

fn presentation(deck: &Deck, ids: &PresentationRels, families: &[fonts::Family]) -> String {
    let embed = if families.is_empty() {
        ""
    } else {
        r#" embedTrueTypeFonts="1""#
    };
    let mut out = format!(
        r#"{DECLARATION}<p:presentation {} saveSubsetFonts="1"{embed}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>"#,
        pml_namespaces()
    );
    if let Some(id) = ids.notes_master {
        write!(
            out,
            r#"<p:notesMasterIdLst><p:notesMasterId r:id="rId{id}"/></p:notesMasterIdLst>"#
        )
        .unwrap();
    }
    out.push_str("<p:sldIdLst>");
    for index in 0..deck.slides.len() {
        write!(
            out,
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            256 + index,
            ids.slide(index)
        )
        .unwrap();
    }
    write!(
        out,
        r#"</p:sldIdLst><p:sldSz cx="{}" cy="{}"/><p:notesSz cx="6858000" cy="9144000"/>"#,
        deck.width, deck.height
    )
    .unwrap();
    if !families.is_empty() {
        out.push_str(&fonts::list(families, |face| ids.font(face)));
    }
    out.push_str("</p:presentation>");
    out
}

fn slide_master() -> String {
    format!(
        r#"{DECLARATION}<p:sldMaster {}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{}</p:spTree></p:cSld>{}<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz="4400"/></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#,
        pml_namespaces(),
        slide::EMPTY_GROUP_PROPS,
        xml::CLR_MAP,
    )
}

fn slide_layout() -> String {
    format!(
        r#"{DECLARATION}<p:sldLayout {} type="blank" preserve="1"><p:cSld name="Blank"><p:spTree>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#,
        pml_namespaces(),
        slide::EMPTY_GROUP_PROPS
    )
}

fn core_props(props: &Properties) -> String {
    let mut out = format!(
        r#"{DECLARATION}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#
    );
    if let Some(title) = &props.title {
        write!(out, "<dc:title>{}</dc:title>", escape(title)).unwrap();
    }
    if let Some(author) = &props.author {
        write!(out, "<dc:creator>{}</dc:creator>", escape(author)).unwrap();
    }
    out.push_str("</cp:coreProperties>");
    out
}

fn app_props(slides: usize) -> String {
    format!(
        r#"{DECLARATION}<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>typst-pptx</Application><Slides>{slides}</Slides></Properties>"#
    )
}
