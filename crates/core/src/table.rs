use crate::formula::{Coord, Expr, Reference, StructuredRef, TableArea, escape_column};
use crate::locale::fold;
use crate::{CellAddr, CellError, CellRange, Workbook};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TotalFunction {
    None,
    Average,
    Count,
    CountNums,
    Max,
    Min,
    Sum,
    StdDev,
    Var,
    Custom,
}

impl TotalFunction {
    pub const MENU: [Self; 9] = [
        Self::None,
        Self::Average,
        Self::Count,
        Self::CountNums,
        Self::Max,
        Self::Min,
        Self::Sum,
        Self::StdDev,
        Self::Var,
    ];

    fn code(self) -> Option<u8> {
        Some(match self {
            Self::Average => 101,
            Self::Count => 103,
            Self::CountNums => 102,
            Self::Max => 104,
            Self::Min => 105,
            Self::Sum => 109,
            Self::StdDev => 107,
            Self::Var => 110,
            Self::None | Self::Custom => return None,
        })
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Average => "average",
            Self::Count => "count",
            Self::CountNums => "countNums",
            Self::Max => "max",
            Self::Min => "min",
            Self::Sum => "sum",
            Self::StdDev => "stdDev",
            Self::Var => "var",
            Self::Custom => "custom",
        }
    }

    pub fn from_key(key: &str) -> Self {
        Self::MENU
            .into_iter()
            .chain([Self::Custom])
            .find(|f| f.key() == key)
            .unwrap_or(Self::None)
    }

    pub fn formula(self, table: &str, column: &str) -> Option<String> {
        let code = self.code()?;
        Some(format!(
            "=SUBTOTAL({code},{table}[{}])",
            escape_column(column)
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    pub name: String,
    pub total: TotalFunction,
    pub label: Option<String>,
}

impl TableColumn {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            total: TotalFunction::None,
            label: None,
        }
    }
}

pub const DEFAULT_TABLE_STYLE: &str = "TableStyleMedium2";

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: String,
    pub range: CellRange,
    pub style: String,
    pub header: bool,
    pub totals: bool,
    pub banded_rows: bool,
    pub banded_cols: bool,
    pub first_col: bool,
    pub last_col: bool,
    pub filter: bool,
    pub columns: Vec<TableColumn>,
}

impl Table {
    pub fn new(name: impl Into<String>, range: CellRange, columns: Vec<TableColumn>) -> Self {
        Self {
            name: name.into(),
            range,
            style: DEFAULT_TABLE_STYLE.to_owned(),
            header: true,
            totals: false,
            banded_rows: true,
            banded_cols: false,
            first_col: false,
            last_col: false,
            filter: true,
            columns,
        }
    }

    pub fn header_row(&self) -> Option<u32> {
        self.header.then_some(self.range.start().row())
    }

    pub fn totals_row(&self) -> Option<u32> {
        self.totals.then_some(self.range.end().row())
    }

    pub fn data_rows(&self) -> (u32, u32) {
        let top = self.range.start().row() + u32::from(self.header);
        let bottom = self
            .range
            .end()
            .row()
            .saturating_sub(u32::from(self.totals));
        (top, bottom.max(top))
    }

    pub fn filter_range(&self) -> CellRange {
        let (_, bottom) = self.data_rows();
        let start = self.range.start();
        let end = CellAddr::new(bottom, self.range.end().col()).unwrap_or(self.range.end());
        CellRange::new(start, end)
    }

    pub fn column_at(&self, col: u32) -> Option<&TableColumn> {
        let left = self.range.start().col();
        col.checked_sub(left)
            .and_then(|i| self.columns.get(i as usize))
    }

    pub fn column_index(&self, name: &str) -> Option<u32> {
        let key = fold(name);
        self.columns
            .iter()
            .position(|c| fold(&c.name) == key)
            .map(|i| self.range.start().col() + i as u32)
    }

