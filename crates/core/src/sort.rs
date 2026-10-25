use std::cmp::Ordering;

use crate::{CellAddr, CellRange, CellValue, Locale, Sheet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortKey {
    pub col: u32,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortError {
    MergedCells,
}

pub fn compare_values(a: &CellValue, b: &CellValue, locale: &Locale) -> Ordering {
    let rank = |value: &CellValue| match value {
        CellValue::Number(_) => 0,
        CellValue::Text(_) => 1,
        CellValue::Bool(_) => 2,
        CellValue::Error(_) => 3,
        CellValue::Empty => 4,
    };
    match (a, b) {
        (CellValue::Number(x), CellValue::Number(y)) => x.total_cmp(y),
        (CellValue::Text(x), CellValue::Text(y)) => locale.collate(x, y),
        (CellValue::Bool(x), CellValue::Bool(y)) => x.cmp(y),
        _ => rank(a).cmp(&rank(b)),
    }
}

pub fn sorted_rows(sheet: &Sheet, rows: &[u32], keys: &[SortKey], locale: &Locale) -> Vec<u32> {
    let value = |row: u32, col: u32| {
        CellAddr::new(row, col)
            .ok()
            .map_or(&CellValue::Empty, |addr| sheet.value(addr))
    };
    let mut order = rows.to_vec();
    order.sort_by(|&a, &b| {
        keys.iter()
            .map(|key| {
                let (x, y) = (value(a, key.col), value(b, key.col));
                match (x.is_empty(), y.is_empty()) {
                    (true, true) => Ordering::Equal,
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    _ if key.descending => compare_values(y, x, locale),
                    _ => compare_values(x, y, locale),
                }
            })
            .find(|ordering| ordering.is_ne())
            .unwrap_or(Ordering::Equal)
    });
    order
}

pub fn guess_header(sheet: &Sheet, range: CellRange) -> bool {
    if range.rows() < 2 {
        return false;
    }
    let row = |r: u32| {
        (range.start().col()..=range.end().col())
            .map(move |col| CellAddr::new(r, col).expect("inside the range"))
    };
    let first = range.start().row();
    let texts = row(first)
        .filter_map(|addr| sheet.get(addr))
        .filter(|cell| cell.has_content())
        .collect::<Vec<_>>();
    if texts.is_empty()
        || texts
            .iter()
            .any(|cell| !matches!(cell.value, CellValue::Text(_)))
    {
        return false;
    }
    let below = row(first + 1)
        .filter_map(|addr| sheet.get(addr))
        .filter(|cell| cell.has_content())
        .collect::<Vec<_>>();
    below
        .iter()
        .any(|cell| !matches!(cell.value, CellValue::Text(_)))
        || (texts.iter().all(|cell| cell.style().bold)
            && below.iter().any(|cell| !cell.style().bold))
}

pub fn current_region(sheet: &Sheet, addr: CellAddr) -> CellRange {
    let mut range = CellRange::single(addr);
    loop {
        let (start, end) = (range.start(), range.end());
        let top = start.row().saturating_sub(1);
        let left = start.col().saturating_sub(1);
        let bottom = (end.row() + 1).min(crate::MAX_ROWS - 1);
        let right = (end.col() + 1).min(crate::MAX_COLS - 1);
        let ring = CellRange::new(
            CellAddr::new(top, left).expect("inside the sheet"),
            CellAddr::new(bottom, right).expect("inside the sheet"),
        );
        let grown = sheet
            .cells_in(ring)
            .filter(|(at, cell)| cell.has_content() && !range.contains(*at))
            .fold(range, |r, (at, _)| r.including(at));
        if grown == range {
            return range;
        }
        range = grown;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cell, TR};

    fn sheet(inputs: &[(&str, &str)]) -> Sheet {
        let mut sheet = Sheet::new("s");
        for (at, input) in inputs {
            sheet.set(at.parse().unwrap(), Cell::parse_in(input, &TR).unwrap());
        }
        sheet
    }

    #[test]
    fn turkish_alphabet_order() {
        let mut words = vec![
            "şeker", "Zeytin", "ıhlamur", "çay", "inci", "Ceviz", "öğle", "ot",
        ];
        words.sort_by(|a, b| TR.collate(a, b));
        assert_eq!(
            words,
            [
                "Ceviz", "çay", "ıhlamur", "inci", "ot", "öğle", "şeker", "Zeytin"
            ]
        );
    }

    #[test]
    fn types_and_blanks_order() {
        let s = sheet(&[
            ("A1", "elma"),
            ("A2", "DOĞRU"),
            ("A3", "5"),
            ("A5", "#YOK"),
            ("A6", "-2"),
        ]);
        let rows = [0, 1, 2, 3, 4, 5];
        let up = sorted_rows(
            &s,
            &rows,
            &[SortKey {
                col: 0,
                descending: false,
            }],
            &TR,
        );
        assert_eq!(up, [5, 2, 0, 1, 4, 3]);
        let down = sorted_rows(
            &s,
            &rows,
            &[SortKey {
                col: 0,
                descending: true,
            }],
            &TR,
        );
        assert_eq!(down, [4, 1, 0, 2, 5, 3]);
    }

    #[test]
    fn later_keys_break_ties() {
        let s = sheet(&[
            ("A1", "b"),
            ("B1", "1"),
            ("A2", "a"),
            ("B2", "2"),
            ("A3", "b"),
            ("B3", "0"),
        ]);
        let keys = [
            SortKey {
                col: 0,
                descending: false,
            },
            SortKey {
                col: 1,
                descending: true,
            },
        ];
        assert_eq!(sorted_rows(&s, &[0, 1, 2], &keys, &TR), [1, 0, 2]);
    }

    #[test]
    fn finds_regions_and_headers() {
        let s = sheet(&[
            ("B2", "Ad"),
            ("C2", "Tutar"),
            ("B3", "x"),
            ("C3", "5"),
            ("D4", "9"),
            ("F9", "uzak"),
        ]);
        let region = current_region(&s, "C3".parse().unwrap());
        assert_eq!(region, "B2:D4".parse().unwrap());
        assert!(guess_header(&s, region));
        assert!(!guess_header(&s, "B3:C4".parse().unwrap()));
        assert_eq!(
            current_region(&s, "A1".parse().unwrap()),
            "A1:D4".parse().unwrap()
        );
        assert_eq!(
            current_region(&s, "H1".parse().unwrap()),
            "H1".parse().unwrap()
        );
    }
}
