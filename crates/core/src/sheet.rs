use std::collections::{BTreeMap, BTreeSet};

use crate::{Cell, CellAddr, CellRange, CellValue, MAX_COLS, MAX_ROWS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub const DEFAULT_COLUMN_WIDTH: f64 = 9.140625;
pub const DEFAULT_ROW_HEIGHT: f64 = 15.0;

#[derive(Debug, Clone, PartialEq)]
pub struct AutoFilter {
    pub range: CellRange,
    pub criteria: BTreeMap<u32, crate::filter::ColumnFilter>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layout {
    pub col_widths: BTreeMap<u32, f64>,
    pub row_heights: BTreeMap<u32, f64>,
    pub merges: Vec<CellRange>,
    pub hide_gridlines: bool,
    pub hide_headings: bool,
    pub freeze_rows: u32,
    pub freeze_cols: u32,
    pub hidden_rows: BTreeSet<u32>,
    pub hidden_cols: BTreeSet<u32>,
    pub filter: Option<AutoFilter>,
    pub conditional: Vec<crate::ConditionalFormat>,
    pub charts: Vec<crate::Chart>,
    pub tab_color: Option<crate::Color>,
    pub sheet_hidden: bool,
    pub page: crate::PageSetup,
    pub ignored_errors: BTreeSet<CellAddr>,
    pub validations: Vec<crate::validation::Validation>,
    pub row_levels: BTreeMap<u32, u8>,
    pub col_levels: BTreeMap<u32, u8>,
    pub collapsed_rows: BTreeSet<u32>,
    pub collapsed_cols: BTreeSet<u32>,
    pub notes: BTreeMap<CellAddr, crate::review::Note>,
    pub comments: BTreeMap<CellAddr, crate::review::Comment>,
    pub links: BTreeMap<CellAddr, crate::review::Link>,
    pub protection: Option<crate::protect::Protection>,
    pub tables: Vec<crate::table::Table>,
    pub sparklines: Vec<crate::SparklineGroup>,
    pub pictures: Vec<crate::drawing::Picture>,
    pub shapes: Vec<crate::drawing::Shape>,
}

impl Layout {
    pub fn col_width(&self, col: u32) -> f64 {
        self.col_widths
            .get(&col)
            .copied()
            .unwrap_or(DEFAULT_COLUMN_WIDTH)
    }

    pub fn row_height(&self, row: u32) -> f64 {
        self.row_heights
            .get(&row)
            .copied()
            .unwrap_or(DEFAULT_ROW_HEIGHT)
    }

    pub fn is_hidden(&self, addr: CellAddr) -> bool {
        self.hidden_rows.contains(&addr.row()) || self.hidden_cols.contains(&addr.col())
    }

    pub fn merge_at(&self, addr: CellAddr) -> Option<CellRange> {
        self.merges.iter().copied().find(|m| m.contains(addr))
    }

    pub fn expand(&self, range: CellRange) -> CellRange {
        let mut range = range;
        loop {
            let grown = self
                .merges
                .iter()
                .filter(|m| overlaps(**m, range))
                .fold(range, |r, m| r.including(m.start()).including(m.end()));
            if grown == range {
                return range;
            }
            range = grown;
        }
    }
}

pub fn overlaps(a: CellRange, b: CellRange) -> bool {
    a.start().row() <= b.end().row()
        && b.start().row() <= a.end().row()
        && a.start().col() <= b.end().col()
        && b.start().col() <= a.end().col()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    name: String,
    cells: BTreeMap<CellAddr, Cell>,
    layout: Layout,
}

impl Sheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: BTreeMap::new(),
            layout: Layout::default(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn rename(&mut self, name: String) {
        self.name = name;
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    pub fn get(&self, addr: CellAddr) -> Option<&Cell> {
        self.cells.get(&addr)
    }

    pub fn value(&self, addr: CellAddr) -> &CellValue {
        static EMPTY: CellValue = CellValue::Empty;
        self.cells.get(&addr).map_or(&EMPTY, |cell| &cell.value)
    }

    pub(crate) fn cell_mut(&mut self, addr: CellAddr) -> Option<&mut Cell> {
        self.cells.get_mut(&addr)
    }

    pub fn set(&mut self, addr: CellAddr, cell: impl Into<Cell>) -> Option<Cell> {
        let cell = cell.into();
        if cell.is_blank() {
            self.cells.remove(&addr)
        } else {
            self.cells.insert(addr, cell)
        }
    }

    pub fn set_input(&mut self, addr: CellAddr, input: &str) -> Option<Cell> {
        self.set(addr, Cell::parse(input))
    }

    pub fn clear(&mut self, addr: CellAddr) -> Option<Cell> {
        self.cells.remove(&addr)
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn cells(&self) -> impl Iterator<Item = (CellAddr, &Cell)> {
        self.cells.iter().map(|(addr, cell)| (*addr, cell))
    }

    pub fn cells_in(&self, range: CellRange) -> impl Iterator<Item = (CellAddr, &Cell)> {
        let row_start = CellAddr::new(range.start().row(), 0).expect("row is valid");
        let row_end = CellAddr::new(range.end().row(), MAX_COLS - 1).expect("row is valid");
        self.cells
            .range(row_start..=row_end)
            .filter(move |(addr, _)| range.contains(**addr))
            .map(|(addr, cell)| (*addr, cell))
    }

    pub fn trim_to_extent(&self, range: CellRange) -> CellRange {
        let (rows, cols) = self.extent();
        let start = range.start();
        let row = range
            .end()
            .row()
            .min(rows.saturating_sub(1))
            .max(start.row());
        let col = range
            .end()
            .col()
            .min(cols.saturating_sub(1))
            .max(start.col());
        CellRange::new(start, CellAddr::new(row, col).expect("inside the range"))
    }

    pub fn jump(&self, from: CellAddr, direction: Direction) -> CellAddr {
        let vertical = matches!(direction, Direction::Up | Direction::Down);
        let forward = matches!(direction, Direction::Down | Direction::Right);
        let (along, max, line): (u32, u32, Vec<u32>) = if vertical {
            let rows = self
                .cells
                .iter()
                .filter(|(a, cell)| a.col() == from.col() && cell.has_content());
            (
                from.row(),
                MAX_ROWS - 1,
                rows.map(|(a, _)| a.row()).collect(),
            )
        } else {
            let first = CellAddr::new(from.row(), 0).expect("row is valid");
            let last = CellAddr::new(from.row(), MAX_COLS - 1).expect("row is valid");
            let cols = self
                .cells
                .range(first..=last)
                .filter(|(_, cell)| cell.has_content())
                .map(|(a, _)| a.col());
            (from.col(), MAX_COLS - 1, cols.collect())
        };
        let filled = |p: u32| line.binary_search(&p).is_ok();
        let step = |p: u32| {
            if forward {
                (p < max).then(|| p + 1)
            } else {
                p.checked_sub(1)
            }
        };
        let target = match step(along) {
            None => along,
            Some(next) if filled(along) && filled(next) => {
                let mut p = next;
                while let Some(n) = step(p).filter(|&n| filled(n)) {
                    p = n;
                }
                p
            }
            Some(_) if forward => line.iter().copied().find(|&p| p > along).unwrap_or(max),
            Some(_) => line.iter().copied().rev().find(|&p| p < along).unwrap_or(0),
        };
        let (row, col) = if vertical {
            (target, from.col())
        } else {
            (from.row(), target)
        };
        CellAddr::new(row, col).expect("jump stays within the sheet")
    }

    pub fn extent(&self) -> (u32, u32) {
        let rows = self
            .cells
            .keys()
            .next_back()
            .map_or(0, |addr| addr.row() + 1);
        let cols = self
            .cells
            .keys()
            .map(|addr| addr.col() + 1)
            .max()
            .unwrap_or(0);
        (rows, cols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_COLS, MAX_ROWS};

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    #[test]
    fn new_sheet_is_empty() {
        let sheet = Sheet::new("Data");
        assert_eq!(sheet.name(), "Data");
        assert!(sheet.is_empty());
        assert_eq!(sheet.extent(), (0, 0));
        assert_eq!(sheet.value(addr("A1")), &CellValue::Empty);
        assert_eq!(sheet.get(addr("A1")), None);
    }

    #[test]
    fn set_get_and_overwrite() {
        let mut sheet = Sheet::new("S");
        assert_eq!(sheet.set(addr("B2"), CellValue::Number(1.0)), None);
        assert_eq!(sheet.value(addr("B2")), &CellValue::Number(1.0));

        let previous = sheet.set(addr("B2"), CellValue::Text("x".into()));
        assert_eq!(previous, Some(CellValue::Number(1.0).into()));
        assert_eq!(sheet.value(addr("B2")), &CellValue::Text("x".into()));
        assert_eq!(sheet.len(), 1);
    }

    #[test]
    fn clear_removes_cell() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("C3"), "TRUE");
        assert_eq!(sheet.clear(addr("C3")), Some(CellValue::Bool(true).into()));
        assert_eq!(sheet.clear(addr("C3")), None);
        assert!(sheet.is_empty());
    }

    #[test]
    fn storing_blank_removes_cell() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("A1"), "5");
        sheet.set_input(addr("A1"), "");
        assert!(sheet.is_empty());
        sheet.set(addr("A2"), CellValue::Empty);
        assert!(sheet.is_empty());
    }

    #[test]
    fn formula_cell_is_stored_without_value() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("A3"), "=A1+A2");
        let cell = sheet.get(addr("A3")).unwrap();
        assert_eq!(cell.formula.as_deref(), Some("=A1+A2"));
        assert_eq!(sheet.len(), 1);
    }

    #[test]
    fn sparse_storage_handles_far_cells() {
        let mut sheet = Sheet::new("S");
        let corner = CellAddr::new(MAX_ROWS - 1, MAX_COLS - 1).unwrap();
        sheet.set(corner, CellValue::Number(7.0));
        sheet.set_input(addr("A1"), "start");
        assert_eq!(sheet.len(), 2);
        assert_eq!(sheet.extent(), (MAX_ROWS, MAX_COLS));
    }

    #[test]
    fn extent_covers_used_cells() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("D2"), "a");
        sheet.set_input(addr("B5"), "b");
        assert_eq!(sheet.extent(), (5, 4));
    }

    #[test]
    fn layout_defaults_and_merges() {
        let mut layout = Layout::default();
        assert_eq!(layout.col_width(3), DEFAULT_COLUMN_WIDTH);
        assert_eq!(layout.row_height(3), DEFAULT_ROW_HEIGHT);
        layout.merges.push("B2:C3".parse().unwrap());
        layout.merges.push("D3:E4".parse().unwrap());
        assert_eq!(layout.merge_at(addr("C3")), Some("B2:C3".parse().unwrap()));
        assert_eq!(layout.merge_at(addr("A1")), None);
        assert_eq!(layout.expand("C2".parse().unwrap()).to_string(), "B2:C3");
        assert_eq!(layout.expand("C3:D3".parse().unwrap()).to_string(), "B2:E4");
    }

    #[test]
    fn jump_skips_cells_that_only_have_formatting() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("A1"), "x");
        let mut styled = Cell::default();
        styled.set_style(crate::Style::with_number_format("0.00"));
        sheet.set(addr("A5"), styled);
        assert_eq!(sheet.len(), 2);
        assert_eq!(
            sheet.jump(addr("A1"), Direction::Down).to_string(),
            "A1048576"
        );
    }

    #[test]
    fn cells_in_range_only() {
        let mut sheet = Sheet::new("S");
        for s in ["A1", "B2", "C2", "D2", "B3", "B9"] {
            sheet.set_input(addr(s), s);
        }
        let inside: Vec<String> = sheet
            .cells_in("B2:C3".parse().unwrap())
            .map(|(a, _)| a.to_string())
            .collect();
        assert_eq!(inside, ["B2", "C2", "B3"]);
    }

    #[test]
    fn trims_range_to_used_area() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("B3"), "x");
        let trim = |s: &str| sheet.trim_to_extent(s.parse().unwrap()).to_string();
        assert_eq!(trim("A1:XFD1048576"), "A1:B3");
        assert_eq!(trim("B1:B1048576"), "B1:B3");
        assert_eq!(trim("A2:A3"), "A2:A3");
        assert_eq!(trim("D5:F9"), "D5");
    }

    fn jump_sheet() -> Sheet {
        let mut sheet = Sheet::new("S");
        for s in ["A1", "A2", "A3", "A6", "C1", "D1", "F1"] {
            sheet.set_input(addr(s), "x");
        }
        sheet
    }

    #[test]
    fn jump_down_follows_ctrl_arrow() {
        let sheet = jump_sheet();
        let down = |s| sheet.jump(addr(s), Direction::Down).to_string();
        assert_eq!(down("A1"), "A3");
        assert_eq!(down("A3"), "A6");
        assert_eq!(down("A4"), "A6");
        assert_eq!(down("A6"), "A1048576");
        assert_eq!(down("B1"), "B1048576");
        assert_eq!(down("A1048576"), "A1048576");
    }

    #[test]
    fn jump_up_follows_ctrl_arrow() {
        let sheet = jump_sheet();
        let up = |s| sheet.jump(addr(s), Direction::Up).to_string();
        assert_eq!(up("A1048576"), "A6");
        assert_eq!(up("A6"), "A3");
        assert_eq!(up("A3"), "A1");
        assert_eq!(up("A5"), "A3");
        assert_eq!(up("A1"), "A1");
        assert_eq!(up("B9"), "B1");
    }

    #[test]
    fn jump_horizontally() {
        let sheet = jump_sheet();
        let right = |s| sheet.jump(addr(s), Direction::Right).to_string();
        let left = |s| sheet.jump(addr(s), Direction::Left).to_string();
        assert_eq!(right("A1"), "C1");
        assert_eq!(right("C1"), "D1");
        assert_eq!(right("D1"), "F1");
        assert_eq!(right("F1"), "XFD1");
        assert_eq!(left("F1"), "D1");
        assert_eq!(left("D1"), "C1");
        assert_eq!(left("C1"), "A1");
        assert_eq!(left("A2"), "A2");
    }

    #[test]
    fn cells_iterate_in_row_major_order() {
        let mut sheet = Sheet::new("S");
        for s in ["B2", "A2", "C1", "A1"] {
            sheet.set_input(addr(s), s);
        }
        let order: Vec<String> = sheet.cells().map(|(a, _)| a.to_string()).collect();
        assert_eq!(order, ["A1", "C1", "A2", "B2"]);
    }
}
