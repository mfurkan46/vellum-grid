use crate::formula::{self, Evaluator, Expr};
use crate::locale::{EN, fold};
use crate::{CellAddr, CellRange, CellValue, Locale, Workbook, value::round_significant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Allow {
    #[default]
    Any,
    Whole,
    Decimal,
    List,
    Date,
    Time,
    TextLength,
    Custom,
}

impl Allow {
    pub const ALL: [Self; 8] = [
        Self::Any,
        Self::Whole,
        Self::Decimal,
        Self::List,
        Self::Date,
        Self::Time,
        Self::TextLength,
        Self::Custom,
    ];

    pub fn compares(self) -> bool {
        matches!(
            self,
            Self::Whole | Self::Decimal | Self::Date | Self::Time | Self::TextLength
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compare {
    #[default]
    Between,
    NotBetween,
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterOrEqual,
    LessOrEqual,
}

impl Compare {
    pub const ALL: [Self; 8] = [
        Self::Between,
        Self::NotBetween,
        Self::Equal,
        Self::NotEqual,
        Self::Greater,
        Self::Less,
        Self::GreaterOrEqual,
        Self::LessOrEqual,
    ];

    pub fn two_operands(self) -> bool {
        matches!(self, Self::Between | Self::NotBetween)
    }

    fn test(self, value: f64, first: f64, second: f64) -> bool {
        match self {
            Self::Between => value >= first.min(second) && value <= first.max(second),
            Self::NotBetween => value < first.min(second) || value > first.max(second),
            Self::Equal => value == first,
            Self::NotEqual => value != first,
            Self::Greater => value > first,
            Self::Less => value < first,
            Self::GreaterOrEqual => value >= first,
            Self::LessOrEqual => value <= first,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertStyle {
    #[default]
    Stop,
    Warning,
    Information,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Validation {
    pub ranges: Vec<CellRange>,
    pub allow: Allow,
    pub compare: Compare,
    pub formula1: String,
    pub formula2: String,
    pub ignore_blank: bool,
    pub dropdown: bool,
    pub show_input: bool,
    pub input_title: String,
    pub input_message: String,
    pub show_error: bool,
    pub style: AlertStyle,
    pub error_title: String,
    pub error_message: String,
}

impl Validation {
    pub fn new(ranges: Vec<CellRange>) -> Self {
        Self {
            ranges,
            ignore_blank: true,
            dropdown: true,
            show_input: true,
            show_error: true,
            ..Self::default()
        }
    }

    pub fn covers(&self, addr: CellAddr) -> bool {
        self.ranges.iter().any(|range| range.contains(addr))
    }

    fn anchor(&self) -> Option<CellAddr> {
        self.ranges.first().map(|range| range.start())
    }

    fn operand(&self, text: &str, at: CellAddr) -> Option<Expr> {
        let expr = formula::parse(text, &EN).ok()?;
        let anchor = self.anchor()?;
        let rows = i64::from(at.row()) - i64::from(anchor.row());
        let cols = i64::from(at.col()) - i64::from(anchor.col());
        Some(expr.shifted(rows, cols))
    }
}

pub fn subtract(range: CellRange, cut: CellRange) -> Vec<CellRange> {
    let (r0, c0, r1, c1) = (
        range.start().row(),
        range.start().col(),
        range.end().row(),
        range.end().col(),
    );
    let (k0, l0, k1, l1) = (
        cut.start().row(),
        cut.start().col(),
        cut.end().row(),
        cut.end().col(),
    );
    if k0 > r1 || k1 < r0 || l0 > c1 || l1 < c0 {
        return vec![range];
    }
    let at = |row, col| CellAddr::new(row, col).expect("inside the range");
    let mut out = Vec::new();
    if k0 > r0 {
        out.push(CellRange::new(at(r0, c0), at(k0 - 1, c1)));
    }
    if k1 < r1 {
        out.push(CellRange::new(at(k1 + 1, c0), at(r1, c1)));
    }
    let (m0, m1) = (k0.max(r0), k1.min(r1));
    if l0 > c0 {
        out.push(CellRange::new(at(m0, c0), at(m1, l0 - 1)));
    }
    if l1 < c1 {
        out.push(CellRange::new(at(m0, l1 + 1), at(m1, c1)));
    }
    out
}

pub fn literal_items(text: &str) -> Option<Vec<String>> {
    match formula::parse(text, &EN).ok()? {
        Expr::Text(list) => Some(list.split(',').map(|item| item.trim().to_owned()).collect()),
        _ => None,
    }
}

pub fn literal(items: &[String]) -> String {
    Expr::Text(items.join(",")).write(&EN)
}

fn evaluator<'a>(
    book: &'a Workbook,
    sheet: usize,
    at: CellAddr,
    locale: &'a Locale,
) -> Evaluator<'a> {
    Evaluator {
        book,
        sheet,
        locale,
        at,
        today: 0.0,
        now: 0.0,
    }
}

pub fn list_items(
    book: &Workbook,
    sheet: usize,
    validation: &Validation,
    locale: &Locale,
) -> Vec<String> {
    if let Some(items) = literal_items(&validation.formula1) {
        return items;
    }
    let Some(anchor) = validation.anchor() else {
        return Vec::new();
    };
    let Some(expr) = validation.operand(&validation.formula1, anchor) else {
        return Vec::new();
    };
    let expr = book.expand_names(&expr, sheet);
    let Some((target, range)) = expr.references().first().map(|(name, range)| {
        let index = name.and_then(|name| book.index_of(name)).unwrap_or(sheet);
        (index, *range)
    }) else {
        return Vec::new();
    };
    let source = &book.sheets()[target];
    let range = source.trim_to_extent(range);
    let mut items = Vec::new();
    for row in range.start().row()..=range.end().row() {
        for col in range.start().col()..=range.end().col() {
            let Ok(addr) = CellAddr::new(row, col) else {
                continue;
            };
            let value = source.value(addr);
            if !value.is_empty() {
                items.push(
                    locale.format_value(value, source.get(addr).map_or("", |c| c.number_format())),
                );
            }
        }
    }
    items
}

pub fn is_valid(
    book: &Workbook,
    sheet: usize,
    at: CellAddr,
    value: &CellValue,
    validation: &Validation,
    locale: &Locale,
) -> bool {
    if value.is_empty() {
        return validation.ignore_blank || validation.allow == Allow::Any;
    }
    let ev = evaluator(book, sheet, at, locale);
    let number = |text: &str| -> Option<f64> {
        let expr = book.expand_names(&validation.operand(text, at)?, sheet);
        match ev.evaluate(&expr) {
            CellValue::Number(n) => Some(n),
            CellValue::Text(text) => locale.parse_value(&text),
            _ => None,
        }
    };
    let compare = |subject: f64| -> bool {
        let Some(first) = number(&validation.formula1) else {
            return false;
        };
        let second = if validation.compare.two_operands() {
            match number(&validation.formula2) {
                Some(second) => second,
                None => return false,
            }
        } else {
            0.0
        };
        validation
            .compare
            .test(round_significant(subject), first, second)
    };
    match validation.allow {
        Allow::Any => true,
        Allow::Whole => {
            matches!(value, CellValue::Number(n) if n.fract() == 0.0 && compare(*n))
        }
        Allow::Decimal | Allow::Date => matches!(value, CellValue::Number(n) if compare(*n)),
        Allow::Time => matches!(value, CellValue::Number(n) if compare(n.fract())),
        Allow::TextLength => compare(locale.display(value).chars().count() as f64),
        Allow::List => {
            let shown = fold(&locale.display(value));
            list_items(book, sheet, validation, locale)
                .iter()
                .any(|item| fold(item) == shown)
        }
        Allow::Custom => {
            let Some(expr) = validation.operand(&validation.formula1, at) else {
                return false;
            };
            let expr = book.expand_names(&expr, sheet);
            match ev.evaluate(&expr) {
                CellValue::Bool(ok) => ok,
                CellValue::Number(n) => n != 0.0,
                _ => false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sheet, TR};

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn range(s: &str) -> CellRange {
        s.parse().unwrap()
    }

    fn book() -> Workbook {
        let mut sheet = Sheet::new("S");
        for (at, input) in [
            ("E1", "Elma"),
            ("E2", "Armut"),
            ("E3", "Çilek"),
            ("F1", "100"),
        ] {
            sheet.set_input(addr(at), input);
        }
        Workbook::from_sheets(vec![sheet]).unwrap()
    }

    fn rule(allow: Allow, compare: Compare, first: &str, second: &str) -> Validation {
        Validation {
            allow,
            compare,
            formula1: first.to_owned(),
            formula2: second.to_owned(),
            ..Validation::new(vec![range("A1:A10")])
        }
    }

    fn check(validation: &Validation, at: &str, value: CellValue) -> bool {
        is_valid(&book(), 0, addr(at), &value, validation, &TR)
    }

    #[test]
    fn checks_numbers_and_lengths() {
        let whole = rule(Allow::Whole, Compare::Between, "1", "$F$1");
        assert!(check(&whole, "A1", CellValue::Number(50.0)));
        assert!(!check(&whole, "A1", CellValue::Number(50.5)));
        assert!(!check(&whole, "A1", CellValue::Number(101.0)));
        assert!(!check(&whole, "A1", CellValue::Text("abc".into())));
        assert!(check(&whole, "A1", CellValue::Empty));
        let decimal = rule(Allow::Decimal, Compare::Greater, "0", "");
        assert!(check(&decimal, "A2", CellValue::Number(0.5)));
        assert!(!check(&decimal, "A2", CellValue::Number(-1.0)));
        let length = rule(Allow::TextLength, Compare::LessOrEqual, "3", "");
        assert!(check(&length, "A1", CellValue::Text("abc".into())));
        assert!(!check(&length, "A1", CellValue::Text("abcd".into())));
    }

    #[test]
    fn checks_lists_and_custom_formulas() {
        let literal_list = rule(
            Allow::List,
            Compare::Between,
            &literal(&["Evet".into(), "Hayır".into()]),
            "",
        );
        assert_eq!(
            literal_items(&literal_list.formula1).unwrap(),
            ["Evet", "Hayır"]
        );
        assert!(check(&literal_list, "A1", CellValue::Text("evet".into())));
        assert!(!check(&literal_list, "A1", CellValue::Text("Belki".into())));
        let range_list = rule(Allow::List, Compare::Between, "$E$1:$E$5", "");
        assert_eq!(
            list_items(&book(), 0, &range_list, &TR),
            ["Elma", "Armut", "Çilek"]
        );
        assert!(check(&range_list, "A3", CellValue::Text("çilek".into())));
        let custom = rule(Allow::Custom, Compare::Between, "$F$1>50", "");
        assert!(check(&custom, "A1", CellValue::Number(3.0)));
        let failing = rule(Allow::Custom, Compare::Between, "$F$1>500", "");
        assert!(!check(&failing, "A1", CellValue::Number(3.0)));
        let mut strict = custom.clone();
        strict.ignore_blank = false;
        assert!(!check(&strict, "A1", CellValue::Empty));
    }

    #[test]
    fn subtracts_ranges() {
        let parts: Vec<String> = subtract(range("A1:C3"), range("B2"))
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(parts, ["A1:C1", "A3:C3", "A2", "C2"]);
        assert_eq!(subtract(range("A1:A3"), range("A1:A3")), []);
        assert_eq!(subtract(range("A1"), range("D4")), [range("A1")]);
    }
}
