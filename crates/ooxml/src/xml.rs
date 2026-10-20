//! Tiny XML string builder. Parts are small and written once, so plain
//! string building with escaping is enough.

use std::fmt::Write;

pub const DECLARATION: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";

/// The `xmlns` attributes every PresentationML part root carries.
pub fn pml_namespaces() -> String {
    format!(r#"xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}""#)
}

/// Escapes text and attribute values and drops characters XML 1.0 forbids.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// A relationship in a `.rels` part.
pub struct Rel {
    pub id: String,
    pub kind: &'static str,
    pub target: String,
    /// `TargetMode="External"` (URLs).
    pub external: bool,
}

pub const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const REL_CORE_PROPS: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
pub const REL_APP_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
pub const REL_SLIDE_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
pub const REL_SLIDE_LAYOUT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
pub const REL_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
pub const REL_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
pub const REL_PRES_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/presProps";
pub const REL_VIEW_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/viewProps";
pub const REL_TABLE_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/tableStyles";

pub const REL_FONT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/font";
pub const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const REL_NOTES_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
pub const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub const REL_NOTES_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";

/// Identity colour map of masters.
pub const CLR_MAP: &str = r#"<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>"#;

pub fn rels(rels: &[Rel]) -> String {
    let mut out = String::from(DECLARATION);
    out.push_str(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in rels {
        write!(
            out,
            r#"<Relationship Id="{}" Type="{}" Target="{}"{}/>"#,
            rel.id,
            rel.kind,
            escape(&rel.target),
            if rel.external {
                r#" TargetMode="External""#
            } else {
                ""
            }
        )
        .unwrap();
    }
    out.push_str("</Relationships>");
    out
}
