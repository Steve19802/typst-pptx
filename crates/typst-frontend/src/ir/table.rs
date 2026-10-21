//! Native tables (plan §5.6): Typst's `table` element as `a:tbl`.
//!
//! A pre-pass over the page collects each table's cells (positions from the
//! cell tags, which sit at the cell's top-left corner), its fill rectangles
//! and border lines, and the text of every cell. A table that fits the
//! DrawingML model becomes one [`Table`] item; anything else keeps the
//! regular mapping (shapes and text boxes) and is listed in the report.

use std::collections::HashMap;

use pptx_ir::{Fidelity, Line, Merge, Paragraph, Table, TableCell, emu};
use typst::foundations::{Content, Repr, Value};
use typst::introspection::Location;
use typst::layout::{Abs, Frame, FrameItem, Point, Transform};
use typst::text::TextItem;
use typst::visualize::{Geometry as TypstGeometry, Shape as TypstShape};

use super::{
    BAKED, DeckBuilder, Item, Located, ParagraphBox, Rect, TextBox, fill, line, line_mode, lines,
    paragraphs, rect, solid, text_box,
};
use crate::walk::{self, Event};

/// How tables are exported (`--tables`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TableMode {
    /// Native where the checks pass, else shapes and text boxes.
    #[default]
    Auto,
    /// Native even where PowerPoint might grow a row.
    Native,
    /// Always shapes and text boxes.
    Shapes,
}

/// A table exported as shapes and text boxes instead of `a:tbl`.
#[derive(Debug, Clone)]
pub struct TableFallback {
    pub at: Located,
    pub reason: &'static str,
}

/// Grid positions closer than this are the same line, in pt.
const SAME_LINE: f64 = 0.05;
/// How far cell text may start above the cell, moved down instead, in pt.
const CELL_TOP_SLACK: f64 = 0.5;

struct Scan<'a> {
    elem: &'a Content,
    /// The table's frame, the first group after its start tag.
    frame: Option<(Point, Point)>,
    cells: Vec<CellScan<'a>>,
    /// Fill rectangles and border lines drawn by the table.
    shapes: Vec<(Transform, &'a TypstShape)>,
    open_cell: Option<usize>,
    failed: Option<&'static str>,
}

struct CellScan<'a> {
    x: usize,
    y: usize,
    colspan: usize,
    rowspan: usize,
    at: Point,
    texts: Vec<(Transform, &'a TextItem)>,
}

impl Scan<'_> {
    fn fail(&mut self, reason: &'static str) {
        self.failed.get_or_insert(reason);
    }
}

fn int_field(elem: &Content, name: &str, default: usize) -> Option<usize> {
    match elem.get_by_name(name) {
        Ok(Value::Int(n)) => usize::try_from(n).ok(),
        Ok(_) => None,
        Err(_) => Some(default),
    }
}

/// Whether a gutter field is set to something other than nothing.
fn has_gutter(elem: &Content) -> bool {
    ["column-gutter", "row-gutter", "gutter"]
        .iter()
        .any(|name| matches!(elem.get_by_name(name), Ok(value) if value.repr() != "()"))
}

