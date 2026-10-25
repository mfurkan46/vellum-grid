use std::collections::{BTreeSet, HashMap};

use crate::{CellAddr, CellRange, CellValue, Sheet, current_region};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kinds {
    pub numbers: bool,
    pub text: bool,
    pub logicals: bool,
    pub errors: bool,
}

impl Kinds {
    pub const ALL: Self = Self {
        numbers: true,
        text: true,
        logicals: true,
        errors: true,
    };

    fn matches(self, value: &CellValue) -> bool {
        match value {
            CellValue::Number(_) => self.numbers,
            CellValue::Text(_) => self.text,
            CellValue::Bool(_) => self.logicals,
            CellValue::Error(_) => self.errors,
            CellValue::Empty => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Special {
    Blanks,
    Constants(Kinds),
    Formulas(Kinds),
    CurrentRegion,
    LastCell,
    Visible,
    Conditional,
}

pub fn go_to_special(
    sheet: &Sheet,
    selection: CellRange,
    active: CellAddr,
    special: Special,
) -> Vec<CellRange> {
    let (rows, cols) = sheet.extent();
    if rows == 0 {
        return match special {
            Special::CurrentRegion => vec![CellRange::single(active)],
            _ => Vec::new(),
        };
    }
    let used = CellRange::new(
        CellAddr::new(0, 0).expect("A1 is valid"),
        CellAddr::new(rows - 1, cols - 1).expect("extent is inside the sheet"),
    );
    let area = if selection.is_single() {
        used
    } else {
        clip(selection, used).unwrap_or(selection)
    };
    let cells: BTreeSet<CellAddr> = match special {
        Special::CurrentRegion => return vec![current_region(sheet, active)],
        Special::LastCell => return vec![CellRange::single(used.end())],
        Special::Conditional => {
            return sheet
                .layout()
                .conditional
                .iter()
                .map(|format| format.range)
                .collect();
        }
        Special::Blanks => addresses(area)
            .filter(|addr| sheet.get(*addr).is_none_or(|cell| !cell.has_content()))
            .collect(),
        Special::Constants(kinds) => sheet
            .cells_in(area)
            .filter(|(_, cell)| cell.formula.is_none() && kinds.matches(&cell.value))
            .map(|(addr, _)| addr)
            .collect(),
        Special::Formulas(kinds) => sheet
            .cells_in(area)
            .filter(|(_, cell)| {
                cell.formula.is_some() && (cell.value.is_empty() || kinds.matches(&cell.value))
            })
            .map(|(addr, _)| addr)
            .collect(),
        Special::Visible => addresses(area)
            .filter(|addr| !sheet.layout().is_hidden(*addr))
            .collect(),
    };
    blocks(&cells)
}

fn clip(range: CellRange, bounds: CellRange) -> Option<CellRange> {
    let top = range.start().row().max(bounds.start().row());
    let left = range.start().col().max(bounds.start().col());
    let bottom = range.end().row().min(bounds.end().row());
    let right = range.end().col().min(bounds.end().col());
    (top <= bottom && left <= right).then(|| {
        CellRange::new(
            CellAddr::new(top, left).expect("inside the sheet"),
            CellAddr::new(bottom, right).expect("inside the sheet"),
        )
    })
}

fn addresses(area: CellRange) -> impl Iterator<Item = CellAddr> {
    (area.start().row()..=area.end().row()).flat_map(move |row| {
        (area.start().col()..=area.end().col())
            .map(move |col| CellAddr::new(row, col).expect("inside the area"))
    })
}

fn blocks(cells: &BTreeSet<CellAddr>) -> Vec<CellRange> {
    let at = |row, col| CellAddr::new(row, col).expect("inside the sheet");
    let mut runs: Vec<(u32, u32, u32)> = Vec::new();
    for addr in cells {
        match runs.last_mut() {
            Some((row, _, end)) if *row == addr.row() && *end + 1 == addr.col() => {
                *end = addr.col();
            }
            _ => runs.push((addr.row(), addr.col(), addr.col())),
        }
    }
    let mut ranges: Vec<CellRange> = Vec::new();
    let mut previous: HashMap<(u32, u32), usize> = HashMap::new();
    let mut current: HashMap<(u32, u32), usize> = HashMap::new();
    let mut row_now = None;
    for (row, left, right) in runs {
        if row_now != Some(row) {
            previous = if row_now.is_some_and(|r: u32| r + 1 == row) {
                std::mem::take(&mut current)
            } else {
                current.clear();
                HashMap::new()
            };
            row_now = Some(row);
        }
        let index = match previous.remove(&(left, right)) {
            Some(index) => {
                ranges[index] = ranges[index].including(at(row, right));
                index
            }
            None => {
                ranges.push(CellRange::new(at(row, left), at(row, right)));
                ranges.len() - 1
            }
        };
        current.insert((left, right), index);
    }
    ranges.sort_by_key(|r| (r.start().row(), r.start().col()));
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn range(s: &str) -> CellRange {
        s.parse().unwrap()
    }

    fn sheet() -> Sheet {
        let mut sheet = Sheet::new("S");
        for (at, input) in [
            ("A1", "1"),
            ("B1", "metin"),
            ("A2", "2"),
            ("B2", "=A2*2"),
            ("A3", "3"),
            ("C3", "TRUE"),
        ] {
            sheet.set_input(addr(at), input);
        }
        sheet
    }

    #[test]
    fn finds_constants_formulas_and_blanks_as_blocks() {
        let sheet = sheet();
        let all = range("A1:C3");
        let numbers = Kinds {
            text: false,
            logicals: false,
            errors: false,
            ..Kinds::ALL
        };
        assert_eq!(
            go_to_special(&sheet, all, addr("A1"), Special::Constants(numbers)),
            vec![range("A1:A3")]
        );
        assert_eq!(
            go_to_special(
                &sheet,
                range("A1"),
                addr("A1"),
                Special::Formulas(Kinds::ALL)
            ),
            vec![range("B2")]
        );
        assert_eq!(
            go_to_special(&sheet, all, addr("A1"), Special::Blanks),
            vec![range("C1:C2"), range("B3")]
        );
    }

    #[test]
    fn finds_regions_last_cell_and_visible_cells() {
        let mut sheet = sheet();
        assert_eq!(
            go_to_special(&sheet, range("A1"), addr("A1"), Special::LastCell),
            vec![range("C3")]
        );
        assert_eq!(
            go_to_special(&sheet, range("A1"), addr("B2"), Special::CurrentRegion),
            vec![range("A1:C3")]
        );
        sheet.layout_mut().hidden_rows.insert(1);
        assert_eq!(
            go_to_special(&sheet, range("A1:C3"), addr("A1"), Special::Visible),
            vec![range("A1:C1"), range("A3:C3")]
        );
    }
}
