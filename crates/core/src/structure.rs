use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::formula::{Coord, Expr, Reference};
use crate::{CellAddr, CellRange, Layout, MAX_COLS, MAX_ROWS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Rows,
    Cols,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shift {
    pub axis: Axis,
    pub at: u32,
    pub count: u32,
    pub span: Option<(u32, u32)>,
    pub insert: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftError {
    MergedCells,
    OffSheet,
}

impl fmt::Display for ShiftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MergedCells => write!(f, "the change would split merged cells"),
            Self::OffSheet => write!(f, "nonblank cells would be pushed off the sheet"),
        }
    }
}

impl std::error::Error for ShiftError {}

impl Shift {
    pub fn rows(at: u32, count: u32, insert: bool) -> Self {
        Self {
            axis: Axis::Rows,
            at,
            count,
            span: None,
            insert,
        }
    }

    pub fn cols(at: u32, count: u32, insert: bool) -> Self {
        Self {
            axis: Axis::Cols,
            at,
            count,
            span: None,
            insert,
        }
    }

    pub fn cells(range: CellRange, axis: Axis, insert: bool) -> Self {
        let (at, count, span) = match axis {
            Axis::Rows => (
                range.start().row(),
                range.rows(),
                (range.start().col(), range.end().col()),
            ),
            Axis::Cols => (
                range.start().col(),
                range.cols(),
                (range.start().row(), range.end().row()),
            ),
        };
        let whole = span == (0, perpendicular_max(axis) - 1);
        Self {
            axis,
            at,
            count,
            span: (!whole).then_some(span),
            insert,
        }
    }

    fn max(&self) -> u32 {
        match self.axis {
            Axis::Rows => MAX_ROWS,
            Axis::Cols => MAX_COLS,
        }
    }

    fn along(&self, addr: CellAddr) -> u32 {
        match self.axis {
            Axis::Rows => addr.row(),
            Axis::Cols => addr.col(),
        }
    }

    fn across(&self, addr: CellAddr) -> u32 {
        match self.axis {
            Axis::Rows => addr.col(),
            Axis::Cols => addr.row(),
        }
    }

    fn covers(&self, lo: u32, hi: u32) -> bool {
        self.span.is_none_or(|(a, b)| a <= lo && hi <= b)
    }

    pub fn moves(&self, addr: CellAddr) -> bool {
        let across = self.across(addr);
        self.covers(across, across) && self.along(addr) >= self.at
    }

    pub fn index(&self, i: u32) -> Option<u32> {
        if i < self.at {
            Some(i)
        } else if self.insert {
            Some(i + self.count).filter(|&i| i < self.max())
        } else if i < self.at + self.count {
            None
        } else {
            Some(i - self.count)
        }
    }

    fn band(&self, lo: u32, hi: u32) -> Option<(u32, u32)> {
        if self.insert {
            let last = self.max() - 1;
            let lo = if lo >= self.at { lo + self.count } else { lo };
            let hi = if hi >= self.at { hi + self.count } else { hi };
            return (lo <= last).then_some((lo, hi.min(last)));
        }
        let end = self.at + self.count;
        let lo = if lo >= end {
            lo - self.count
        } else {
            lo.min(self.at)
        };
        let hi = if hi >= end {
            hi - self.count
        } else if hi >= self.at {
            self.at.checked_sub(1)?
        } else {
            hi
        };
        (lo <= hi).then_some((lo, hi))
    }

    fn make(&self, along: u32, across: u32) -> CellAddr {
        let (row, col) = match self.axis {
            Axis::Rows => (along, across),
            Axis::Cols => (across, along),
        };
        CellAddr::new(row, col).expect("shifted cells stay inside the sheet")
    }

    pub fn addr(&self, addr: CellAddr) -> Option<CellAddr> {
        if !self.moves(addr) {
            return Some(addr);
        }
        Some(self.make(self.index(self.along(addr))?, self.across(addr)))
    }

