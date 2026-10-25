use std::collections::HashSet;

use crate::filter::{ColumnFilter, Condition, Context, Entry, Operator, matches};
use crate::locale::fold;
use crate::{Cell, CellAddr, CellRange, CellValue, Locale, Sheet, Style, date_serial, numfmt};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Delimiters {
    pub tab: bool,
    pub semicolon: bool,
    pub comma: bool,
    pub space: bool,
    pub other: Option<char>,
}

impl Delimiters {
    fn contains(&self, c: char) -> bool {
        (self.tab && c == '\t')
            || (self.semicolon && c == ';')
            || (self.comma && c == ',')
            || (self.space && c == ' ')
            || self.other == Some(c)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Split {
    Delimited {
        delimiters: Delimiters,
        merge: bool,
        qualifier: Option<char>,
    },
    Fixed(Vec<usize>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    Dmy,
    Mdy,
    Ymd,
    Ydm,
    Myd,
    Dym,
}

impl DateOrder {
    pub const ALL: [Self; 6] = [
        Self::Dmy,
        Self::Mdy,
        Self::Ymd,
        Self::Ydm,
        Self::Myd,
        Self::Dym,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Dmy => "GAY",
            Self::Mdy => "AGY",
            Self::Ymd => "YAG",
            Self::Ydm => "YGA",
            Self::Myd => "AYG",
            Self::Dym => "GYA",
        }
    }

    fn arrange(self, [a, b, c]: [i64; 3]) -> (i64, i64, i64) {
        match self {
            Self::Dmy => (c, b, a),
            Self::Mdy => (c, a, b),
            Self::Ymd => (a, b, c),
            Self::Ydm => (a, c, b),
            Self::Myd => (b, a, c),
            Self::Dym => (b, c, a),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    General,
    Text,
    Date(DateOrder),
    Skip,
}

pub fn split_line(line: &str, split: &Split) -> Vec<String> {
    match split {
        Split::Fixed(breaks) => {
            let chars: Vec<char> = line.chars().collect();
            let mut bounds: Vec<usize> = breaks
                .iter()
                .copied()
                .filter(|b| *b > 0 && *b < chars.len())
                .collect();
            bounds.sort_unstable();
            bounds.dedup();
            let mut out = Vec::new();
            let mut start = 0;
            for end in bounds.into_iter().chain([chars.len()]) {
                out.push(
                    chars[start..end]
                        .iter()
                        .collect::<String>()
                        .trim()
                        .to_owned(),
                );
                start = end;
            }
            out
        }
        Split::Delimited {
            delimiters,
            merge,
            qualifier,
        } => {
            let mut out = Vec::new();
            let mut field = String::new();
            let mut quoted = false;
            let mut chars = line.chars().peekable();
            let mut last_was_delimiter = false;
            while let Some(c) = chars.next() {
                if Some(c) == *qualifier {
                    if quoted && chars.peek() == Some(&c) {
                        field.push(c);
                        chars.next();
                    } else {
                        quoted = !quoted;
                    }
                    last_was_delimiter = false;
                } else if !quoted && delimiters.contains(c) {
                    if !(*merge && last_was_delimiter) {
                        out.push(std::mem::take(&mut field));
                    }
                    last_was_delimiter = true;
                } else {
                    field.push(c);
                    last_was_delimiter = false;
                }
            }
            out.push(field);
            out
        }
    }
}

fn parse_date(text: &str, order: DateOrder) -> Option<f64> {
    let parts: Vec<i64> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    let parts: [i64; 3] = parts.try_into().ok()?;
    let (year, month, day) = order.arrange(parts);
    let year = match year {
        0..=29 => 2000 + year,
        30..=99 => 1900 + year,
        other => other,
    };
    date_serial(year, u32::try_from(month).ok()?, u32::try_from(day).ok()?)
}

pub fn convert(text: &str, kind: ColumnKind, locale: &Locale) -> Option<Cell> {
    let text_cell = |format: Option<&str>| {
        let mut cell: Cell = CellValue::Text(text.to_owned()).into();
        if let Some(code) = format {
            cell.set_style(Style {
                number_format: code.to_owned(),
                ..Style::default()
            });
        }
        cell
    };
    match kind {
        ColumnKind::Skip => None,
        _ if text.is_empty() => Some(Cell::default()),
        ColumnKind::Text => Some(text_cell(Some(numfmt::TEXT))),
        ColumnKind::Date(order) => Some(match parse_date(text, order) {
            Some(serial) => {
                let mut cell: Cell = CellValue::Number(serial).into();
                cell.set_style(Style {
                    number_format: numfmt::DATE.to_owned(),
                    ..Style::default()
                });
                cell
            }
            None => text_cell(None),
        }),
        ColumnKind::General if text.starts_with('=') => Some(text_cell(None)),
        ColumnKind::General => {
            Some(Cell::parse_in(text.trim(), locale).unwrap_or_else(|_| text_cell(None)))
        }
    }
}

pub fn source_text(cell: Option<&Cell>, locale: &Locale) -> String {
    match cell.map(|cell| &cell.value) {
        Some(CellValue::Text(text)) => text.clone(),
        Some(CellValue::Empty) | None => String::new(),
        Some(value) => locale.display(value),
    }
}

pub fn split_rows<S: AsRef<str>>(
    lines: impl IntoIterator<Item = S>,
    split: &Split,
    kinds: &[ColumnKind],
    dest: CellAddr,
    locale: &Locale,
) -> Vec<(CellAddr, Cell)> {
    let mut out = Vec::new();
    for (offset, line) in lines.into_iter().enumerate() {
        let text = line.as_ref();
        if text.is_empty() {
            continue;
        }
        let target_row = dest.row() + offset as u32;
        let mut target_col = dest.col();
        for (index, part) in split_line(text, split).iter().enumerate() {
            let kind = kinds.get(index).copied().unwrap_or(ColumnKind::General);
            let Some(cell) = convert(part, kind, locale) else {
                continue;
            };
            if let Ok(addr) = CellAddr::new(target_row, target_col) {
                out.push((addr, cell));
            }
            target_col += 1;
        }
    }
    out
}

pub fn text_to_columns(
    sheet: &Sheet,
    source: CellRange,
    split: &Split,
    kinds: &[ColumnKind],
    dest: CellAddr,
    locale: &Locale,
) -> Vec<(CellAddr, Cell)> {
    let col = source.start().col();
    let lines = (source.start().row()..=source.end().row()).map(|row| {
        CellAddr::new(row, col)
            .map(|at| source_text(sheet.get(at), locale))
            .unwrap_or_default()
    });
    split_rows(lines, split, kinds, dest, locale)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    Same,
    Upper,
    Lower,
    Proper,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Literal(String),
    Word {
        source: usize,
        index: usize,
        from_end: bool,
        case: Case,
        initial: bool,
    },
    Whole {
        source: usize,
        case: Case,
    },
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn apply_case(text: &str, case: Case, locale: &Locale) -> String {
    match case {
        Case::Same => text.to_owned(),
        Case::Upper => locale.upper(text),
        Case::Lower => locale.lower(text),
        Case::Proper => {
            let mut chars = text.chars();
            match chars.next() {
                Some(first) => locale.upper(&first.to_string()) + &locale.lower(chars.as_str()),
                None => String::new(),
            }
        }
    }
}

const CASES: [Case; 4] = [Case::Same, Case::Proper, Case::Upper, Case::Lower];

fn run(program: &[Part], sources: &[String], locale: &Locale) -> Option<String> {
    let mut out = String::new();
    for part in program {
        match part {
            Part::Literal(text) => out.push_str(text),
            Part::Whole { source, case } => {
                out.push_str(&apply_case(sources.get(*source)?, *case, locale))
            }
            Part::Word {
                source,
                index,
                from_end,
                case,
                initial,
            } => {
                let list = words(sources.get(*source)?);
                let at = if *from_end {
                    list.len().checked_sub(index + 1)?
                } else {
                    *index
                };
                let word = list.get(at)?;
                let word = if *initial {
                    word.chars().next()?.to_string()
                } else {
                    word.clone()
                };
                out.push_str(&apply_case(&word, *case, locale));
            }
        }
    }
    Some(out)
}

fn learn(sources: &[String], example: &str, from_end: bool, locale: &Locale) -> Vec<Part> {
    let mut program: Vec<Part> = Vec::new();
    let chars: Vec<char> = example.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        let rest: String = chars[at..].iter().collect();
        let mut best: Option<(usize, Part)> = None;
        let mut consider = |len: usize, part: Part| {
            if len > 0 && best.as_ref().is_none_or(|(known, _)| len > *known) {
                best = Some((len, part));
            }
        };
        for (source, text) in sources.iter().enumerate() {
            for case in CASES {
                let whole = apply_case(text, case, locale);
                if !whole.is_empty() && rest.starts_with(&whole) {
                    consider(whole.chars().count(), Part::Whole { source, case });
                }
            }
            let list = words(text);
            for (position, word) in list.iter().enumerate() {
                let index = if from_end {
                    list.len() - 1 - position
                } else {
                    position
                };
                for case in CASES {
                    let full = apply_case(word, case, locale);
                    if rest.starts_with(&full) {
                        let part = Part::Word {
                            source,
                            index,
                            from_end,
                            case,
                            initial: false,
                        };
                        consider(full.chars().count(), part);
                    }
                    let first = apply_case(
                        &word.chars().next().unwrap_or(' ').to_string(),
                        case,
                        locale,
                    );
                    if rest.starts_with(&first) && word.chars().count() > 1 {
                        let part = Part::Word {
                            source,
                            index,
                            from_end,
                            case,
                            initial: true,
                        };
                        consider(1, part);
                    }
                }
            }
        }
        match best {
            Some((len, part))
                if len > 1 || !chars[at].is_ascii_punctuation() && !chars[at].is_whitespace() =>
            {
                program.push(part);
                at += len;
            }
            _ => {
                match program.last_mut() {
                    Some(Part::Literal(text)) => text.push(chars[at]),
                    _ => program.push(Part::Literal(chars[at].to_string())),
                }
                at += 1;
            }
        }
    }
    program
}

pub fn flash_fill(
    rows: &[(Vec<String>, Option<String>)],
    locale: &Locale,
) -> Option<Vec<Option<String>>> {
    let examples: Vec<&(Vec<String>, Option<String>)> = rows
        .iter()
        .filter(|(_, example)| example.is_some())
        .collect();
    let (sources, example) = examples.first()?;
    let example = example.as_deref()?;
    for from_end in [false, true] {
        let program = learn(sources, example, from_end, locale);
        let fits = examples.iter().all(|(sources, example)| {
            run(&program, sources, locale).as_deref() == example.as_deref()
        });
        if fits {
            return Some(
                rows.iter()
                    .map(|(sources, example)| match example {
                        Some(_) => None,
                        None => run(&program, sources, locale).filter(|text| !text.is_empty()),
                    })
                    .collect(),
            );
        }
    }
    None
}

fn key(value: &CellValue) -> String {
    match value {
        CellValue::Empty => String::new(),
        CellValue::Number(n) => format!("n{n}"),
        CellValue::Text(text) => format!("t{}", fold(text)),
        CellValue::Bool(b) => format!("b{b}"),
        CellValue::Error(e) => format!("e{}", e.code()),
    }
}

pub fn unique_rows(sheet: &Sheet, range: CellRange, columns: &[u32], header: bool) -> Vec<u32> {
    let first = range.start().row() + u32::from(header);
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for row in first..=range.end().row() {
        let row_key: Vec<String> = columns
            .iter()
            .filter_map(|col| CellAddr::new(row, *col).ok())
            .map(|addr| key(sheet.value(addr)))
            .collect();
        if seen.insert(row_key) {
            kept.push(row);
        }
    }
    kept
}

fn criterion(cell: &Cell, locale: &Locale) -> Option<Condition> {
    let value = match &cell.value {
        CellValue::Empty => return None,
        CellValue::Text(text) => text.trim().to_owned(),
        CellValue::Number(n) => {
            return Some(Condition {
                op: Operator::Equal,
                value: crate::formula::Expr::Number(*n).write(&crate::EN),
            });
        }
        other => locale.display(other),
    };
    if value.is_empty() {
        return None;
    }
    let (op, rest) = [
        ("<>", Operator::NotEqual),
        (">=", Operator::GreaterOrEqual),
        ("<=", Operator::LessOrEqual),
        (">", Operator::Greater),
        ("<", Operator::Less),
        ("=", Operator::Equal),
    ]
    .into_iter()
    .find_map(|(prefix, op)| value.strip_prefix(prefix).map(|rest| (op, rest.trim())))
    .unwrap_or((Operator::BeginsWith, value.as_str()));
    let canonical = match locale.parse_value(rest) {
        Some(n) if !rest.is_empty() => crate::formula::Expr::Number(n).write(&crate::EN),
        _ => rest.to_owned(),
    };
    let op = match (op, locale.parse_value(rest)) {
        (Operator::BeginsWith, Some(_)) => Operator::Equal,
        (op, _) => op,
    };
    Some(Condition {
        op,
        value: canonical,
    })
}

pub fn advanced_rows(
    sheet: &Sheet,
    list: CellRange,
    criteria: Option<CellRange>,
    unique: bool,
    locale: &Locale,
) -> Vec<u32> {
    let header_row = list.start().row();
    let column_of = |label: &str| {
        let key = fold(label.trim());
        (list.start().col()..=list.end().col()).find(|col| {
            CellAddr::new(header_row, *col)
                .is_ok_and(|addr| fold(&source_text(sheet.get(addr), locale)) == key)
        })
    };
    let mut rules: Vec<Vec<(u32, Condition)>> = Vec::new();
    if let Some(criteria) = criteria {
        let columns: Vec<Option<u32>> = (criteria.start().col()..=criteria.end().col())
            .map(|col| {
                let addr = CellAddr::new(criteria.start().row(), col).ok()?;
                column_of(&source_text(sheet.get(addr), locale))
            })
            .collect();
        for row in criteria.start().row() + 1..=criteria.end().row() {
            let conditions: Vec<(u32, Condition)> = (criteria.start().col()..=criteria.end().col())
                .zip(&columns)
                .filter_map(|(col, target)| {
                    let cell = sheet.get(CellAddr::new(row, col).ok()?)?;
                    Some(((*target)?, criterion(cell, locale)?))
                })
                .collect();
            rules.push(conditions);
        }
    }
    let context = Context {
        threshold: None,
        average: None,
        today: 0.0,
    };
    let mut seen = HashSet::new();
    (header_row + 1..=list.end().row())
        .filter(|row| {
            let passes = rules.is_empty()
                || rules.iter().any(|conditions| {
                    conditions.iter().all(|(col, condition)| {
                        let Ok(addr) = CellAddr::new(*row, *col) else {
                            return false;
                        };
                        let text = locale.format_value(
                            sheet.value(addr),
                            sheet.get(addr).map_or("", Cell::number_format),
                        );
                        let entry = Entry {
                            value: sheet.value(addr),
                            text: &text,
                            fill: None,
                            color: None,
                        };
                        let filter = ColumnFilter::Custom {
                            first: condition.clone(),
                            second: None,
                            and: true,
                        };
                        matches(&filter, &entry, &context, locale)
                    })
                });
            if !passes {
                return false;
            }
            if !unique {
                return true;
            }
            let row_key: Vec<String> = (list.start().col()..=list.end().col())
                .filter_map(|col| CellAddr::new(*row, col).ok())
                .map(|addr| key(sheet.value(addr)))
                .collect();
            seen.insert(row_key)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CellError, TR};

    fn delimited(comma: bool, semicolon: bool, merge: bool) -> Split {
        Split::Delimited {
            delimiters: Delimiters {
                comma,
                semicolon,
                space: merge,
                ..Delimiters::default()
            },
            merge,
            qualifier: Some('"'),
        }
    }

    #[test]
    fn splits_delimited_text_with_qualifiers() {
        assert_eq!(
            split_line(
                "a;\"b;c\";\"say \"\"x\"\"\";",
                &delimited(false, true, false)
            ),
            ["a", "b;c", "say \"x\"", ""]
        );
        assert_eq!(
            split_line("Ali  Veli   Ayşe", &delimited(false, false, true)),
            ["Ali", "Veli", "Ayşe"]
        );
        assert_eq!(
            split_line("1,2,,3", &delimited(true, false, false)),
            ["1", "2", "", "3"]
        );
    }

    #[test]
    fn splits_fixed_width_text() {
        let split = Split::Fixed(vec![10, 4, 40]);
        assert_eq!(
            split_line("0532ANKARA   ÇANKAYA", &split),
            ["0532", "ANKARA", "ÇANKAYA"]
        );
    }

    #[test]
    fn converts_columns_by_kind() {
        let number = convert("1.234,5", ColumnKind::General, &TR).unwrap();
        assert_eq!(number.value, CellValue::Number(1234.5));
        let text = convert("00123", ColumnKind::Text, &TR).unwrap();
        assert_eq!(text.value, CellValue::Text("00123".into()));
        assert_eq!(text.number_format(), "@");
        let date = convert("03/15/24", ColumnKind::Date(DateOrder::Mdy), &TR).unwrap();
        assert_eq!(
            date.value,
            CellValue::Number(date_serial(2024, 3, 15).unwrap())
        );
        let ymd = convert("2024.03.15", ColumnKind::Date(DateOrder::Ymd), &TR).unwrap();
        assert_eq!(ymd.value, date.value);
        assert_eq!(convert("x", ColumnKind::Skip, &TR), None);
        assert_eq!(
            convert("=1+1", ColumnKind::General, &TR).unwrap().value,
            CellValue::Text("=1+1".into())
        );
        assert_eq!(
            convert("#YOK", ColumnKind::General, &TR).unwrap().value,
            CellValue::Error(CellError::NA)
        );
    }

    #[test]
    fn text_to_columns_writes_to_the_destination() {
        let mut sheet = Sheet::new("S");
        sheet.set_input("A1".parse().unwrap(), "Ad;Yaş;Şehir");
        sheet.set_input("A2".parse().unwrap(), "Ayşe;30;İzmir");
        let edits = text_to_columns(
            &sheet,
            "A1:A3".parse().unwrap(),
            &delimited(false, true, false),
            &[ColumnKind::General, ColumnKind::Skip],
            "B1".parse().unwrap(),
            &TR,
        );
        let written: Vec<_> = edits
            .iter()
            .map(|(addr, cell)| format!("{addr}={}", TR.display(&cell.value)))
            .collect();
        assert_eq!(written, ["B1=Ad", "C1=Şehir", "B2=Ayşe", "C2=İzmir"]);
    }

    #[test]
    fn advanced_criteria_combine_rows_and_columns() {
        let mut sheet = Sheet::new("S");
        for (at, input) in [
            ("A1", "Şehir"),
            ("B1", "Tutar"),
            ("A2", "İzmir"),
            ("B2", "50"),
            ("A3", "Ankara"),
            ("B3", "500"),
            ("A4", "İstanbul"),
            ("B4", "5"),
            ("A5", "İzmir"),
            ("B5", "50"),
            ("D1", "şehir"),
            ("E1", "Tutar"),
            ("D2", "i"),
            ("E2", ">10"),
            ("E3", ">=500"),
        ] {
            sheet.set_input(at.parse().unwrap(), input);
        }
        let list = "A1:B5".parse().unwrap();
        let criteria = Some("D1:E3".parse().unwrap());
        assert_eq!(advanced_rows(&sheet, list, criteria, false, &TR), [1, 2, 4]);
        assert_eq!(advanced_rows(&sheet, list, criteria, true, &TR), [1, 2]);
        assert_eq!(advanced_rows(&sheet, list, None, true, &TR), [1, 2, 3]);
        let exact = Some("D1:D2".parse().unwrap());
        sheet.set_input("D2".parse().unwrap(), "'=Ankara");
        assert_eq!(advanced_rows(&sheet, list, exact, false, &TR), [2]);
    }

    #[test]
    fn flash_fill_learns_from_examples() {
        let row = |sources: &[&str], example: Option<&str>| {
            (
                sources.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
                example.map(str::to_owned),
            )
        };
        let rows = [
            row(&["ali veli", "İzmir"], Some("Veli, A. (İZMİR)")),
            row(&["ayşe yılmaz", "Bursa"], None),
            row(&["can öz", "ıspir"], None),
        ];
        let filled = flash_fill(&rows, &TR).unwrap();
        assert_eq!(filled[0], None);
        assert_eq!(filled[1].as_deref(), Some("Yılmaz, A. (BURSA)"));
        assert_eq!(filled[2].as_deref(), Some("Öz, C. (ISPİR)"));
        let emails = [
            row(&["ali.veli@firma.com"], Some("ali")),
            row(&["zeynep@okul.edu.tr"], None),
        ];
        assert_eq!(
            flash_fill(&emails, &TR).unwrap()[1].as_deref(),
            Some("zeynep")
        );
        let last = [
            row(&["Mehmet Ali Kaya"], Some("Kaya")),
            row(&["Su Ak"], Some("Ak")),
            row(&["Ali Rıza Bey Demir"], None),
        ];
        assert_eq!(flash_fill(&last, &TR).unwrap()[2].as_deref(), Some("Demir"));
        assert_eq!(flash_fill(&[row(&["a"], None)], &TR), None);
    }

    #[test]
    fn finds_unique_rows_case_insensitively() {
        let mut sheet = Sheet::new("S");
        for (at, input) in [
            ("A1", "Ad"),
            ("B1", "Şehir"),
            ("A2", "Ali"),
            ("B2", "İzmir"),
            ("A3", "ali"),
            ("B3", "izmir"),
            ("A4", "Ali"),
            ("B4", "Bursa"),
        ] {
            sheet.set_input(at.parse().unwrap(), input);
        }
        let range = "A1:B4".parse().unwrap();
        assert_eq!(unique_rows(&sheet, range, &[0, 1], true), [1, 3]);
        assert_eq!(unique_rows(&sheet, range, &[0], true), [1]);
        assert_eq!(unique_rows(&sheet, range, &[0], false), [0, 1]);
    }
}
