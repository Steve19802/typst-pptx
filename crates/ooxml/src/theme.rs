//! The theme part: colour scheme and fonts derived from the deck (plan §7),
//! defaults where it has none.

use std::fmt::Write;

use pptx_ir::Theme;

use crate::xml::{DECLARATION, NS_A};

/// Office's default colours, in [`Theme::SLOTS`] order.
const DEFAULT_COLORS: [&str; 12] = [
    "000000", "FFFFFF", "44546A", "E7E6E6", "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5",
    "70AD47", "0563C1", "954F72",
];

pub fn theme(theme: Option<&Theme>) -> String {
    let mut colors = String::new();
    for (index, slot) in Theme::SLOTS.iter().enumerate() {
        let hex = match theme.and_then(|t| t.colors[index]) {
            Some(c) => format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b),
            None => DEFAULT_COLORS[index].to_string(),
        };
        write!(colors, r#"<a:{slot}><a:srgbClr val="{hex}"/></a:{slot}>"#).unwrap();
    }
    let font = |f: Option<&String>| crate::xml::escape(f.map_or("Arial", String::as_str));
    let major = font(theme.and_then(|t| t.major_font.as_ref()));
    let minor = font(theme.and_then(|t| t.minor_font.as_ref()));
    format!(
        r#"{DECLARATION}<a:theme xmlns:a="{NS_A}" name="typst-pptx"><a:themeElements><a:clrScheme name="typst-pptx">{colors}</a:clrScheme><a:fontScheme name="typst-pptx"><a:majorFont><a:latin typeface="{major}"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="{minor}"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="typst-pptx"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#
    )
}