    pub fn range(&self, range: CellRange) -> Option<CellRange> {
        let (start, end) = (range.start(), range.end());
        if !self.covers(self.across(start), self.across(end)) {
            return Some(range);
        }
        let (lo, hi) = self.band(self.along(start), self.along(end))?;
        Some(CellRange::new(
            self.make(lo, self.across(start)),
            self.make(hi, self.across(end)),
        ))
    }

    fn reference(&self, reference: Reference) -> Option<Reference> {
        let coord = |c: Coord, index: u32| Coord {
            index,
            absolute: c.absolute,
        };
        Some(match reference {
            Reference::Cell { row, col } => {
                let addr = self.addr(CellAddr::new(row.index, col.index).ok()?)?;
                Reference::Cell {
                    row: coord(row, addr.row()),
                    col: coord(col, addr.col()),
                }
            }
            Reference::Area {
                top,
                left,
                bottom,
                right,
            } => {
                let range = self.range(reference.range())?;
                Reference::Area {
                    top: coord(top, range.start().row()),
                    left: coord(left, range.start().col()),
                    bottom: coord(bottom, range.end().row()),
                    right: coord(right, range.end().col()),
                }
            }
            Reference::Rows { top, bottom } if self.axis == Axis::Rows && self.span.is_none() => {
                let (lo, hi) = self.band(top.index, bottom.index)?;
                Reference::Rows {
                    top: coord(top, lo),
                    bottom: coord(bottom, hi),
                }
            }
            Reference::Cols { left, right } if self.axis == Axis::Cols && self.span.is_none() => {
                let (lo, hi) = self.band(left.index, right.index)?;
                Reference::Cols {
                    left: coord(left, lo),
                    right: coord(right, hi),
                }
            }
            other => other,
        })
    }

    pub fn expr(&self, expr: &Expr, on_sheet: impl Fn(Option<&str>) -> bool) -> Expr {
        expr.map_references(&|sheet, reference| {
            if on_sheet(sheet) {
                self.reference(reference)
            } else {
                Some(reference)
            }
        })
    }

    pub fn check_merges(&self, merges: &[CellRange]) -> Result<(), ShiftError> {
        let Some((a, b)) = self.span else {
            return Ok(());
        };
        let split = merges.iter().any(|m| {
            let (lo, hi) = (self.across(m.start()), self.across(m.end()));
            let touches = lo <= b && a <= hi;
            touches && !(a <= lo && hi <= b) && self.along(m.end()) >= self.at
        });
        if split {
            Err(ShiftError::MergedCells)
        } else {
            Ok(())
        }
    }

