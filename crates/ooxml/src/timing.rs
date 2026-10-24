//! Slide transition and click animations (plan §6.3): `p:transition` with
//! Morph, `p:timing` with Appear/Disappear effects, one click group each.

use std::collections::HashMap;
use std::fmt::Write;

use pptx_ir::{EffectKind, Item, Slide, Transition};

/// Morph inside `mc:AlternateContent`; applications without it fade.
const MORPH: &str = r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:p159="http://schemas.microsoft.com/office/powerpoint/2015/09/main" Requires="p159"><p:transition spd="fast"><p159:morph option="byObject"/></p:transition></mc:Choice><mc:Fallback><p:transition spd="fast"><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>"#;

pub fn transition(slide: &Slide) -> &'static str {
    match slide.transition {
        Some(Transition::Morph) => MORPH,
        None => "",
    }
}

/// `p:timing` for the slide's clicks; `ids` are the shape ids of its items.
pub fn timing(slide: &Slide, ids: &[usize]) -> String {
    if slide.clicks.is_empty() {
        return String::new();
    }
    // Build groups: a shape's effects of one kind in a row share one.
    let mut groups: HashMap<usize, (u32, EffectKind)> = HashMap::new();
    let mut builds: Vec<(usize, u32, bool)> = Vec::new();
    let mut next = 3;
    let mut clicks = String::new();
    for effects in &slide.clicks {
        let click = next;
        let inner = next + 1;
        next += 2;
        let mut body = String::new();
        for (index, effect) in effects.iter().enumerate() {
            let spid = ids[effect.item];
            let group = match groups.get(&spid) {
                Some(&(group, kind)) if kind == effect.kind => group,
                Some(&(group, _)) => group + 1,
                None => 0,
            };
            groups.insert(spid, (group, effect.kind));
            let by_paragraph = effect.paragraphs.is_some();
            if !builds.iter().any(|&(s, g, _)| s == spid && g == group) {
                builds.push((spid, group, by_paragraph));
            }
            let (class, value) = match effect.kind {
                EffectKind::Appear => ("entr", "visible"),
                EffectKind::Disappear => ("exit", "hidden"),
            };
            let node = if index == 0 {
                "clickEffect"
            } else {
                "withEffect"
            };
            let target = match effect.paragraphs {
                Some((first, last)) => format!(
                    r#"<p:spTgt spid="{spid}"><p:txEl><p:pRg st="{first}" end="{last}"/></p:txEl></p:spTgt>"#
                ),
                None => format!(r#"<p:spTgt spid="{spid}"/>"#),
            };
            write!(
                body,
                r#"<p:par><p:cTn id="{}" presetID="1" presetClass="{class}" presetSubtype="0" fill="hold" grpId="{group}" nodeType="{node}"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:set><p:cBhvr><p:cTn id="{}" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl>{target}</p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="{value}"/></p:to></p:set></p:childTnLst></p:cTn></p:par>"#,
                next,
                next + 1,
            )
            .unwrap();
            next += 2;
        }
        write!(
            clicks,
            r#"<p:par><p:cTn id="{click}" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="{inner}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>{body}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#
        )
        .unwrap();
    }

    let mut out = format!(
        r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{clicks}</p:childTnLst></p:cTn><p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst>"#
    );
    // Build entries for shapes with a text body only (`p:sp`).
    let kinds: HashMap<usize, &Item> = ids.iter().copied().zip(&slide.items).collect();
    let mut list = String::new();
    for (spid, group, by_paragraph) in builds {
        let build = match kinds[&spid] {
            Item::TextBox(_) if by_paragraph => r#" build="p""#,
            Item::TextBox(_) => "",
            Item::Shape(shape) if !matches!(shape.geometry, pptx_ir::Geometry::Line { .. }) => {
                r#" animBg="1""#
            }
            _ => continue,
        };
        write!(list, r#"<p:bldP spid="{spid}" grpId="{group}"{build}/>"#).unwrap();
    }
    if !list.is_empty() {
        write!(out, "<p:bldLst>{list}</p:bldLst>").unwrap();
    }
    out.push_str("</p:timing>");
    out
}