    pub fn resolve(&self, reference: &StructuredRef, at: CellAddr) -> Result<CellRange, CellError> {
        let (data_top, data_bottom) = self.data_rows();
        let (top, bottom) = match reference.area {
            TableArea::Data => (data_top, data_bottom),
            TableArea::All => (self.range.start().row(), self.range.end().row()),
            TableArea::Headers => {
                let row = self.header_row().ok_or(CellError::Ref)?;
                (row, row)
            }
            TableArea::Totals => {
                let row = self.totals_row().ok_or(CellError::Ref)?;
                (row, row)
            }
            TableArea::ThisRow => {
                if at.row() < data_top || at.row() > data_bottom {
                    return Err(CellError::Value);
                }
                (at.row(), at.row())
            }
            TableArea::HeadersData => (self.range.start().row(), data_bottom),
            TableArea::DataTotals => (data_top, self.range.end().row()),
        };
        let (left, right) = match &reference.columns {
            None => (self.range.start().col(), self.range.end().col()),
            Some((first, last)) => {
                let a = self.column_index(first).ok_or(CellError::Ref)?;
                let b = match last {
                    Some(last) => self.column_index(last).ok_or(CellError::Ref)?,
                    None => a,
                };
                (a.min(b), a.max(b))
            }
        };
        let corner = |row, col| CellAddr::new(row, col).map_err(|_| CellError::Ref);
        Ok(CellRange::new(corner(top, left)?, corner(bottom, right)?))
    }
}