    pub fn layout(&self, layout: &Layout) -> Layout {
        let mut out = layout.clone();
        if self.span.is_none() {
            match self.axis {
                Axis::Rows => {
                    out.row_heights = self.keys(&layout.row_heights);
                    out.hidden_rows = self.set(&layout.hidden_rows);
                    out.row_levels = self.keys(&layout.row_levels);
                    out.collapsed_rows = self.set(&layout.collapsed_rows);
                }
                Axis::Cols => {
                    out.col_widths = self.keys(&layout.col_widths);
                    out.hidden_cols = self.set(&layout.hidden_cols);
                    out.col_levels = self.keys(&layout.col_levels);
                    out.collapsed_cols = self.set(&layout.collapsed_cols);
                }
            }
        }
        out.merges = layout
            .merges
            .iter()
            .filter_map(|m| self.range(*m))
            .filter(|m| !m.is_single())
            .collect();
        out.filter = layout.filter.as_ref().and_then(|filter| {
            let range = self.range(filter.range)?;
            let mut filter = filter.clone();
            if self.axis == Axis::Cols && range != filter.range {
                filter.criteria = filter
                    .criteria
                    .into_iter()
                    .filter_map(|(col, shown)| Some((self.index(col)?, shown)))
                    .collect();
            }
            filter.range = range;
            Some(filter)
        });
        let page = &mut out.page;
        page.print_area = layout.page.print_area.and_then(|area| self.range(area));
        if self.span.is_none() {
            let (breaks, titles) = match self.axis {
                Axis::Rows => (&mut page.row_breaks, &mut page.title_rows),
                Axis::Cols => (&mut page.col_breaks, &mut page.title_cols),
            };
            *breaks = self.set(breaks);
            *titles = titles.and_then(|(a, b)| self.band(a, b));
        }
        out.conditional = layout
            .conditional
            .iter()
            .filter_map(|format| {
                let mut format = format.clone();
                format.range = self.range(format.range)?;
                Some(format)
            })
            .collect();
        out.notes = self.addrs(&layout.notes);
        out.comments = self.addrs(&layout.comments);
        out.links = self.addrs(&layout.links);
        out.tables = layout
            .tables
            .iter()
            .filter_map(|table| {
                let range = self.range(table.range)?;
                let mut table = table.clone();
                if range.cols() != table.range.cols() {
                    let left = table.range.start().col();
                    let mut columns = vec![crate::TableColumn::new(""); range.cols() as usize];
                    for (i, column) in table.columns.iter().enumerate() {
                        let slot = self
                            .index(left + i as u32)
                            .and_then(|col| col.checked_sub(range.start().col()))
                            .and_then(|j| columns.get_mut(j as usize));
                        if let Some(slot) = slot {
                            *slot = column.clone();
                        }
                    }
                    table.columns = columns;
                }
                table.range = range;
                Some(table)
            })
            .collect();
        out.validations = layout
            .validations
            .iter()
            .filter_map(|validation| {
                let mut validation = validation.clone();
                validation.ranges = validation
                    .ranges
                    .iter()
                    .filter_map(|range| self.range(*range))
                    .collect();
                for formula in [&mut validation.formula1, &mut validation.formula2] {
                    if let Ok(expr) = crate::formula::parse(formula, &crate::EN) {
                        *formula = self.expr(&expr, |sheet| sheet.is_none()).write(&crate::EN);
                    }
                }
                (!validation.ranges.is_empty()).then_some(validation)
            })
            .collect();
        out.charts = layout
            .charts
            .iter()
            .filter_map(|chart| {
                let mut chart = chart.clone();
                chart.data = self.range(chart.data)?;
                chart.anchor = self.addr(chart.anchor).unwrap_or_else(|| {
                    let along = self.at.min(self.max() - 1);
                    self.make(along, self.across(chart.anchor))
                });
                Some(chart)
            })
            .collect();
        let moved = |anchor: crate::CellAddr| {
            self.addr(anchor).unwrap_or_else(|| {
                let along = self.at.min(self.max() - 1);
                self.make(along, self.across(anchor))
            })
        };
        for picture in &mut out.pictures {
            picture.anchor = moved(picture.anchor);
        }
        for shape in &mut out.shapes {
            shape.anchor = moved(shape.anchor);
        }
        out.sparklines = layout
            .sparklines
            .iter()
            .filter_map(|group| {
                let mut group = group.clone();
                group.cells = group
                    .cells
                    .iter()
                    .filter_map(|(location, data)| {
                        Some((self.addr(*location)?, self.range(*data)?))
                    })
                    .collect();
                (!group.cells.is_empty()).then_some(group)
            })
            .collect();
        out
    }

    fn keys<V: Clone>(&self, map: &BTreeMap<u32, V>) -> BTreeMap<u32, V> {
        map.iter()
            .filter_map(|(&i, v)| Some((self.index(i)?, v.clone())))
            .collect()
    }

    fn addrs<V: Clone>(&self, map: &BTreeMap<CellAddr, V>) -> BTreeMap<CellAddr, V> {
        map.iter()
            .filter_map(|(addr, value)| Some((self.addr(*addr)?, value.clone())))
            .collect()
    }