impl DeckBuilder {
    /// Native tables of a page by the location of their element. Tables
    /// that don't qualify are missing and reported.
    pub(super) fn tables(&mut self, frame: &Frame) -> HashMap<Location, Item> {
        let mut out = HashMap::new();
        if self.tables == TableMode::Shapes {
            return out;
        }
        let mut scans: Vec<Scan> = Vec::new();
        let mut done: Vec<Scan> = Vec::new();
        walk::walk(frame, &mut |ctx, event| match event {
            Event::TagStart(elem) if elem.elem().name() == "table" => {
                if let Some(outer) = scans.last_mut() {
                    outer.fail("a cell holds another table");
                }
                let mut scan = Scan {
                    elem,
                    frame: None,
                    cells: Vec::new(),
                    shapes: Vec::new(),
                    open_cell: None,
                    failed: None,
                };
                if has_gutter(elem) {
                    scan.fail("the table has gutters");
                }
                scans.push(scan);
            }
            Event::TagEnd(elem) if elem.elem().name() == "table" => {
                if let Some(index) = scans
                    .iter()
                    .rposition(|scan| scan.elem.location() == elem.location())
                {
                    done.push(scans.remove(index));
                }
            }
            _ => {
                let Some(scan) = scans.last_mut() else {
                    return;
                };
                match event {
                    Event::GroupStart(group) => {
                        if scan.frame.is_none() {
                            if !ctx.ts.is_only_translate() {
                                scan.fail("the table is rotated or scaled");
                            }
                            let min = walk::origin(ctx.ts);
                            let size = group.frame.size();
                            scan.frame = Some((min, min + Point::new(size.x, size.y)));
                        }
                    }
                    Event::TagStart(elem) if elem.elem().name() == "cell" => {
                        if scan.open_cell.is_some() {
                            scan.fail("a cell holds a grid");
                            return;
                        }
                        let fields = (
                            int_field(elem, "x", 0),
                            int_field(elem, "y", 0),
                            int_field(elem, "colspan", 1),
                            int_field(elem, "rowspan", 1),
                        );
                        let (Some(x), Some(y), Some(colspan), Some(rowspan)) = fields else {
                            scan.fail("a cell has no grid position");
                            return;
                        };
                        if !ctx.ts.is_only_translate() {
                            scan.fail("the table is rotated or scaled");
                        }
                        scan.open_cell = Some(scan.cells.len());
                        scan.cells.push(CellScan {
                            x,
                            y,
                            colspan,
                            rowspan,
                            at: walk::origin(ctx.ts),
                            texts: Vec::new(),
                        });
                    }
                    Event::TagEnd(elem) if elem.elem().name() == "cell" => scan.open_cell = None,
                    Event::TagStart(elem) if BAKED.contains(&elem.elem().name()) => {
                        scan.fail("a cell holds an equation");
                    }
                    Event::Item(item) => match (item, scan.open_cell) {
                        (FrameItem::Link(..), _) => {}
                        (FrameItem::Text(text), Some(cell)) => {
                            scan.cells[cell].texts.push((ctx.ts, text))
                        }
                        (FrameItem::Shape(shape, _), None) => scan.shapes.push((ctx.ts, shape)),
                        (_, Some(_)) => scan.fail("a cell holds a shape or an image"),
                        (_, None) => scan.fail("the table draws more than fills and lines"),
                    },
                    _ => {}
                }
            }
        });
        for scan in done {
            let Some(location) = scan.elem.location() else {
                continue;
            };
            // Themes and Touying rebuild elements, so a table's own span
            // may point into a package: the first cell text is the user's.
            let in_package = scan
                .elem
                .span()
                .id()
                .is_none_or(|id| !matches!(id.root(), typst::syntax::VirtualRoot::Project));
            let span = scan
                .cells
                .iter()
                .flat_map(|cell| &cell.texts)
                .map(|(_, text)| super::item_span_text(text))
                .find(|span| !span.is_detached())
                .filter(|_| in_package)
                .unwrap_or(scan.elem.span());
            let at = self.at(span);
            match self.native_table(&scan) {
                Ok(table) => {
                    out.insert(location, Item::Table(table));
                }
                Err(reason) => self
                    .report
                    .table_fallbacks
                    .push(TableFallback { at, reason }),
            }
        }
        out
    }

    fn native_table(&mut self, scan: &Scan) -> Result<Table, &'static str> {
        if let Some(reason) = scan.failed {
            return Err(reason);
        }
        let (min, max) = scan.frame.ok_or("the table has no frame")?;
        if scan.cells.is_empty() {
            return Err("the table has no cells");
        }
        let n_cols = scan.cells.iter().map(|c| c.x + c.colspan).max().unwrap();
        let n_rows = scan.cells.iter().map(|c| c.y + c.rowspan).max().unwrap();

