use std::fmt;
use std::str::FromStr;

use crate::{AddressError, CellAddr, MAX_COLS, MAX_ROWS, column_index};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellRange {
    start: CellAddr,
    end: CellAddr,
}

impl CellRange {
    pub fn new(a: CellAddr, b: CellAddr) -> Self {
        let corner = |row, col| CellAddr::new(row, col).expect("corner lies between valid cells");
        Self {
            start: corner(a.row().min(b.row()), a.col().min(b.col())),
            end: corner(a.row().max(b.row()), a.col().max(b.col())),
        }
    }

    pub fn including(self, addr: CellAddr) -> Self {
        Self::new(
            Self::new(self.start, addr).start,
            Self::new(self.end, addr).end,
        )
    }

    pub fn single(addr: CellAddr) -> Self {
        Self {
            start: addr,
            end: addr,
        }
    }

    pub fn start(self) -> CellAddr {
        self.start
    }

    pub fn end(self) -> CellAddr {
        self.end
    }

    pub fn rows(self) -> u32 {
        self.end.row() - self.start.row() + 1
    }

    pub fn cols(self) -> u32 {
        self.end.col() - self.start.col() + 1
    }

    pub fn is_single(self) -> bool {
        self.start == self.end
    }

    pub fn contains(self, addr: CellAddr) -> bool {
        (self.start.row()..=self.end.row()).contains(&addr.row())
            && (self.start.col()..=self.end.col()).contains(&addr.col())
    }

    pub fn is_full_rows(self) -> bool {
        self.start.col() == 0 && self.end.col() == MAX_COLS - 1
    }

    pub fn is_full_cols(self) -> bool {
        self.start.row() == 0 && self.end.row() == MAX_ROWS - 1
    }
}

impl FromStr for CellRange {
    type Err = AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((a, b)) = s.split_once(':') else {
            return Ok(Self::single(s.parse()?));
        };
        let at = |row, col| CellAddr::new(row, col);
        if let (Ok(left), Ok(right)) = (column_index(a), column_index(b)) {
            return Ok(Self::new(at(0, left)?, at(MAX_ROWS - 1, right)?));
        }
        if let (Some(top), Some(bottom)) = (row_number(a), row_number(b)) {
            return Ok(Self::new(at(top, 0)?, at(bottom, MAX_COLS - 1)?));
        }
        Ok(Self::new(a.parse()?, b.parse()?))
    }
}

fn row_number(s: &str) -> Option<u32> {
    let valid = !s.is_empty() && !s.starts_with('0') && s.bytes().all(|b| b.is_ascii_digit());
    let n: u32 = s.parse().ok().filter(|_| valid)?;
    (1..=MAX_ROWS).contains(&n).then(|| n - 1)
}

impl fmt::Display for CellRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_single() {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}:{}", self.start, self.end)
        }
    }
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

    #[test]
    fn normalizes_corners() {
        let r = CellRange::new(addr("C1"), addr("A3"));
        assert_eq!(r.start(), addr("A1"));
        assert_eq!(r.end(), addr("C3"));
        assert_eq!((r.rows(), r.cols()), (3, 3));
    }

    #[test]
    fn including_grows_to_cover_cell() {
        assert_eq!(range("B2:C3").including(addr("A5")), range("A2:C5"));
        assert_eq!(range("B2").including(addr("B2")), range("B2"));
    }

    #[test]
    fn parses_and_displays() {
        assert_eq!(range("b2:a1").to_string(), "A1:B2");
        assert_eq!(range("C5").to_string(), "C5");
        assert_eq!(range("A1:A1").to_string(), "A1");
        assert!("A1:".parse::<CellRange>().is_err());
        assert!("A1:B2:C3".parse::<CellRange>().is_err());
    }

    #[test]
    fn parses_whole_columns_and_rows() {
        assert_eq!(range("B:D"), range("B1:D1048576"));
        assert_eq!(range("3:1"), range("A1:XFD3"));
        assert!("A:1".parse::<CellRange>().is_err());
        assert!("0:1".parse::<CellRange>().is_err());
    }

    #[test]
    fn contains_cells_inside_only() {
        let r = range("B2:C3");
        assert!(r.contains(addr("B2")));
        assert!(r.contains(addr("C3")));
        assert!(!r.contains(addr("A2")));
        assert!(!r.contains(addr("C4")));
    }

    #[test]
    fn detects_full_rows_and_columns() {
        assert!(range("A2:XFD3").is_full_rows());
        assert!(range("B1:B1048576").is_full_cols());
        assert!(!range("A1:C3").is_full_rows());
    }
}
