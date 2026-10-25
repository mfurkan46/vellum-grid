mod criteria;
mod eval;
mod functions;
mod lexer;
mod library;
mod parser;

use std::fmt;

use crate::locale::fold;
use crate::{CellAddr, CellError, CellRange, Locale, MAX_COLS, MAX_ROWS, column_name};

pub use eval::Evaluator;
pub use functions::names as function_names;
pub use lexer::{Highlight, highlights};
pub use parser::parse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableArea {
    Data,
    All,
    Headers,
    Totals,
    ThisRow,
    HeadersData,
    DataTotals,
}

impl TableArea {
    const ITEMS: [Self; 5] = [
        Self::All,
        Self::Data,
        Self::Headers,
        Self::Totals,
        Self::ThisRow,
    ];

    fn parts(self) -> &'static [Self] {
        match self {
            Self::HeadersData => &[Self::Headers, Self::Data],
            Self::DataTotals => &[Self::Data, Self::Totals],
            Self::Data => &[],
            Self::All => &[Self::All],
            Self::Headers => &[Self::Headers],
            Self::Totals => &[Self::Totals],
            Self::ThisRow => &[Self::ThisRow],
        }
    }

    fn name(self, locale: &Locale) -> &'static str {
        let index = Self::ITEMS
            .iter()
            .position(|item| *item == self)
            .unwrap_or(0);
        locale.table_items[index]
    }

    fn combine(self, other: Self) -> Option<Self> {
        match (self, other) {
            (a, b) if a == b => Some(a),
            (Self::Headers, Self::Data) | (Self::Data, Self::Headers) => Some(Self::HeadersData),
            (Self::Data, Self::Totals) | (Self::Totals, Self::Data) => Some(Self::DataTotals),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredRef {
    pub table: Option<String>,
    pub area: TableArea,
    pub columns: Option<(String, Option<String>)>,
}

pub fn escape_column(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if matches!(c, '[' | ']' | '#' | '\'') {
            out.push('\'');
        }
        out.push(c);
    }
    out
}

impl StructuredRef {
    fn write_to(&self, out: &mut String, locale: &Locale, short_this_row: bool) {
        if let Some(table) = &self.table {
            out.push_str(table);
        }
        let column = |name: &str| format!("[{}]", escape_column(name));
        let columns = self.columns.as_ref().map(|(first, last)| match last {
            Some(last) => format!("{}:{}", column(first), column(last)),
            None => column(first),
        });
        let simple = self
            .columns
            .as_ref()
            .is_some_and(|(_, last)| last.is_none());
        match (self.area, columns) {
            (TableArea::ThisRow, columns) if short_this_row || self.table.is_none() => {
                out.push_str("[@");
                match columns {
                    Some(text) if simple => {
                        let inner = &text[1..text.len() - 1];
                        if inner.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            out.push_str(inner);
                        } else {
                            out.push_str(&text);
                        }
                    }
                    Some(text) => out.push_str(&text),
                    None => {}
                }
                out.push(']');
            }
            (TableArea::Data, Some(text)) if simple => out.push_str(&text),
            (TableArea::Data, None) => {
                if self.table.is_none() {
                    out.push_str("[]");
                }
            }
            (area, columns) => {
                let mut items: Vec<String> = area
                    .parts()
                    .iter()
                    .map(|part| format!("[{}]", part.name(locale)))
                    .collect();
                items.extend(columns);
                if items.len() == 1 && self.columns.is_none() {
                    out.push_str(&items[0]);
                } else {
                    out.push('[');
                    out.push_str(&items.join(&locale.list.to_string()));
                    out.push(']');
                }
            }
        }
    }
}