    fn set(&self, set: &BTreeSet<u32>) -> BTreeSet<u32> {
        set.iter().filter_map(|&i| self.index(i)).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: CellRange,
    pub to: CellAddr,
}

impl Move {
    pub fn target(&self) -> CellRange {
        let end = self.offset(self.from.end());
        CellRange::new(self.to, end)
    }

    fn offset(&self, addr: CellAddr) -> CellAddr {
        let rows = i64::from(self.to.row()) - i64::from(self.from.start().row());
        let cols = i64::from(self.to.col()) - i64::from(self.from.start().col());
        addr.offset(rows, cols)
    }

    pub fn addr(&self, addr: CellAddr) -> Option<CellAddr> {
        if self.from.contains(addr) {
            Some(self.offset(addr))
        } else if self.target().contains(addr) {
            None
        } else {
            Some(addr)
        }
    }

    pub fn range(&self, range: CellRange) -> Option<CellRange> {
        if inside(range, self.from) {
            Some(CellRange::new(
                self.offset(range.start()),
                self.offset(range.end()),
            ))
        } else if inside(range, self.target()) {
            None
        } else {
            Some(range)
        }
    }

    fn reference(&self, reference: Reference) -> Option<Reference> {
        let coord = |c: Coord, index: u32| Coord {
            index,
            absolute: c.absolute,
        };
        Some(match reference {
            Reference::Cell { row, col } => {
                let addr = self.addr(CellAddr::new(row.index, col.index).ok()?)?;
                Reference::Cell {
                    row: coord(row, addr.row()),
                    col: coord(col, addr.col()),
                }
            }
            Reference::Area {
                top,
                left,
                bottom,
                right,
            } => {
                let range = self.range(reference.range())?;
                Reference::Area {
                    top: coord(top, range.start().row()),
                    left: coord(left, range.start().col()),
                    bottom: coord(bottom, range.end().row()),
                    right: coord(right, range.end().col()),
                }
            }
            other => other,
        })
    }