        // Grid lines from the cell corners, closed by the table's frame.
        let mut xs: Vec<Option<Abs>> = vec![None; n_cols + 1];
        let mut ys: Vec<Option<Abs>> = vec![None; n_rows + 1];
        for cell in &scan.cells {
            for (slot, value) in [(&mut xs[cell.x], cell.at.x), (&mut ys[cell.y], cell.at.y)] {
                match slot {
                    Some(known) if (*known - value).to_pt().abs() > SAME_LINE => {
                        return Err("cells don't line up");
                    }
                    _ => *slot = Some(value),
                }
            }
        }
        xs[n_cols] = Some(max.x);
        ys[n_rows] = Some(max.y);
        let xs: Vec<Abs> = xs
            .into_iter()
            .collect::<Option<_>>()
            .ok_or("a column has no cell")?;
        let ys: Vec<Abs> = ys
            .into_iter()
            .collect::<Option<_>>()
            .ok_or("a row has no cell")?;
        if (xs[0] - min.x).to_pt().abs() > SAME_LINE
            || (ys[0] - min.y).to_pt().abs() > SAME_LINE
            || xs.windows(2).any(|w| w[1] <= w[0])
            || ys.windows(2).any(|w| w[1] <= w[0])
        {
            return Err("cells don't line up");
        }

        // Which cell covers each grid position.
        let mut owner = vec![vec![None; n_cols]; n_rows];
        for (index, cell) in scan.cells.iter().enumerate() {
            for row in &mut owner[cell.y..cell.y + cell.rowspan] {
                for slot in &mut row[cell.x..cell.x + cell.colspan] {
                    if slot.replace(index).is_some() {
                        return Err("cells overlap");
                    }
                }
            }
        }
        if owner.iter().flatten().any(Option::is_none) {
            return Err("the table has holes");
        }
        let owner: Vec<Vec<usize>> = owner
            .into_iter()
            .map(|row| row.into_iter().map(Option::unwrap).collect())
            .collect();
        let area = |cell: &CellScan| {
            (
                Point::new(xs[cell.x], ys[cell.y]),
                Point::new(xs[cell.x + cell.colspan], ys[cell.y + cell.rowspan]),
            )
        };