pub(crate) fn parse_area(text: &str, locale: &Locale) -> Option<TableArea> {
    let key = fold(text.trim());
    TableArea::ITEMS
        .iter()
        .copied()
        .find(|item| fold(item.name(locale)) == key || fold(item.name(&crate::EN)) == key)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord {
    pub index: u32,
    pub absolute: bool,
}

impl Coord {
    fn shifted(self, delta: i64, max: u32) -> Option<Self> {
        if self.absolute {
            return Some(self);
        }
        let index = i64::from(self.index) + delta;
        (0..i64::from(max)).contains(&index).then_some(Self {
            index: index as u32,
            absolute: false,
        })
    }

    fn write_col(self, out: &mut String) {
        if self.absolute {
            out.push('$');
        }
        out.push_str(&column_name(self.index));
    }

    fn write_row(self, out: &mut String) {
        if self.absolute {
            out.push('$');
        }
        out.push_str(&(self.index + 1).to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reference {
    Cell {
        row: Coord,
        col: Coord,
    },
    Area {
        top: Coord,
        left: Coord,
        bottom: Coord,
        right: Coord,
    },
    Cols {
        left: Coord,
        right: Coord,
    },
    Rows {
        top: Coord,
        bottom: Coord,
    },
}

impl Reference {
    pub fn range(self) -> CellRange {
        let addr =
            |row: u32, col: u32| CellAddr::new(row, col).expect("references stay inside the sheet");
        match self {
            Self::Cell { row, col } => CellRange::single(addr(row.index, col.index)),
            Self::Area {
                top,
                left,
                bottom,
                right,
            } => CellRange::new(addr(top.index, left.index), addr(bottom.index, right.index)),
            Self::Cols { left, right } => {
                CellRange::new(addr(0, left.index), addr(MAX_ROWS - 1, right.index))
            }
            Self::Rows { top, bottom } => {
                CellRange::new(addr(top.index, 0), addr(bottom.index, MAX_COLS - 1))
            }
        }
    }

    fn shifted(self, rows: i64, cols: i64) -> Option<Self> {
        let row = |c: Coord| c.shifted(rows, MAX_ROWS);
        let col = |c: Coord| c.shifted(cols, MAX_COLS);
        Some(match self {
            Self::Cell { row: r, col: c } => Self::Cell {
                row: row(r)?,
                col: col(c)?,
            },
            Self::Area {
                top,
                left,
                bottom,
                right,
            } => Self::Area {
                top: row(top)?,
                left: col(left)?,
                bottom: row(bottom)?,
                right: col(right)?,
            },
            Self::Cols { left, right } => Self::Cols {
                left: col(left)?,
                right: col(right)?,
            },
            Self::Rows { top, bottom } => Self::Rows {
                top: row(top)?,
                bottom: row(bottom)?,
            },
        })
    }

    fn write(self, out: &mut String) {
        match self {
            Self::Cell { row, col } => {
                col.write_col(out);
                row.write_row(out);
            }
            Self::Area {
                top,
                left,
                bottom,
                right,
            } => {
                left.write_col(out);
                top.write_row(out);
                out.push(':');
                right.write_col(out);
                bottom.write_row(out);
            }
            Self::Cols { left, right } => {
                left.write_col(out);
                out.push(':');
                right.write_col(out);
            }
            Self::Rows { top, bottom } => {
                top.write_row(out);
                out.push(':');
                bottom.write_row(out);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Pow => "^",
            Self::Concat => "&",
            Self::Eq => "=",
            Self::Ne => "<>",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(CellError),
    Reference {
        sheet: Option<String>,
        reference: Reference,
    },
    Name(String),
    Structured(StructuredRef),
    Missing,
    Paren(Box<Expr>),
    Neg(Box<Expr>),
    Plus(Box<Expr>),
    Percent(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

impl Expr {
    pub fn write(&self, locale: &Locale) -> String {
        let mut out = String::new();
        self.write_to(&mut out, locale);
        out
    }

    fn write_to(&self, out: &mut String, locale: &Locale) {
        match self {
            Self::Number(n) => out.push_str(&locale.format_number(*n)),
            Self::Text(s) => {
                out.push('"');
                out.push_str(&s.replace('"', "\"\""));
                out.push('"');
            }
            Self::Bool(b) => out.push_str(locale.bool_name(*b)),
            Self::Error(e) => out.push_str(locale.error_name(*e)),
            Self::Reference { sheet, reference } => {
                if let Some(sheet) = sheet {
                    write_sheet(sheet, out);
                }
                reference.write(out);
            }
            Self::Name(name) => out.push_str(name),
            Self::Structured(reference) => {
                reference.write_to(out, locale, locale.list != crate::EN.list)
            }
            Self::Missing => {}
            Self::Paren(inner) => {
                out.push('(');
                inner.write_to(out, locale);
                out.push(')');
            }
            Self::Neg(inner) => {
                out.push('-');
                inner.write_to(out, locale);
            }
            Self::Plus(inner) => {
                out.push('+');
                inner.write_to(out, locale);
            }
            Self::Percent(inner) => {
                inner.write_to(out, locale);
                out.push('%');
            }
            Self::Binary(op, left, right) => {
                left.write_to(out, locale);
                out.push_str(op.symbol());
                right.write_to(out, locale);
            }
            Self::Call(name, args) => {
                out.push_str(locale.function_name(name));
                out.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        out.push(locale.list);
                    }
                    arg.write_to(out, locale);
                }
                out.push(')');
            }
        }
    }

    pub fn rename_sheet(&self, old: &str, new: Option<&str>) -> Self {
        let rename = |e: &Self| Box::new(e.rename_sheet(old, new));
        match self {
            Self::Reference {
                sheet: Some(name),
                reference,
            } if fold(name) == fold(old) => match new {
                Some(new) => Self::Reference {
                    sheet: Some(new.to_owned()),
                    reference: *reference,
                },
                None => Self::Error(CellError::Ref),
            },
            Self::Paren(e) => Self::Paren(rename(e)),
            Self::Neg(e) => Self::Neg(rename(e)),
            Self::Plus(e) => Self::Plus(rename(e)),
            Self::Percent(e) => Self::Percent(rename(e)),
            Self::Binary(op, a, b) => Self::Binary(*op, rename(a), rename(b)),
            Self::Call(name, args) => Self::Call(
                name.clone(),
                args.iter().map(|a| a.rename_sheet(old, new)).collect(),
            ),
            other => other.clone(),
        }
    }

    pub fn shifted(&self, rows: i64, cols: i64) -> Self {
        let shift = |e: &Self| Box::new(e.shifted(rows, cols));
        match self {
            Self::Reference { sheet, reference } => match reference.shifted(rows, cols) {
                Some(reference) => Self::Reference {
                    sheet: sheet.clone(),
                    reference,
                },
                None => Self::Error(CellError::Ref),
            },
            Self::Paren(e) => Self::Paren(shift(e)),
            Self::Neg(e) => Self::Neg(shift(e)),
            Self::Plus(e) => Self::Plus(shift(e)),
            Self::Percent(e) => Self::Percent(shift(e)),
            Self::Binary(op, a, b) => Self::Binary(*op, shift(a), shift(b)),
            Self::Call(name, args) => Self::Call(
                name.clone(),
                args.iter().map(|a| a.shifted(rows, cols)).collect(),
            ),
            other => other.clone(),
        }
    }

    pub fn map_references(
        &self,
        f: &impl Fn(Option<&str>, Reference) -> Option<Reference>,
    ) -> Self {
        let map = |e: &Self| Box::new(e.map_references(f));
        match self {
            Self::Reference { sheet, reference } => match f(sheet.as_deref(), *reference) {
                Some(reference) => Self::Reference {
                    sheet: sheet.clone(),
                    reference,
                },
                None => Self::Error(CellError::Ref),
            },
            Self::Paren(e) => Self::Paren(map(e)),
            Self::Neg(e) => Self::Neg(map(e)),
            Self::Plus(e) => Self::Plus(map(e)),
            Self::Percent(e) => Self::Percent(map(e)),
            Self::Binary(op, a, b) => Self::Binary(*op, map(a), map(b)),
            Self::Call(name, args) => Self::Call(
                name.clone(),
                args.iter().map(|a| a.map_references(f)).collect(),
            ),
            other => other.clone(),
        }
    }

    pub fn map_names(&self, f: &impl Fn(&str) -> Option<Expr>) -> Self {
        let map = |e: &Self| Box::new(e.map_names(f));
        match self {
            Self::Name(name) => f(name).unwrap_or_else(|| self.clone()),
            Self::Paren(e) => Self::Paren(map(e)),
            Self::Neg(e) => Self::Neg(map(e)),
            Self::Plus(e) => Self::Plus(map(e)),
            Self::Percent(e) => Self::Percent(map(e)),
            Self::Binary(op, a, b) => Self::Binary(*op, map(a), map(b)),
            Self::Call(name, args) => {
                Self::Call(name.clone(), args.iter().map(|a| a.map_names(f)).collect())
            }
            other => other.clone(),
        }
    }

    pub fn map_structured(&self, f: &impl Fn(&StructuredRef) -> Option<Expr>) -> Self {
        let map = |e: &Self| Box::new(e.map_structured(f));
        match self {
            Self::Structured(reference) => f(reference).unwrap_or_else(|| self.clone()),
            Self::Paren(e) => Self::Paren(map(e)),
            Self::Neg(e) => Self::Neg(map(e)),
            Self::Plus(e) => Self::Plus(map(e)),
            Self::Percent(e) => Self::Percent(map(e)),
            Self::Binary(op, a, b) => Self::Binary(*op, map(a), map(b)),
            Self::Call(name, args) => Self::Call(
                name.clone(),
                args.iter().map(|a| a.map_structured(f)).collect(),
            ),
            other => other.clone(),
        }
    }

    pub fn structured(&self) -> Vec<&StructuredRef> {
        let mut out = Vec::new();
        self.visit(&mut |e| {
            if let Self::Structured(reference) = e {
                out.push(reference);
            }
        });
        out
    }

    pub fn qualify(&self, sheet: &str) -> Self {
        let qualify = |e: &Self| Box::new(e.qualify(sheet));
        match self {
            Self::Reference {
                sheet: None,
                reference,
            } => Self::Reference {
                sheet: Some(sheet.to_owned()),
                reference: *reference,
            },
            Self::Paren(e) => Self::Paren(qualify(e)),
            Self::Neg(e) => Self::Neg(qualify(e)),
            Self::Plus(e) => Self::Plus(qualify(e)),
            Self::Percent(e) => Self::Percent(qualify(e)),
            Self::Binary(op, a, b) => Self::Binary(*op, qualify(a), qualify(b)),
            Self::Call(name, args) => Self::Call(
                name.clone(),
                args.iter().map(|a| a.qualify(sheet)).collect(),
            ),
            other => other.clone(),
        }
    }

    pub fn names(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.visit(&mut |e| {
            if let Self::Name(name) = e {
                out.push(name.as_str());
            }
        });
        out
    }

    pub fn references(&self) -> Vec<(Option<&str>, CellRange)> {
        let mut out = Vec::new();
        self.visit(&mut |e| {
            if let Self::Reference { sheet, reference } = e {
                out.push((sheet.as_deref(), reference.range()));
            }
        });
        out
    }

    pub fn is_supported(&self) -> bool {
        let mut supported = true;
        self.visit(&mut |e| {
            if let Self::Call(name, _) = e {
                supported &= functions::arity(name).is_some();
            }
        });
        supported
    }

    pub fn is_volatile(&self) -> bool {
        library::VOLATILE.iter().any(|name| self.calls(name))
    }

    pub fn calls(&self, function: &str) -> bool {
        let mut found = false;
        self.visit(&mut |e| found |= matches!(e, Self::Call(name, _) if name == function));
        found
    }

    fn visit<'a>(&'a self, f: &mut impl FnMut(&'a Self)) {
        f(self);
        match self {
            Self::Paren(e) | Self::Neg(e) | Self::Plus(e) | Self::Percent(e) => e.visit(f),
            Self::Binary(_, a, b) => {
                a.visit(f);
                b.visit(f);
            }
            Self::Call(_, args) => args.iter().for_each(|a| a.visit(f)),
            _ => {}
        }
    }
}

fn write_sheet(sheet: &str, out: &mut String) {
    let plain = sheet
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && sheet.chars().all(|c| c.is_alphanumeric() || c == '_')
        && sheet.parse::<CellAddr>().is_err();
    if plain {
        out.push_str(sheet);
    } else {
        out.push('\'');
        out.push_str(&sheet.replace('\'', "''"));
        out.push('\'');
    }
    out.push('!');
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormulaError {
    Syntax,
    UnterminatedText,
    TooFewArguments(String),
    TooManyArguments(String),
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax => write!(f, "invalid formula"),
            Self::UnterminatedText => write!(f, "unterminated text in formula"),
            Self::TooFewArguments(name) => write!(f, "too few arguments for {name}"),
            Self::TooManyArguments(name) => write!(f, "too many arguments for {name}"),
        }
    }
}

impl std::error::Error for FormulaError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EN, TR};

    fn round_trip(src: &str, locale: &Locale) -> String {
        parse(src, locale).unwrap().write(locale)
    }

    #[test]
    fn writes_back_what_was_parsed() {
        for src in [
            "SUM(A1:B2,$C$3,D:D,2:5)",
            "-A1^2+B1%*(C1-1)",
            "IF(A1>=10,\"büyük \"\"x\"\"\",FALSE)",
            "A1&\" \"&TEXT",
            "#N/A",
            "Sheet2!A1+'My Sheet'!$B$2",
            "VLOOKUP(A1,B:C,2,)",
            "1.5E+20*0.25",
        ] {
            assert_eq!(round_trip(src, &EN), src);
        }
    }

    #[test]
    fn translates_between_turkish_and_canonical() {
        let expr = parse("EĞER(TOPLA(A1:A3)>1,5;\"evet\";YANLIŞ)", &TR).unwrap();
        assert_eq!(expr.write(&EN), "IF(SUM(A1:A3)>1.5,\"evet\",FALSE)");
        let back = parse(&expr.write(&EN), &EN).unwrap();
        assert_eq!(back.write(&TR), "EĞER(TOPLA(A1:A3)>1,5;\"evet\";YANLIŞ)");
        assert_eq!(
            parse("#SAYI/0!+#YOK", &TR).unwrap().write(&EN),
            "#DIV/0!+#N/A"
        );
    }

    #[test]
    fn shifts_relative_references_only() {
        let expr = parse("A1+$B$2+C$3+$D4+SUM(E5:F6)", &EN).unwrap();
        assert_eq!(expr.shifted(2, 1).write(&EN), "B3+$B$2+D$3+$D6+SUM(F7:G8)");
        assert_eq!(parse("A1", &EN).unwrap().shifted(-1, 0).write(&EN), "#REF!");
        assert_eq!(parse("B:B", &EN).unwrap().shifted(5, 1).write(&EN), "C:C");
    }

    #[test]
    fn renames_and_removes_sheet_references() {
        let expr = parse("Veri!A1+'veri'!B2+Diğer!C3+D4", &EN).unwrap();
        assert_eq!(
            expr.rename_sheet("VERI", Some("Yeni Ad")).write(&EN),
            "'Yeni Ad'!A1+'Yeni Ad'!B2+Diğer!C3+D4"
        );
        assert_eq!(
            expr.rename_sheet("Veri", None).write(&EN),
            "#REF!+#REF!+Diğer!C3+D4"
        );
    }

    #[test]
    fn structured_references_round_trip() {
        let en = |src: &str| round_trip(src, &EN);
        assert_eq!(en("SUM(Table1[Amount])"), "SUM(Table1[Amount])");
        assert_eq!(
            en("Table1[[#Headers],[Amount]]"),
            "Table1[[#Headers],[Amount]]"
        );
        assert_eq!(
            en("Table1[[#This Row],[Price]]*2"),
            "Table1[[#This Row],[Price]]*2"
        );
        assert_eq!(en("[@Price]*2"), "[@Price]*2");
        assert_eq!(en("Table1[#All]"), "Table1[#All]");
        assert_eq!(en("Table1"), "Table1");
        assert_eq!(en("Table1[[Q1]:[Q4]]"), "Table1[[Q1]:[Q4]]");
        assert_eq!(en("Table1[Unit ''Price]"), "Table1[Unit ''Price]");
        let parsed = parse("Tablo1[[#Bu Satır];[Fiyat]]*[@[Birim Fiyat]]", &TR).unwrap();
        assert_eq!(
            parsed.write(&EN),
            "Tablo1[[#This Row],[Fiyat]]*[@[Birim Fiyat]]"
        );
        assert_eq!(parsed.write(&TR), "Tablo1[@Fiyat]*[@[Birim Fiyat]]");
        let totals = parse("SUM(Tablo1[[#Veri];[#Toplamlar];[Tutar]])", &TR).unwrap();
        assert_eq!(totals.write(&EN), "SUM(Tablo1[[#Data],[#Totals],[Tutar]])");
        assert_eq!(totals.structured().len(), 1);
    }

    #[test]
    fn lists_references_and_calls() {
        let expr = parse("SUM(A1:B2)+Other!C3+TODAY()", &EN).unwrap();
        let refs = expr.references();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0], (None, "A1:B2".parse().unwrap()));
        assert_eq!(refs[1], (Some("Other"), "C3".parse().unwrap()));
        assert!(expr.calls("TODAY"));
        assert!(!expr.calls("NOW"));
    }
}
