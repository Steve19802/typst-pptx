//! Embedded fonts (plan §5.1): `ppt/fonts/fontN.fntdata` parts in Embedded
//! OpenType (EOT) format, listed in `p:embeddedFontLst`.
//!
//! The EOT wrapper is version 0x00020001 with the TrueType data uncompressed
//! and unencrypted (Flags = 0), the simplest form the format allows.

use std::fmt::Write;

use pptx_ir::{EmbeddedFont, FontRef};
use ttf_parser::Tag;

use crate::xml::escape;

pub const CONTENT_TYPE: &str = "application/x-fontdata";
pub const EXTENSION: &str = "fntdata";

/// The families of the embedded faces, in first-use order, with the index
/// of each style's face in the deck's font list.
pub struct Family {
    pub typeface: String,
    pub panose: Option<String>,
    /// regular, bold, italic, bold italic
    pub styles: [Option<usize>; 4],
}

pub fn families(fonts: &[EmbeddedFont]) -> Vec<Family> {
    let mut out: Vec<Family> = Vec::new();
    for (index, font) in fonts.iter().enumerate() {
        let FontRef {
            family,
            bold,
            italic,
        } = &font.font;
        let slot = usize::from(*bold) + 2 * usize::from(*italic);
        let position = match out.iter().position(|f| f.typeface == *family) {
            Some(position) => position,
            None => {
                out.push(Family {
                    typeface: family.clone(),
                    panose: panose(&font.data),
                    styles: [None; 4],
                });
                out.len() - 1
            }
        };
        out[position].styles[slot].get_or_insert(index);
    }
    out
}

