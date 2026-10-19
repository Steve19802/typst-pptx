//! Depth-first walk over a page frame (plan §4 steps 3–4).
//!
//! Every leaf item is reported with its absolute transform (page coordinates,
//! points, y down) and the stack of elements whose tags are open around it.

use typst::foundations::Content;
use typst::introspection::{Location, Tag};
use typst::layout::{Frame, FrameItem, GroupItem, Point, Transform};

/// What the walker reports for one frame item.
pub enum Event<'a> {
    /// Entering a group; `ts` already includes the group's own transform.
    GroupStart(&'a GroupItem),
    GroupEnd,
    /// A text, shape, image or link item.
    Item(&'a FrameItem),
    TagStart(&'a Content),
    /// An element closed; the stack no longer contains it.
    TagEnd(&'a Content),
}

/// State at the current position of the walk.
pub struct Context<'a> {
    /// Maps item-local coordinates to page coordinates.
    pub ts: Transform,
    /// Elements whose start tag has been seen and whose end tag hasn't,
    /// outermost first.
    pub tags: &'a [&'a Content],
    /// Group nesting depth (0 = page frame).
    pub depth: usize,
}

/// Walks one page frame. Returns the elements still open at the end, which
/// happens for elements that continue on the next page.
pub fn walk<'a>(frame: &'a Frame, visit: &mut impl FnMut(&Context, Event<'a>)) -> Vec<&'a Content> {
    let mut walker = Walker {
        tags: Vec::new(),
        visit,
    };
    walker.frame(frame, Transform::identity(), 0);
    walker.tags
}

struct Walker<'a, 'v, F> {
    tags: Vec<&'a Content>,
    visit: &'v mut F,
}

impl<'a, F: FnMut(&Context, Event<'a>)> Walker<'a, '_, F> {
    fn frame(&mut self, frame: &'a Frame, ts: Transform, depth: usize) {
        for (pos, item) in frame.items() {
            let ts = ts.pre_concat(Transform::translate(pos.x, pos.y));
            match item {
                FrameItem::Group(group) => {
                    let ts = ts.pre_concat(group.transform);
                    self.emit(ts, depth, Event::GroupStart(group));
                    self.frame(&group.frame, ts, depth + 1);
                    self.emit(ts, depth, Event::GroupEnd);
                }
                FrameItem::Tag(Tag::Start(elem, _)) => {
                    self.emit(ts, depth, Event::TagStart(elem));
                    self.tags.push(elem);
                }
                FrameItem::Tag(Tag::End(loc, ..)) => {
                    if let Some(elem) = self.close(*loc) {
                        self.emit(ts, depth, Event::TagEnd(elem));
                    }
                }
                _ => self.emit(ts, depth, Event::Item(item)),
            }
        }
    }

    /// Removes the element with this location from the stack. Usually it is
    /// the innermost one, but placed content and page-spanning elements can
    /// close out of order.
    fn close(&mut self, loc: Location) -> Option<&'a Content> {
        let index = self
            .tags
            .iter()
            .rposition(|elem| elem.location() == Some(loc))?;
        Some(self.tags.remove(index))
    }

    fn emit(&mut self, ts: Transform, depth: usize, event: Event<'a>) {
        let ctx = Context {
            ts,
            tags: &self.tags,
            depth,
        };
        (self.visit)(&ctx, event);
    }
}

/// Page coordinates of the item origin.
pub fn origin(ts: Transform) -> Point {
    Point::zero().transform(ts)
}