    pub fn expr(&self, expr: &Expr, on_sheet: impl Fn(Option<&str>) -> bool) -> Expr {
        expr.map_references(&|sheet, reference| {
            if on_sheet(sheet) {
                self.reference(reference)
            } else {
                Some(reference)
            }
        })
    }
}

pub fn inside(inner: CellRange, outer: CellRange) -> bool {
    outer.contains(inner.start()) && outer.contains(inner.end())
}

fn perpendicular_max(axis: Axis) -> u32 {
    match axis {
        Axis::Rows => MAX_COLS,
        Axis::Cols => MAX_ROWS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EN;
    use crate::formula::parse;

    fn addr(s: &str) -> CellAddr {
        range(s).start()
    }

    fn range(s: &str) -> CellRange {
        s.parse().unwrap()
    }

    fn rewrite(src: &str, shift: Shift) -> String {
        shift
            .expr(&parse(src, &EN).unwrap(), |sheet| sheet.is_none())
            .write(&EN)
    }

    #[test]
    fn inserting_rows_moves_and_grows_references() {
        let shift = Shift::rows(2, 2, true);
        assert_eq!(
            rewrite("A1+$A$3+SUM(A1:A5)+SUM(A4:B6)+3:4+B:B", shift),
            "A1+$A$5+SUM(A1:A7)+SUM(A6:B8)+5:6+B:B"
        );
    }

    #[test]
    fn deleting_rows_shrinks_ranges_and_breaks_deleted_cells() {
        let shift = Shift::rows(2, 2, false);
        assert_eq!(
            rewrite("A3+A5+SUM(A1:A6)+SUM(A3:A4)+SUM(A4:A8)", shift),
            "#REF!+A3+SUM(A1:A4)+SUM(#REF!)+SUM(A3:A6)"
        );
    }

    #[test]
    fn column_changes_follow_the_same_rules() {
        assert_eq!(
            rewrite("C1+SUM(A1:D1)+C:D", Shift::cols(1, 1, true)),
            "D1+SUM(A1:E1)+D:E"
        );
        assert_eq!(
            rewrite("B1+C1+A:C", Shift::cols(1, 1, false)),
            "#REF!+B1+A:B"
        );
    }

    #[test]
    fn shifting_cells_moves_only_ranges_inside_the_span() {
        let shift = Shift::cells(range("B2:B3"), Axis::Rows, true);
        assert_eq!(
            rewrite("B2+A2+SUM(B1:B5)+SUM(A1:B5)+B:B", shift),
            "B4+A2+SUM(B1:B7)+SUM(A1:B5)+B:B"
        );
        let shift = Shift::cells(range("B2:C2"), Axis::Cols, false);
        assert_eq!(rewrite("B2+D2+D3", shift), "#REF!+B2+D3");
    }

    #[test]
    fn whole_rows_selected_as_cells_become_row_shifts() {
        let shift = Shift::cells(range("A3:XFD4"), Axis::Rows, true);
        assert_eq!(shift, Shift::rows(2, 2, true));
    }

    #[test]
    fn ranges_at_the_sheet_edge_stay_inside() {
        let shift = Shift::rows(0, 1, true);
        assert_eq!(rewrite("SUM(A1:A1048576)", shift), "SUM(A2:A1048576)");
        assert_eq!(rewrite("A1048576", shift), "#REF!");
    }

    #[test]
    fn merges_straddling_the_span_block_cell_shifts() {
        let shift = Shift::cells(range("B2"), Axis::Rows, true);
        assert_eq!(
            shift.check_merges(&[range("A5:B5")]),
            Err(ShiftError::MergedCells)
        );
        assert_eq!(shift.check_merges(&[range("A1:B1")]), Ok(()));
        assert_eq!(shift.check_merges(&[range("B5:B6")]), Ok(()));
    }

    #[test]
    fn moving_cells_carries_references_along() {
        let shift = Move {
            from: range("A1:A2"),
            to: "C5".parse().unwrap(),
        };
        let rewrite = |src: &str| {
            shift
                .expr(&parse(src, &EN).unwrap(), |sheet| sheet.is_none())
                .write(&EN)
        };
        assert_eq!(rewrite("A1+$A$2+SUM(A1:A2)+B1"), "C5+$C$6+SUM(C5:C6)+B1");
        assert_eq!(
            rewrite("C5+SUM(A1:A3)+SUM(C5:C6)"),
            "#REF!+SUM(A1:A3)+SUM(#REF!)"
        );
    }

    #[test]
    fn layout_follows_the_shift() {
        let mut layout = Layout::default();
        layout.row_heights.insert(4, 30.0);
        layout.hidden_rows.insert(1);
        layout.hidden_rows.insert(5);
        layout.merges.push(range("A3:B4"));
        layout.merges.push(range("A6:A7"));
        let out = Shift::rows(3, 2, false).layout(&layout);
        assert!(out.row_heights.is_empty());
        assert_eq!(out.hidden_rows, BTreeSet::from([1, 3]));
        assert_eq!(out.merges, vec![range("A3:B3"), range("A4:A5")]);
    }

    #[test]
    fn pictures_and_shapes_move_with_rows() {
        use crate::drawing::{Shape, ShapeKind};
        let mut layout = Layout::default();
        layout
            .shapes
            .push(Shape::new(ShapeKind::Ellipse, addr("C6")));
        layout
            .shapes
            .push(Shape::new(ShapeKind::TextBox, addr("C4")));
        let out = Shift::rows(1, 2, true).layout(&layout);
        assert_eq!(out.shapes[0].anchor, addr("C8"));
        let out = Shift::rows(3, 2, false).layout(&layout);
        assert_eq!(out.shapes[0].anchor, addr("C4"));
        assert_eq!(out.shapes[1].anchor, addr("C4"));
    }
}
