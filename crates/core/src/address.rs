use std::fmt;
use std::str::FromStr;

pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u32 = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellAddr {
    row: u32,
    col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressError {
    Syntax(String),
    RowOutOfRange,
    ColumnOutOfRange,
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(s) => write!(f, "invalid cell address '{s}'"),
            Self::RowOutOfRange => write!(f, "row out of range (1..={MAX_ROWS})"),
            Self::ColumnOutOfRange => {
                write!(f, "column out of range (A..={})", column_name(MAX_COLS - 1))
            }
        }
    }
}

impl std::error::Error for AddressError {}

impl CellAddr {
    pub fn new(row: u32, col: u32) -> Result<Self, AddressError> {
        if row >= MAX_ROWS {
            return Err(AddressError::RowOutOfRange);
        }
        if col >= MAX_COLS {
            return Err(AddressError::ColumnOutOfRange);
        }
        Ok(Self { row, col })
    }

    pub fn row(self) -> u32 {
        self.row
    }

    pub fn col(self) -> u32 {
        self.col
    }

    pub fn offset(self, rows: i64, cols: i64) -> Self {
        let clamp = |value: u32, delta: i64, max: u32| {
            (i64::from(value) + delta).clamp(0, i64::from(max - 1)) as u32
        };
        Self {
            row: clamp(self.row, rows, MAX_ROWS),
            col: clamp(self.col, cols, MAX_COLS),
        }
    }
}

pub fn column_name(col: u32) -> String {
    let mut n = u64::from(col) + 1;
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push(char::from(b'A' + (n % 26) as u8));
        n /= 26;
    }
    letters.iter().rev().collect()
}

pub fn column_index(name: &str) -> Result<u32, AddressError> {
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(AddressError::Syntax(name.to_owned()));
    }
    let mut n: u32 = 0;
    for b in name.bytes() {
        n = n * 26 + u32::from(b.to_ascii_uppercase() - b'A') + 1;
        if n > MAX_COLS {
            return Err(AddressError::ColumnOutOfRange);
        }
    }
    Ok(n - 1)
}

impl FromStr for CellAddr {
    type Err = AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let syntax = || AddressError::Syntax(s.to_owned());
        let split = s.find(|c: char| c.is_ascii_digit()).ok_or_else(syntax)?;
        let (letters, digits) = s.split_at(split);
        if letters.is_empty()
            || !letters.bytes().all(|b| b.is_ascii_alphabetic())
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || (digits.len() > 1 && digits.starts_with('0'))
        {
            return Err(syntax());
        }
        let col = column_index(letters)?;
        let row: u32 = digits.parse().map_err(|_| AddressError::RowOutOfRange)?;
        if row == 0 {
            return Err(AddressError::RowOutOfRange);
        }
        Self::new(row - 1, col)
    }
}

impl fmt::Display for CellAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", column_name(self.col), self.row + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    #[test]
    fn column_names_at_boundaries() {
        let cases = [
            (0, "A"),
            (25, "Z"),
            (26, "AA"),
            (51, "AZ"),
            (52, "BA"),
            (701, "ZZ"),
            (702, "AAA"),
            (16_383, "XFD"),
        ];
        for (index, name) in cases {
            assert_eq!(column_name(index), name);
            assert_eq!(column_index(name), Ok(index));
        }
    }

    #[test]
    fn column_index_is_case_insensitive() {
        assert_eq!(column_index("xfd"), Ok(16_383));
        assert_eq!(column_index("aA"), Ok(26));
    }

    #[test]
    fn column_round_trip_for_every_column() {
        for col in 0..MAX_COLS {
            assert_eq!(column_index(&column_name(col)), Ok(col));
        }
    }

    #[test]
    fn column_index_rejects_out_of_range() {
        assert_eq!(column_index("XFE"), Err(AddressError::ColumnOutOfRange));
        assert_eq!(
            column_index("ZZZZZZZZZZ"),
            Err(AddressError::ColumnOutOfRange)
        );
    }

    #[test]
    fn column_index_rejects_non_letters() {
        for name in ["", "A1", "$A", "Ç", " A"] {
            assert!(
                matches!(column_index(name), Err(AddressError::Syntax(_))),
                "{name}"
            );
        }
    }

    #[test]
    fn parses_addresses() {
        assert_eq!(addr("A1"), CellAddr::new(0, 0).unwrap());
        assert_eq!(addr("Z1"), CellAddr::new(0, 25).unwrap());
        assert_eq!(addr("AA10"), CellAddr::new(9, 26).unwrap());
        assert_eq!(addr("az3"), CellAddr::new(2, 51).unwrap());
        assert_eq!(
            addr("XFD1048576"),
            CellAddr::new(MAX_ROWS - 1, MAX_COLS - 1).unwrap()
        );
    }

    #[test]
    fn display_round_trips() {
        for s in ["A1", "Z99", "AA1", "AZ7", "XFD1048576"] {
            assert_eq!(addr(s).to_string(), s);
        }
        assert_eq!(addr("b2").to_string(), "B2");
    }

    #[test]
    fn rejects_malformed_addresses() {
        for s in [
            "", "A", "1", "11", "1A", "A1B", "A-1", "A01", "$A$1", " A1", "A1 ", "Ä1", "A1:B2",
        ] {
            assert!(
                matches!(s.parse::<CellAddr>(), Err(AddressError::Syntax(_))),
                "{s}"
            );
        }
    }

    #[test]
    fn rejects_out_of_range_addresses() {
        assert_eq!("A0".parse::<CellAddr>(), Err(AddressError::RowOutOfRange));
        assert_eq!(
            "A1048577".parse::<CellAddr>(),
            Err(AddressError::RowOutOfRange)
        );
        assert_eq!(
            "A99999999999".parse::<CellAddr>(),
            Err(AddressError::RowOutOfRange)
        );
        assert_eq!(
            "XFE1".parse::<CellAddr>(),
            Err(AddressError::ColumnOutOfRange)
        );
    }

    #[test]
    fn new_validates_bounds() {
        assert!(CellAddr::new(MAX_ROWS - 1, MAX_COLS - 1).is_ok());
        assert_eq!(CellAddr::new(MAX_ROWS, 0), Err(AddressError::RowOutOfRange));
        assert_eq!(
            CellAddr::new(0, MAX_COLS),
            Err(AddressError::ColumnOutOfRange)
        );
    }

    #[test]
    fn offset_clamps_to_sheet_bounds() {
        assert_eq!(addr("B2").offset(1, 2), addr("D3"));
        assert_eq!(addr("B2").offset(-5, -5), addr("A1"));
        assert_eq!(addr("A1").offset(i64::MAX, i64::MAX), addr("XFD1048576"));
    }

    #[test]
    fn orders_row_major() {
        assert!(addr("Z1") < addr("A2"));
        assert!(addr("A1") < addr("B1"));
    }
}