        // Fills and borders. Border lines are kept per grid segment:
        // horizontal[r][c] lies on row line r under column c, vertical[r][c]
        // on column line c beside row r.
        let mut fills = vec![None; scan.cells.len()];
        let mut horizontal: Vec<Vec<Option<Line>>> = vec![vec![None; n_cols]; n_rows + 1];
        let mut vertical: Vec<Vec<Option<Line>>> = vec![vec![None; n_cols + 1]; n_rows];
        let near = |a: Abs, b: Abs| (a - b).to_pt().abs() <= SAME_LINE;
        let index_of = |lines: &[Abs], at: Abs| lines.iter().position(|&l| near(l, at));
        for (ts, shape) in &scan.shapes {
            if !ts.is_only_translate() {
                return Err("the table draws transformed shapes");
            }
            let origin = walk::origin(*ts);
            match &shape.geometry {
                TypstGeometry::Rect(size) if shape.stroke.is_none() => {
                    let Some(paint) = &shape.fill else { continue };
                    let far = origin + Point::new(size.x, size.y);
                    let index = scan
                        .cells
                        .iter()
                        .position(|cell| {
                            let (a, b) = area(cell);
                            near(a.x, origin.x)
                                && near(a.y, origin.y)
                                && near(b.x, far.x)
                                && near(b.y, far.y)
                        })
                        .ok_or("a fill doesn't match a cell")?;
                    fills[index] =
                        Some(fill(paint, rect(origin, far)).ok_or("a cell fill can't be mapped")?);
                }
                TypstGeometry::Line(to) => {
                    let stroke = shape.stroke.as_ref().ok_or("a table line has no stroke")?;
                    let (line, _) = line(stroke).ok_or("a table line can't be mapped")?;
                    let half = stroke.thickness / 2.0;
                    let (a, b) = (origin, origin + *to);
                    if near(a.y, b.y) {
                        let (x0, x1) = (a.x.min(b.x) + half, a.x.max(b.x) - half);
                        let row = index_of(&ys, a.y).ok_or("a border is off the grid")?;
                        let (c0, c1) = (
                            index_of(&xs, x0).ok_or("a border is off the grid")?,
                            index_of(&xs, x1).ok_or("a border is off the grid")?,
                        );
                        for slot in &mut horizontal[row][c0..c1] {
                            *slot = Some(line.clone());
                        }
                    } else if near(a.x, b.x) {
                        let (y0, y1) = (a.y.min(b.y) + half, a.y.max(b.y) - half);
                        let col = index_of(&xs, a.x).ok_or("a border is off the grid")?;
                        let (r0, r1) = (
                            index_of(&ys, y0).ok_or("a border is off the grid")?,
                            index_of(&ys, y1).ok_or("a border is off the grid")?,
                        );
                        for row in &mut vertical[r0..r1] {
                            row[col] = Some(line.clone());
                        }
                    } else {
                        return Err("the table draws a diagonal line");
                    }
                }
                _ => return Err("the table draws other shapes"),
            }
        }
        // A border inside a merged cell can't be drawn.
        for cell in &scan.cells {
            let inner_h = (cell.y + 1..cell.y + cell.rowspan)
                .any(|r| (cell.x..cell.x + cell.colspan).any(|c| horizontal[r][c].is_some()));
            let inner_v = (cell.y..cell.y + cell.rowspan)
                .any(|r| (cell.x + 1..cell.x + cell.colspan).any(|c| vertical[r][c].is_some()));
            if inner_h || inner_v {
                return Err("a border crosses a merged cell");
            }
        }
        // One line per cell side: every segment along it must agree.
        let side = |segments: Vec<&Option<Line>>| -> Result<Option<Line>, &'static str> {
            let first = segments[0];
            let same = segments.iter().all(|s| match (s, first) {
                (None, None) => true,
                (Some(a), Some(b)) => a.color == b.color && a.width == b.width && a.dash == b.dash,
                _ => false,
            });
            if same {
                Ok(first.clone())
            } else {
                Err("a border covers part of a cell side")
            }
        };

        // Cell text and margins.
        let mut cells = Vec::with_capacity(scan.cells.len());
        for (index, cell) in scan.cells.iter().enumerate() {
            let (a, b) = area(cell);
            let rows = cell.y..cell.y + cell.rowspan;
            let cols = cell.x..cell.x + cell.colspan;
            let borders = [
                side(rows.clone().map(|r| &vertical[r][cell.x]).collect())?,
                side(
                    rows.clone()
                        .map(|r| &vertical[r][cell.x + cell.colspan])
                        .collect(),
                )?,
                side(cols.clone().map(|c| &horizontal[cell.y][c]).collect())?,
                side(
                    cols.clone()
                        .map(|c| &horizontal[cell.y + cell.rowspan][c])
                        .collect(),
                )?,
            ];
            let (margins, paragraphs) = self.cell_text(cell, rect(a, b), index)?;
            cells.push(TableCell {
                merge: Merge::Origin {
                    columns: cell.colspan as u32,
                    rows: cell.rowspan as u32,
                },
                fill: fills[index].clone(),
                borders,
                margins,
                paragraphs,
            });
        }

        let grid: Vec<Vec<TableCell>> = (0..n_rows)
            .map(|r| {
                (0..n_cols)
                    .map(|c| {
                        let index = owner[r][c];
                        let cell = &scan.cells[index];
                        if cell.x == c && cell.y == r {
                            cells[index].clone()
                        } else {
                            TableCell {
                                merge: Merge::Covered {
                                    horizontal: c > cell.x,
                                    vertical: r > cell.y,
                                },
                                ..cells[index].clone()
                            }
                        }
                    })
                    .collect()
            })
            .collect();
        let edges = |lines: &[Abs]| -> Vec<i64> {
            let at: Vec<i64> = lines.iter().map(|l| emu(l.to_pt())).collect();
            at.windows(2).map(|w| w[1] - w[0]).collect()
        };
        Ok(Table {
            name: format!("Table {}", self.report.tables + 1),
            frame: rect(Point::new(xs[0], ys[0]), Point::new(xs[n_cols], ys[n_rows])),
            columns: edges(&xs),
            rows: edges(&ys),
            cells: grid,
            fidelity: Fidelity::Native,
        })
        .inspect(|_| self.report.tables += 1)
    }

    /// A cell's text through the text-box pipeline, and the margins that put
    /// it where Typst did. The text gets the widest room its alignment
    /// allows, so PowerPoint has no reason to wrap it, and must fit the row
    /// height or PowerPoint would grow the row.
    fn cell_text(
        &mut self,
        cell: &CellScan,
        area: Rect,
        index: usize,
    ) -> Result<([i64; 4], Vec<Paragraph>), &'static str> {
        let mut pieces = Vec::new();
        for (ts, text) in &cell.texts {
            if !ts.is_only_translate() {
                return Err("a cell holds rotated text");
            }
            let color = solid(&text.fill).ok_or("a cell holds gradient text")?;
            pieces.push(self.piece(*ts, walk::origin(*ts), text, color));
        }
        let (boxes, split) = line_mode(paragraphs(lines(pieces), self.model));
        if !split.is_empty() || boxes.len() > 1 {
            return Err("a cell holds more than one block of text");
        }
        let Some(paragraph) = boxes.into_iter().next() else {
            return Ok(([0; 4], vec![Paragraph::empty()]));
        };
        // The model's own spacing where the text fits the row, else
        // spacing rounded down.
        let (text, top) = self
            .place_cell_text(cell, area, index, paragraph.clone(), false)
            .or_else(|_| self.place_cell_text(cell, area, index, paragraph, true))?;
        let f = text.frame;
        let (left, right) = (f.x - area.x, area.x + area.w - (f.x + f.w));
        let align = text.paragraphs[0].align;
        let (left, right) = match align {
            pptx_ir::Align::Left => (left, 0),
            pptx_ir::Align::Right => (0, right),
            pptx_ir::Align::Center => {
                let room = left.min(right);
                (left - room, right - room)
            }
        };
        Ok(([left, right, top, 0], text.paragraphs))
    }

    /// One cell's text box and top margin; `cell_spacing` rounds exact
    /// spacing down ([`TextModel::cell_line_spacing`]).
    fn place_cell_text(
        &mut self,
        cell: &CellScan,
        area: Rect,
        index: usize,
        paragraph: ParagraphBox,
        cell_spacing: bool,
    ) -> Result<(TextBox, i64), &'static str> {
        let first_size = paragraph.lines[0].max_size();
        let mut text = text_box(
            format!("Cell {index}"),
            paragraph,
            self.model,
            false,
            cell_spacing,
        );
        // Cells may place the first baseline differently from text boxes.
        let (_, first) = cell
            .texts
            .iter()
            .filter(|(_, t)| (t.size.to_pt() - first_size).abs() < 0.01)
            .map(|(ts, t)| (walk::origin(*ts).y, *t))
            .fold((Abs::inf(), None), |(y, best), (ty, t)| {
                if ty < y { (ty, Some(t)) } else { (y, best) }
            });
        if let Some(first) = first {
            // hhea metrics, as LibreOffice uses (Typst's are OS/2 typo).
            let ttf = first.font.ttf();
            let em = f64::from(ttf.units_per_em());
            let spacing = text.paragraphs[0]
                .line_spacing
                .map(|s| f64::from(s) / 100.0);
            let delta = self.model.first_baseline(first_size, spacing)
                - self.model.cell_first_baseline(
                    first_size,
                    spacing,
                    f64::from(ttf.ascender()) / em,
                    -f64::from(ttf.descender()) / em,
                );
            text.frame.y += emu(delta);
        }
        if let Some(link) = text.link.take() {
            for run in text
                .paragraphs
                .iter_mut()
                .flat_map(|p| p.lines.iter_mut())
                .flatten()
            {
                run.link = Some(link.clone());
            }
        }
        let f = text.frame;
        let (left, right) = (f.x - area.x, area.x + area.w - (f.x + f.w));
        let top = f.y - area.y;
        // Text slightly above the cell's top starts at the top instead.
        let top = if top < 0 && top >= -emu(CELL_TOP_SLACK) {
            0
        } else {
            top
        };
        if left < 0 || right < 0 || top < 0 {
            return Err("a cell's text doesn't fit it in PowerPoint");
        }
        if top + f.h > area.h && self.tables != TableMode::Native {
            return Err("a cell's text is taller than its row");
        }
        Ok((text, top))
    }
}
