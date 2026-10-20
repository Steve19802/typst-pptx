//! Notes master and notes slides (speaker notes, plan §6.1).

use std::fmt::Write;

use crate::slide::EMPTY_GROUP_PROPS;
use crate::xml::{CLR_MAP, DECLARATION, escape, pml_namespaces};

/// Slide image and body placeholders, laid out on the portrait notes page.
const SLIDE_IMAGE: &str = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="381000" y="685800"/><a:ext cx="6096000" cy="3429000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="12700"><a:solidFill><a:prstClr val="black"/></a:solidFill></a:ln></p:spPr></p:sp>"#;
const BODY_PROPS: &str = r#"<p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr>"#;

pub fn master() -> String {
    format!(
        r#"{DECLARATION}<p:notesMaster {}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{EMPTY_GROUP_PROPS}{SLIDE_IMAGE}<p:sp>{BODY_PROPS}<p:spPr><a:xfrm><a:off x="685800" y="4400550"/><a:ext cx="5486400" cy="3600450"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0"/><a:lstStyle/><a:p><a:pPr lvl="0"/><a:r><a:rPr lang="en-US"/><a:t>Notes</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>{CLR_MAP}<p:notesStyle><a:lvl1pPr marL="0" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1200" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:notesStyle></p:notesMaster>"#,
        pml_namespaces()
    )
}

/// A notes slide with one paragraph per entry; single newlines inside an
/// entry become line breaks.
pub fn slide(notes: &[String]) -> String {
    let mut body = String::new();
    for note in notes {
        body.push_str("<a:p>");
        for (index, line) in note.split('\n').enumerate() {
            if index > 0 {
                body.push_str(r#"<a:br><a:rPr lang="en-US" dirty="0"/></a:br>"#);
            }
            write!(
                body,
                r#"<a:r><a:rPr lang="en-US" dirty="0"/><a:t>{}</a:t></a:r>"#,
                escape(line)
            )
            .unwrap();
        }
        body.push_str("</a:p>");
    }
    format!(
        r#"{DECLARATION}<p:notes {}><p:cSld><p:spTree>{EMPTY_GROUP_PROPS}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp>{}<p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{body}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#,
        pml_namespaces(),
        BODY_PROPS.replace(r#" sz="quarter" idx="3""#, r#" idx="1""#),
    )
}
