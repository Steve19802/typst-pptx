//! Simple SVG images as native shapes (plan §5.5).
//!
//! Only trees made of groups and paths with solid paint convert; anything
//! else (images, text, gradients, patterns, masks, filters, clip paths)
//! keeps the picture. Group opacity (also what SVG's element `opacity`
//! becomes) is folded into the paint alpha: exact for a single path with
//! only a fill or only a stroke, approximate otherwise. Typst draws an SVG scaled from the tree's size
//! to the image's size, and so does this conversion.

use pptx_ir::{Color, Fidelity, Fill, Line, LineCap, LineJoin};
use typst::layout::{Abs, Point, Size};
use typst::visualize::{Curve, SvgImage};
use usvg::tiny_skia_path::{PathSegment, Transform};

/// One converted path: absolute page geometry plus paint.
pub struct SvgPath {
    pub curve: Curve,
    pub fill: Option<Fill>,
    pub line: Option<Line>,
    pub fidelity: Fidelity,
}

/// The paths of an SVG shown at `origin` with `size`, or `None` if the tree
/// uses anything that doesn't convert or has more than `max_nodes` nodes.
pub fn paths(svg: &SvgImage, origin: Point, size: Size, max_nodes: usize) -> Option<Vec<SvgPath>> {
    let tree = svg.tree();
    let scale = (
        size.x.to_pt() / f64::from(tree.size().width()),
        size.y.to_pt() / f64::from(tree.size().height()),
    );
    let mut walk = Walk {
        origin,
        scale,
        max_nodes,
        nodes: 0,
        out: Vec::new(),
    };
    walk.group(tree.root(), 1.0)?;
    (!walk.out.is_empty()).then_some(walk.out)
}

struct Walk {
    origin: Point,
    scale: (f64, f64),
    max_nodes: usize,
    nodes: usize,
    out: Vec<SvgPath>,
}

impl Walk {
    fn group(&mut self, group: &usvg::Group, opacity: f64) -> Option<()> {
        if group.clip_path().is_some() || group.mask().is_some() || !group.filters().is_empty() {
            return None;
        }
        let own = f64::from(group.opacity().get());
        let opacity = opacity * own;
        let first = self.out.len();
        for child in group.children() {
            self.nodes += 1;
            if self.nodes > self.max_nodes {
                return None;
            }
            match child {
                usvg::Node::Group(inner) => self.group(inner, opacity)?,
                usvg::Node::Path(path) => {
                    if path.is_visible() {
                        let converted = self::path(path, self.origin, self.scale, opacity)?;
                        self.out.push(converted);
                    }
                }
                usvg::Node::Image(_) | usvg::Node::Text(_) => return None,
            }
        }
        // Group opacity composites the group as a whole; per-paint alpha
        // differs where its paths (or a path's fill and stroke) overlap.
        let added = &mut self.out[first..];
        if own < 1.0
            && (added.len() > 1 || added.iter().any(|p| p.fill.is_some() && p.line.is_some()))
        {
            for path in added {
                path.fidelity = Fidelity::Approximate;
            }
        }
        Some(())
    }
}

fn path(path: &usvg::Path, origin: Point, scale: (f64, f64), opacity: f64) -> Option<SvgPath> {
    let ts = path.abs_transform();
    let map = |p: usvg::tiny_skia_path::Point| {
        let mut p = p;
        ts.map_point(&mut p);
        origin
            + Point::new(
                Abs::pt(f64::from(p.x) * scale.0),
                Abs::pt(f64::from(p.y) * scale.1),
            )
    };

    let mut curve = Curve::new();
    let mut current = None;
    let mut subpaths = 0;
    for segment in path.data().segments() {
        match segment {
            PathSegment::MoveTo(p) => {
                subpaths += 1;
                current = Some(map(p));
                curve.move_(map(p));
            }
            PathSegment::LineTo(p) => {
                current = Some(map(p));
                curve.line(map(p));
            }
            PathSegment::QuadTo(q, p) => {
                // Degree elevation: the cubic's controls are 2/3 of the way
                // from each end point to the quadratic's control.
                let (start, q, end) = (current?, map(q), map(p));
                let c1 = start + (q - start) * (2.0 / 3.0);
                let c2 = end + (q - end) * (2.0 / 3.0);
                curve.cubic(c1, c2, end);
                current = Some(end);
            }
            PathSegment::CubicTo(a, b, p) => {
                current = Some(map(p));
                curve.cubic(map(a), map(b), map(p));
            }
            PathSegment::Close => curve.close(),
        }
    }

    let mut fidelity = Fidelity::Native;
    let fill = match path.fill() {
        Some(fill) => {
            if fill.rule() == usvg::FillRule::EvenOdd && subpaths > 1 {
                // DrawingML has no fill rule.
                fidelity = Fidelity::Approximate;
            }
            Some(Fill::Solid {
                color: color(fill.paint(), fill.opacity(), opacity)?,
            })
        }
        None => None,
    };
    let line = match path.stroke() {
        Some(stroke) => {
            if path.fill().is_some() && path.paint_order() != usvg::PaintOrder::FillAndStroke {
                return None;
            }
            let (factor, uniform) = stroke_scale(ts, scale);
            if !uniform {
                fidelity = Fidelity::Approximate;
            }
            let width = f64::from(stroke.width().get());
            let mut dash: Vec<i32> = stroke
                .dasharray()
                .unwrap_or_default()
                .iter()
                .map(|len| (f64::from(*len) / width * 100_000.0).round().max(1.0) as i32)
                .collect();
            if dash.len() % 2 == 1 {
                dash.extend_from_within(..);
            }
            Some(Line {
                color: color(stroke.paint(), stroke.opacity(), opacity)?,
                width: pptx_ir::emu(width * factor),
                cap: match stroke.linecap() {
                    usvg::LineCap::Butt => LineCap::Flat,
                    usvg::LineCap::Round => LineCap::Round,
                    usvg::LineCap::Square => LineCap::Square,
                },
                join: match stroke.linejoin() {
                    usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => LineJoin::Miter {
                        limit: f64::from(stroke.miterlimit().get()),
                    },
                    usvg::LineJoin::Round => LineJoin::Round,
                    usvg::LineJoin::Bevel => LineJoin::Bevel,
                },
                dash,
            })
        }
        None => None,
    };
    if fill.is_none() && line.is_none() {
        return None;
    }
    Some(SvgPath {
        curve,
        fill,
        line,
        fidelity,
    })
}

/// Mean scale of the path transform times the image scale, and whether it
/// is uniform within 10 % (stroke widths can only scale uniformly).
fn stroke_scale(ts: Transform, scale: (f64, f64)) -> (f64, bool) {
    let x = f64::from(ts.sx).hypot(f64::from(ts.ky)) * scale.0;
    let y = f64::from(ts.kx).hypot(f64::from(ts.sy)) * scale.1;
    let ratio = if x.min(y) > 0.0 {
        x.max(y) / x.min(y)
    } else {
        f64::INFINITY
    };
    ((x * y).sqrt(), ratio <= 1.1)
}

fn color(paint: &usvg::Paint, opacity: usvg::Opacity, group: f64) -> Option<Color> {
    let usvg::Paint::Color(c) = paint else {
        return None;
    };
    Some(Color {
        r: c.red,
        g: c.green,
        b: c.blue,
        a: (f64::from(opacity.get()) * group * 255.0).round() as u8,
    })
}