/// `p:embeddedFontLst`; `rel_id` gives the relationship id of a face.
pub fn list(families: &[Family], rel_id: impl Fn(usize) -> usize) -> String {
    let mut out = String::from("<p:embeddedFontLst>");
    for family in families {
        out.push_str("<p:embeddedFont>");
        write!(out, r#"<p:font typeface="{}""#, escape(&family.typeface)).unwrap();
        if let Some(panose) = &family.panose {
            write!(out, r#" panose="{panose}""#).unwrap();
        }
        out.push_str("/>");
        for (style, tag) in ["regular", "bold", "italic", "boldItalic"]
            .iter()
            .enumerate()
        {
            if let Some(face) = family.styles[style] {
                write!(out, r#"<p:{tag} r:id="rId{}"/>"#, rel_id(face)).unwrap();
            }
        }
        out.push_str("</p:embeddedFont>");
    }
    out.push_str("</p:embeddedFontLst>");
    out
}

fn panose(ttf: &[u8]) -> Option<String> {
    let os2 = table(ttf, b"OS/2")?;
    let bytes = os2.get(32..42)?;
    Some(bytes.iter().map(|b| format!("{b:02X}")).collect())
}

fn table<'a>(ttf: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    ttf_parser::RawFace::parse(ttf, 0)
        .ok()?
        .table(Tag::from_bytes(tag))
}

/// Wraps a TrueType font in an uncompressed EOT container.
pub fn eot(ttf: &[u8], italic: bool) -> Option<Vec<u8>> {
    let face = ttf_parser::Face::parse(ttf, 0).ok()?;
    let os2 = table(ttf, b"OS/2")?;
    let head = table(ttf, b"head")?;
    let be16 = |data: &[u8], at: usize| {
        data.get(at..at + 2)
            .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]))
    };
    let be32 = |data: &[u8], at: usize| {
        data.get(at..at + 4)
            .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let name = |id: u16| -> Vec<u8> {
        face.names()
            .into_iter()
            .filter(|name| name.name_id == id)
            .find_map(|name| name.to_string())
            .unwrap_or_default()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect()
    };

    let mut out = Vec::with_capacity(ttf.len() + 256);
    let u32le = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    let u16le = |out: &mut Vec<u8>, v: u16| out.extend_from_slice(&v.to_le_bytes());

    u32le(&mut out, 0); // EOTSize, patched below
    u32le(&mut out, u32::try_from(ttf.len()).ok()?); // FontDataSize
    u32le(&mut out, 0x0002_0001); // Version
    u32le(&mut out, 0); // Flags: not subset, not compressed, not encrypted
    out.extend_from_slice(os2.get(32..42)?); // FontPANOSE
    out.push(1); // Charset: DEFAULT_CHARSET
    out.push(u8::from(italic)); // Italic
    u32le(&mut out, u32::from(be16(os2, 4))); // Weight (usWeightClass)
    u16le(&mut out, be16(os2, 8)); // fsType
    u16le(&mut out, 0x504C); // MagicNumber
    for at in [42, 46, 50, 54] {
        u32le(&mut out, be32(os2, at)); // UnicodeRange1-4
    }
    for at in [78, 82] {
        u32le(&mut out, be32(os2, at)); // CodePageRange1-2 (0 in OS/2 v0)
    }
    u32le(&mut out, be32(head, 8)); // CheckSumAdjustment
    for _ in 0..4 {
        u32le(&mut out, 0); // Reserved1-4
    }
    for id in [
        ttf_parser::name_id::FAMILY,
        ttf_parser::name_id::SUBFAMILY,
        ttf_parser::name_id::VERSION,
        ttf_parser::name_id::FULL_NAME,
    ] {
        let text = name(id);
        u16le(&mut out, 0); // Padding
        u16le(&mut out, u16::try_from(text.len()).ok()?);
        out.extend_from_slice(&text);
    }
    u16le(&mut out, 0); // Padding5
    u16le(&mut out, 0); // RootStringSize: no root string
    out.extend_from_slice(ttf);

    let size = u32::try_from(out.len()).ok()?;
    out[..4].copy_from_slice(&size.to_le_bytes());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plex(style: &str) -> Vec<u8> {
        let path = format!(
            "{}/../../fixtures/fonts/ibm-plex/IBMPlexSans-{style}.ttf",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).unwrap()
    }

    fn le16(data: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([data[at], data[at + 1]])
    }

    fn le32(data: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(data[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn eot_header_wraps_the_font_unchanged() {
        let ttf = plex("BoldItalic");
        let eot = eot(&ttf, true).unwrap();
        assert_eq!(le32(&eot, 0) as usize, eot.len(), "EOTSize");
        assert_eq!(le32(&eot, 4) as usize, ttf.len(), "FontDataSize");
        assert_eq!(le32(&eot, 8), 0x0002_0001, "Version");
        assert_eq!(le32(&eot, 12), 0, "Flags");
        assert_eq!(eot[27], 1, "Italic");
        assert_eq!(le32(&eot, 28), 700, "Weight");
        assert_eq!(le16(&eot, 34), 0x504C, "MagicNumber");

        // Four length-prefixed UTF-16LE names, then an empty root string.
        let mut at = 80;
        let mut names = Vec::new();
        for _ in 0..4 {
            let len = le16(&eot, at + 2) as usize;
            let units: Vec<u16> = eot[at + 4..at + 4 + len]
                .chunks(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            names.push(String::from_utf16(&units).unwrap());
            at += 4 + len;
        }
        assert_eq!(names[0], "IBM Plex Sans");
        assert_eq!(names[1], "Bold Italic");
        assert_eq!(le16(&eot, at + 2), 0, "RootStringSize");
        assert_eq!(&eot[at + 4..], &ttf[..]);
    }

    #[test]
    fn faces_are_grouped_by_family_and_style() {
        let face = |family: &str, bold, italic| EmbeddedFont {
            font: FontRef {
                family: family.into(),
                bold,
                italic,
            },
            data: plex("Regular"),
        };
        let fonts = [
            face("IBM Plex Sans", true, false),
            face("IBM Plex Sans", false, false),
            face("IBM Plex Sans SmBld", false, false),
        ];
        let families = families(&fonts);
        assert_eq!(families.len(), 2);
        assert_eq!(families[0].styles, [Some(1), Some(0), None, None]);
        assert_eq!(families[0].panose.as_deref(), Some("020B0503050203000203"));
        let xml = list(&families, |face| face + 10);
        assert!(xml.contains(
            r#"<p:font typeface="IBM Plex Sans" panose="020B0503050203000203"/><p:regular r:id="rId11"/><p:bold r:id="rId10"/>"#
        ));
    }
}
