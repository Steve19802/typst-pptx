//! PresentationML/DrawingML writer and font embedding.

mod slide;
mod theme;
mod xml;

use std::fmt::Write as _;
use std::io::{self, Seek, Write};

use pptx_ir::Deck;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::xml::{
    DECLARATION, REL_APP_PROPS, REL_CORE_PROPS, REL_OFFICE_DOCUMENT, REL_PRES_PROPS, REL_SLIDE,
    REL_SLIDE_LAYOUT, REL_SLIDE_MASTER, REL_TABLE_STYLES, REL_THEME, REL_VIEW_PROPS, Rel, escape,
    pml_namespaces, rels,
};

/// Document properties written to `docProps/core.xml`.
#[derive(Debug, Clone, Default)]
pub struct Properties {
    pub title: Option<String>,
    pub author: Option<String>,
}

/// Writes the deck as a `.pptx` package. Output is deterministic: parts in a
/// fixed order with fixed timestamps (plan §8).
pub fn write_pptx<W: Write + Seek>(deck: &Deck, props: &Properties, out: W) -> io::Result<()> {
    let mut zip = ZipWriter::new(out);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default());
    let mut part = |name: &str, data: String| -> io::Result<()> {
        zip.start_file(name, options)?;
        zip.write_all(data.as_bytes())
    };

    let n = deck.slides.len();
    part("[Content_Types].xml", content_types(n))?;
    part(
        "_rels/.rels",
        rels(&[
            rel(1, REL_OFFICE_DOCUMENT, "ppt/presentation.xml"),
            rel(2, REL_CORE_PROPS, "docProps/core.xml"),
            rel(3, REL_APP_PROPS, "docProps/app.xml"),
        ]),
    )?;
    part("docProps/core.xml", core_props(props))?;
    part("docProps/app.xml", app_props(n))?;
    part("ppt/presentation.xml", presentation(deck))?;
    part("ppt/_rels/presentation.xml.rels", presentation_rels(n))?;
    part(
        "ppt/presProps.xml",
        format!("{DECLARATION}<p:presentationPr {}/>", pml_namespaces()),
    )?;
    part(
        "ppt/viewProps.xml",
        format!("{DECLARATION}<p:viewPr {}/>", pml_namespaces()),
    )?;
    part(
        "ppt/tableStyles.xml",
        format!(
            r#"{DECLARATION}<a:tblStyleLst xmlns:a="{}" def="{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}"/>"#,
            xml::NS_A
        ),
    )?;
    part("ppt/theme/theme1.xml", theme::theme())?;
    part("ppt/slideMasters/slideMaster1.xml", slide_master())?;
    part(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        rels(&[
            rel(1, REL_SLIDE_LAYOUT, "../slideLayouts/slideLayout1.xml"),
            rel(2, REL_THEME, "../theme/theme1.xml"),
        ]),
    )?;
    part("ppt/slideLayouts/slideLayout1.xml", slide_layout())?;
    part(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        rels(&[rel(1, REL_SLIDE_MASTER, "../slideMasters/slideMaster1.xml")]),
    )?;
    for (index, s) in deck.slides.iter().enumerate() {
        let number = index + 1;
        part(&format!("ppt/slides/slide{number}.xml"), slide::slide(s))?;
        part(
            &format!("ppt/slides/_rels/slide{number}.xml.rels"),
            rels(&[rel(1, REL_SLIDE_LAYOUT, "../slideLayouts/slideLayout1.xml")]),
        )?;
    }
    zip.finish()?;
    Ok(())
}

fn rel(id: usize, kind: &'static str, target: &str) -> Rel {
    Rel {
        id: format!("rId{id}"),
        kind,
        target: target.into(),
    }
}

fn content_types(slides: usize) -> String {
    const PML: &str = "application/vnd.openxmlformats-officedocument.presentationml";
    let mut out = format!(
        r#"{DECLARATION}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#
    );
    let mut override_ = |part: &str, kind: &str| {
        write!(out, r#"<Override PartName="{part}" ContentType="{kind}"/>"#).unwrap();
    };
    override_(
        "/ppt/presentation.xml",
        &format!("{PML}.presentation.main+xml"),
    );
    override_("/ppt/presProps.xml", &format!("{PML}.presProps+xml"));
    override_("/ppt/viewProps.xml", &format!("{PML}.viewProps+xml"));
    override_("/ppt/tableStyles.xml", &format!("{PML}.tableStyles+xml"));
    override_(
        "/ppt/theme/theme1.xml",
        "application/vnd.openxmlformats-officedocument.theme+xml",
    );
    override_(
        "/ppt/slideMasters/slideMaster1.xml",
        &format!("{PML}.slideMaster+xml"),
    );
    override_(
        "/ppt/slideLayouts/slideLayout1.xml",
        &format!("{PML}.slideLayout+xml"),
    );
    for number in 1..=slides {
        override_(
            &format!("/ppt/slides/slide{number}.xml"),
            &format!("{PML}.slide+xml"),
        );
    }
    override_(
        "/docProps/core.xml",
        "application/vnd.openxmlformats-package.core-properties+xml",
    );
    override_(
        "/docProps/app.xml",
        "application/vnd.openxmlformats-officedocument.extended-properties+xml",
    );
    out.push_str("</Types>");
    out
}

/// Relationship ids of `presentation.xml`: rId1 = master, rId2.. = slides,
/// then theme and the property parts.
fn presentation_rels(slides: usize) -> String {
    let mut list = vec![rel(1, REL_SLIDE_MASTER, "slideMasters/slideMaster1.xml")];
    for number in 1..=slides {
        list.push(rel(
            number + 1,
            REL_SLIDE,
            &format!("slides/slide{number}.xml"),
        ));
    }
    let next = slides + 2;
    list.push(rel(next, REL_THEME, "theme/theme1.xml"));
    list.push(rel(next + 1, REL_PRES_PROPS, "presProps.xml"));
    list.push(rel(next + 2, REL_VIEW_PROPS, "viewProps.xml"));
    list.push(rel(next + 3, REL_TABLE_STYLES, "tableStyles.xml"));
    rels(&list)
}

fn presentation(deck: &Deck) -> String {
    let mut out = format!(
        r#"{DECLARATION}<p:presentation {} saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>"#,
        pml_namespaces()
    );
    for index in 0..deck.slides.len() {
        write!(
            out,
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            256 + index,
            index + 2
        )
        .unwrap();
    }
    write!(
        out,
        r#"</p:sldIdLst><p:sldSz cx="{}" cy="{}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#,
        deck.width, deck.height
    )
    .unwrap();
    out
}

fn slide_master() -> String {
    format!(
        r#"{DECLARATION}<p:sldMaster {}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{}</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz="4400"/></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#,
        pml_namespaces(),
        slide::EMPTY_GROUP_PROPS
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