pub fn unique_columns(raw: impl IntoIterator<Item = String>, prefix: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (index, name) in raw.into_iter().enumerate() {
        let base = if name.trim().is_empty() {
            format!("{prefix}{}", index + 1)
        } else {
            name
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while out
            .iter()
            .any(|existing| fold(existing) == fold(&candidate))
        {
            candidate = format!("{base}{n}");
            n += 1;
        }
        out.push(candidate);
    }
    out
}

pub fn rename_column(expr: &Expr, table: &str, inside: bool, old: &str, new: &str) -> Expr {
    expr.map_structured(&|r| {
        let ours = match &r.table {
            Some(name) => fold(name) == fold(table),
            None => inside,
        };
        let (first, last) = r.columns.as_ref().filter(|_| ours)?;
        let swap = |c: &String| {
            if fold(c) == fold(old) {
                new.to_owned()
            } else {
                c.clone()
            }
        };
        let columns = Some((swap(first), last.as_ref().map(swap)));
        (columns != r.columns).then(|| {
            Expr::Structured(StructuredRef {
                columns,
                ..r.clone()
            })
        })
    })
}

fn range_expr(sheet: Option<&str>, range: CellRange) -> Expr {
    let coord = |index| Coord {
        index,
        absolute: true,
    };
    let reference = if range.is_single() {
        Reference::Cell {
            row: coord(range.start().row()),
            col: coord(range.start().col()),
        }
    } else {
        Reference::Area {
            top: coord(range.start().row()),
            left: coord(range.start().col()),
            bottom: coord(range.end().row()),
            right: coord(range.end().col()),
        }
    };
    Expr::Reference {
        sheet: sheet.map(str::to_owned),
        reference,
    }
}

impl Workbook {
    pub fn tables(&self) -> impl Iterator<Item = (usize, &Table)> {
        self.sheets()
            .iter()
            .enumerate()
            .flat_map(|(index, sheet)| sheet.layout().tables.iter().map(move |t| (index, t)))
    }

    pub fn find_table(&self, name: &str) -> Option<(usize, &Table)> {
        let key = fold(name);
        self.tables().find(|(_, table)| fold(&table.name) == key)
    }

    pub fn table_at(&self, sheet: usize, addr: CellAddr) -> Option<&Table> {
        self.sheets()
            .get(sheet)?
            .layout()
            .tables
            .iter()
            .find(|table| table.range.contains(addr))
    }

    pub fn next_table_name(&self, prefix: &str) -> String {
        (1..)
            .map(|n| format!("{prefix}{n}"))
            .find(|name| self.find_table(name).is_none() && self.find_name(name, 0).is_none())
            .expect("an unused table name")
    }

    pub fn expand_tables(&self, expr: &Expr, sheet: usize, at: CellAddr) -> Expr {
        self.expand_table_refs(expr, sheet, at, None)
    }

    pub fn expand_table_refs(
        &self,
        expr: &Expr,
        sheet: usize,
        at: CellAddr,
        only: Option<&str>,
    ) -> Expr {
        let wanted = |name: &str| only.is_none_or(|only| fold(only) == fold(name));
        let target = |index: usize| (index != sheet).then(|| self.sheets()[index].name());
        let expr = expr.map_names(&|name| {
            let (index, table) = self.find_table(name).filter(|(_, t)| wanted(&t.name))?;
            let reference = StructuredRef {
                table: Some(table.name.clone()),
                area: TableArea::Data,
                columns: None,
            };
            Some(match table.resolve(&reference, at) {
                Ok(range) => range_expr(target(index), range),
                Err(error) => Expr::Error(error),
            })
        });
        expr.map_structured(&|reference| {
            let found = match &reference.table {
                Some(name) => self.find_table(name),
                None => self.table_at(sheet, at).map(|table| (sheet, table)),
            };
            let Some((index, table)) = found else {
                return only.is_none().then_some(Expr::Error(CellError::Ref));
            };
            if !wanted(&table.name) {
                return None;
            }
            Some(match table.resolve(reference, at) {
                Ok(range) => range_expr(target(index), range),
                Err(error) => Expr::Error(error),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EN, Sheet, TR, formula};

    fn table() -> Table {
        let mut table = Table::new(
            "Satışlar",
            "B2:D6".parse().unwrap(),
            vec![
                TableColumn::new("Ürün"),
                TableColumn::new("Adet"),
                TableColumn::new("Birim Fiyat"),
            ],
        );
        table.totals = true;
        table
    }

    #[test]
    fn resolves_areas_and_columns() {
        let table = table();
        let at: CellAddr = "E4".parse().unwrap();
        let resolve = |src: &str| {
            let expr = formula::parse(src, &TR).unwrap();
            let reference = expr.structured()[0].clone();
            table.resolve(&reference, at).map(|r| r.to_string())
        };
        assert_eq!(resolve("Satışlar[Adet]"), Ok("C3:C5".into()));
        assert_eq!(resolve("Satışlar[#Tümü]"), Ok("B2:D6".into()));
        assert_eq!(resolve("Satışlar[[#Başlıklar];[Ürün]]"), Ok("B2".into()));
        assert_eq!(resolve("Satışlar[[#Toplamlar];[Adet]]"), Ok("C6".into()));
        assert_eq!(resolve("Satışlar[@[Birim Fiyat]]"), Ok("D4".into()));
        assert_eq!(
            resolve("Satışlar[[Adet]:[Birim Fiyat]]"),
            Ok("C3:D5".into())
        );
        assert_eq!(resolve("Satışlar[Yok]"), Err(CellError::Ref));
        assert_eq!(
            table.resolve(
                &formula::parse("[@Adet]", &TR).unwrap().structured()[0].clone(),
                "E9".parse().unwrap()
            ),
            Err(CellError::Value)
        );
    }

    #[test]
    fn expands_into_sheet_references() {
        let mut sheet = Sheet::new("Veri");
        sheet.layout_mut().tables.push(table());
        let book = Workbook::from_sheets(vec![sheet, Sheet::new("Özet")]).unwrap();
        let expand = |src: &str, sheet: usize, at: &str| {
            let expr = formula::parse(src, &EN).unwrap();
            book.expand_tables(&expr, sheet, at.parse().unwrap())
                .write(&EN)
        };
        assert_eq!(
            expand("SUM(Satışlar[Adet])", 1, "A1"),
            "SUM(Veri!$C$3:$C$5)"
        );
        assert_eq!(expand("[@Adet]*[@[Birim Fiyat]]", 0, "D3"), "$C$3*$D$3");
        assert_eq!(
            expand("COUNTA(Satışlar)", 1, "A1"),
            "COUNTA(Veri!$B$3:$D$5)"
        );
        assert_eq!(expand("[@Adet]", 1, "A1"), "#REF!");
    }

    #[test]
    fn names_columns_uniquely_and_builds_totals() {
        let names = unique_columns(["Ad".to_owned(), String::new(), "ad".to_owned()], "Sütun");
        assert_eq!(names, ["Ad", "Sütun2", "ad2"]);
        assert_eq!(
            TotalFunction::Sum.formula("Tablo1", "Birim [TL]"),
            Some("=SUBTOTAL(109,Tablo1[Birim '[TL']])".into())
        );
        assert_eq!(
            TotalFunction::from_key("countNums"),
            TotalFunction::CountNums
        );
        assert_eq!(TotalFunction::None.formula("T", "A"), None);
    }
}
