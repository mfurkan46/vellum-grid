use std::fmt;
use std::ops::Range;

use std::sync::Arc;

use crate::formula::{self, FormulaError};
use crate::locale::{EN, Locale};
use crate::numfmt;
use crate::style::{DEFAULT_STYLE, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellError {
    Null,
    Div0,
    Value,
    Ref,
    Name,
    Num,
    NA,
}

impl CellError {
    pub fn code(self) -> &'static str {
        EN.error_name(self)
    }
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum CellValue {
    #[default]
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
    Error(CellError),
}

impl CellValue {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => Ok(()),
            Self::Number(n) => f.write_str(&format_number(*n)),
            Self::Text(s) => f.write_str(s),
            Self::Bool(true) => f.write_str("TRUE"),
            Self::Bool(false) => f.write_str("FALSE"),
            Self::Error(e) => e.fmt(f),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell {
    pub value: CellValue,
    pub formula: Option<String>,
    pub style: Option<Arc<Style>>,
}

impl Cell {
    pub fn parse(input: &str) -> Self {
        Self::parse_in(input, &EN).unwrap_or_else(|_| CellValue::Text(input.to_owned()).into())
    }

    pub fn parse_in(input: &str, locale: &Locale) -> Result<Self, FormulaError> {
        if let Some(text) = input.strip_prefix('\'') {
            return Ok(CellValue::Text(text.to_owned()).into());
        }
        if let Some(body) = input.strip_prefix('=').filter(|body| !body.is_empty()) {
            let expr = formula::parse(body, locale)?;
            return Ok(Self {
                formula: Some(format!("={}", expr.write(&EN))),
                ..Self::default()
            });
        }
        if let Some((serial, code)) = locale.parse_date_time(input) {
            return Ok(Self::number(serial, Some(code)));
        }
        let value = if input.is_empty() {
            CellValue::Empty
        } else if let Some(b) = locale.parse_bool(input) {
            CellValue::Bool(b)
        } else if let Some(e) = locale.parse_error(input) {
            CellValue::Error(e)
        } else if let Some(n) = locale.parse_number(input) {
            if input.trim_end().ends_with('%') {
                let whole = round_significant(n * 100.0).fract() == 0.0;
                let code = if whole {
                    numfmt::PERCENT
                } else {
                    numfmt::PERCENT_DECIMAL
                };
                return Ok(Self::number(n, Some(code)));
            }
            CellValue::Number(n)
        } else {
            CellValue::Text(input.to_owned())
        };
        Ok(value.into())
    }

    fn number(n: f64, code: Option<&str>) -> Self {
        Self {
            value: CellValue::Number(n),
            formula: None,
            style: code.map(|code| Arc::new(Style::with_number_format(code))),
        }
    }

    pub fn style(&self) -> &Style {
        self.style.as_deref().unwrap_or(&DEFAULT_STYLE)
    }

    pub fn number_format(&self) -> &str {
        &self.style().number_format
    }

    pub fn set_style(&mut self, style: Style) {
        self.style = (!style.is_default()).then(|| Arc::new(style));
    }

    pub fn is_blank(&self) -> bool {
        self.formula.is_none() && self.value.is_empty() && self.style().is_default()
    }

    pub fn has_content(&self) -> bool {
        self.formula.is_some() || !self.value.is_empty()
    }

    pub fn input_text(&self) -> String {
        self.input_text_in(&EN)
    }

    pub fn input_text_in(&self, locale: &Locale) -> String {
        if let Some(formula) = &self.formula {
            return match formula::parse(&formula[1..], &EN) {
                Ok(expr) => format!("={}", expr.write(locale)),
                Err(_) => formula.clone(),
            };
        }
        match &self.value {
            CellValue::Text(s)
                if Self::parse_in(s, locale).ok().is_none_or(|parsed| {
                    parsed.value != self.value || parsed.formula.is_some()
                }) =>
            {
                format!("'{s}")
            }
            value => locale.edit_text(value, self.number_format()),
        }
    }

    pub fn shifted(&self, rows: i64, cols: i64) -> Self {
        match self
            .formula
            .as_deref()
            .map(|f| formula::parse(&f[1..], &EN))
        {
            Some(Ok(expr)) => Self {
                value: CellValue::Empty,
                formula: Some(format!("={}", expr.shifted(rows, cols).write(&EN))),
                style: self.style.clone(),
            },
            _ => self.clone(),
        }
    }
}

impl From<CellValue> for Cell {
    fn from(value: CellValue) -> Self {
        Self {
            value,
            ..Self::default()
        }
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.formula {
            Some(formula) => f.write_str(formula),
            None => f.write_str(&EN.format_value(&self.value, self.number_format())),
        }
    }
}

pub(crate) fn parse_canonical_number(s: &str) -> Option<f64> {
    if !s
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-' | b'e' | b'E'))
    {
        return None;
    }
    let n = s.parse::<f64>().ok().filter(|n| n.is_finite())?;
    Some(if n == 0.0 { 0.0 } else { n })
}

pub(crate) fn round_significant(n: f64) -> f64 {
    if n.is_finite() {
        format!("{:.*e}", SIGNIFICANT_DIGITS - 1, n)
            .parse()
            .unwrap_or(n)
    } else {
        n
    }
}

pub fn round_decimal(n: f64, places: i32) -> f64 {
    if !n.is_finite() || n == 0.0 {
        return n;
    }
    let decimal = Decimal::round(n, SIGNIFICANT_DIGITS);
    let keep = decimal.exponent + 1 + places;
    let digits = decimal.digits.as_bytes();
    if keep >= digits.len() as i32 {
        return round_significant(n);
    }
    if keep < 0 {
        return 0.0;
    }
    let keep = keep as usize;
    let mut mantissa: u64 = decimal.digits[..keep].parse().unwrap_or(0);
    if digits[keep] >= b'5' {
        mantissa += 1;
    }
    let scale = decimal.exponent + 1 - keep as i32;
    let magnitude: f64 = format!("{mantissa}e{scale}").parse().unwrap_or(0.0);
    if decimal.negative {
        -magnitude
    } else {
        magnitude
    }
}

const SIGNIFICANT_DIGITS: usize = 15;
const PLAIN_EXPONENTS: Range<i32> = -9..SIGNIFICANT_DIGITS as i32;

pub fn format_number(n: f64) -> String {
    special_number(n).unwrap_or_else(|| {
        let decimal = Decimal::round(n, SIGNIFICANT_DIGITS);
        if PLAIN_EXPONENTS.contains(&decimal.exponent) {
            decimal.plain()
        } else {
            decimal.scientific()
        }
    })
}

pub fn format_number_fit(n: f64, max_len: usize) -> Option<String> {
    let fits = |s: String| (s.len() <= max_len).then_some(s);
    if let Some(s) = special_number(n) {
        return fits(s);
    }
    let precisions = || (1..=SIGNIFICANT_DIGITS).rev();
    precisions()
        .map(|significant| Decimal::round(n, significant))
        .filter(|decimal| PLAIN_EXPONENTS.contains(&decimal.exponent))
        .find_map(|decimal| fits(decimal.plain()))
        .or_else(|| {
            precisions().find_map(|significant| fits(Decimal::round(n, significant).scientific()))
        })
}

fn special_number(n: f64) -> Option<String> {
    if !n.is_finite() {
        Some(CellError::Num.code().to_owned())
    } else if n == 0.0 {
        Some("0".to_owned())
    } else {
        None
    }
}

struct Decimal {
    negative: bool,
    digits: String,
    exponent: i32,
}

impl Decimal {
    fn round(n: f64, significant: usize) -> Self {
        let sci = format!("{:.*e}", significant - 1, n.abs());
        let (mantissa, exponent) = sci
            .split_once('e')
            .expect("LowerExp output has an exponent");
        let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
        Self {
            negative: n < 0.0,
            digits: digits.trim_end_matches('0').to_owned(),
            exponent: exponent.parse().expect("LowerExp exponent is an integer"),
        }
    }

    fn sign(&self) -> &'static str {
        if self.negative { "-" } else { "" }
    }

    fn plain(&self) -> String {
        let digits = &self.digits;
        let body = if self.exponent < 0 {
            format!("0.{}{digits}", "0".repeat((-self.exponent - 1) as usize))
        } else {
            let int_len = self.exponent as usize + 1;
            if digits.len() <= int_len {
                format!("{digits}{}", "0".repeat(int_len - digits.len()))
            } else {
                format!("{}.{}", &digits[..int_len], &digits[int_len..])
            }
        };
        format!("{}{body}", self.sign())
    }

    fn scientific(&self) -> String {
        let (first, rest) = self.digits.split_at(1);
        let fraction = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        let exp_sign = if self.exponent < 0 { '-' } else { '+' };
        format!(
            "{}{first}{fraction}E{exp_sign}{:02}",
            self.sign(),
            self.exponent.abs()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(input: &str) -> CellValue {
        Cell::parse(input).value
    }

    #[test]
    fn parses_scalar_inputs() {
        assert_eq!(parsed(""), CellValue::Empty);
        assert_eq!(parsed("42"), CellValue::Number(42.0));
        assert_eq!(parsed("-1.5"), CellValue::Number(-1.5));
        assert_eq!(parsed(".5"), CellValue::Number(0.5));
        assert_eq!(parsed("1E+3"), CellValue::Number(1000.0));
        assert_eq!(parsed("true"), CellValue::Bool(true));
        assert_eq!(parsed("FALSE"), CellValue::Bool(false));
        assert_eq!(parsed("#DIV/0!"), CellValue::Error(CellError::Div0));
        assert_eq!(parsed("#n/a"), CellValue::Error(CellError::NA));
        assert_eq!(parsed("elma"), CellValue::Text("elma".into()));
    }

    #[test]
    fn unsupported_numbers_stay_text() {
        for s in [
            "inf", "NaN", "infinity", "1e999", "1,5", " 1", "1 ", "0x10", "1-",
        ] {
            assert_eq!(parsed(s), CellValue::Text(s.into()), "{s}");
        }
    }

    #[test]
    fn negative_zero_is_normalized() {
        let CellValue::Number(n) = parsed("-0") else {
            panic!()
        };
        assert!(n.is_sign_positive());
    }

    #[test]
    fn apostrophe_forces_text() {
        assert_eq!(parsed("'007"), CellValue::Text("007".into()));
        assert_eq!(parsed("'=A1"), CellValue::Text("=A1".into()));
        assert_eq!(parsed("'"), CellValue::Text(String::new()));
    }

    #[test]
    fn formulas_are_kept_as_raw_text() {
        let cell = Cell::parse("=SUM(A1:A3)");
        assert_eq!(cell.formula.as_deref(), Some("=SUM(A1:A3)"));
        assert_eq!(cell.value, CellValue::Empty);
        assert!(!cell.is_blank());
        assert_eq!(cell.to_string(), "=SUM(A1:A3)");
        assert_eq!(parsed("="), CellValue::Text("=".into()));
    }

    #[test]
    fn input_text_parses_back_to_the_same_cell() {
        let cells = [
            Cell::parse("12.5"),
            Cell::parse("TRUE"),
            Cell::parse("#REF!"),
            Cell::parse("elma"),
            Cell::parse("=A1*2"),
            Cell::parse("'007"),
            Cell::parse("'=A1"),
            Cell::parse("'TRUE"),
            Cell::parse("''x"),
            Cell::parse("'"),
        ];
        for cell in cells {
            assert_eq!(Cell::parse(&cell.input_text()), cell, "{cell:?}");
        }
        assert_eq!(Cell::parse("'007").input_text(), "'007");
        assert_eq!(Cell::parse("elma").input_text(), "elma");
    }

    #[test]
    fn displays_values() {
        assert_eq!(CellValue::Empty.to_string(), "");
        assert_eq!(CellValue::Bool(true).to_string(), "TRUE");
        assert_eq!(CellValue::Error(CellError::Name).to_string(), "#NAME?");
        assert_eq!(CellValue::Number(2.0).to_string(), "2");
    }

    #[test]
    fn formats_with_fifteen_significant_digits() {
        assert_eq!(format_number(0.1 + 0.2), "0.3");
        assert_eq!(format_number(1.0 - 0.9), "0.1");
        assert_eq!(format_number(1.1 * 3.0), "3.3");
        assert_eq!(format_number(1.0 / 3.0), "0.333333333333333");
        assert_eq!(format_number(2.0 / 3.0), "0.666666666666667");
    }

    #[test]
    fn formats_plain_numbers() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(1.0), "1");
        assert_eq!(format_number(-2.5), "-2.5");
        assert_eq!(format_number(1500.0), "1500");
        assert_eq!(format_number(0.000123), "0.000123");
        assert_eq!(format_number(123_456_789_012_345.0), "123456789012345");
        assert_eq!(format_number(0.000_000_001), "0.000000001");
    }

    #[test]
    fn formats_scientific_numbers() {
        assert_eq!(format_number(1e15), "1E+15");
        assert_eq!(format_number(-1.25e20), "-1.25E+20");
        assert_eq!(format_number(1e-10), "1E-10");
        assert_eq!(format_number(9.999_999_999_999_999e14), "1E+15");
        assert_eq!(format_number(f64::MAX), "1.79769313486232E+308");
        assert_eq!(format_number(f64::NAN), "#NUM!");
    }

    #[test]
    fn fits_numbers_into_column_width() {
        assert_eq!(format_number_fit(42.0, 10).as_deref(), Some("42"));
        assert_eq!(format_number_fit(1.0 / 3.0, 8).as_deref(), Some("0.333333"));
        assert_eq!(
            format_number_fit(1_234_567.891, 8).as_deref(),
            Some("1234568")
        );
        assert_eq!(
            format_number_fit(123_456_789_012.0, 8).as_deref(),
            Some("1.23E+11")
        );
        assert_eq!(
            format_number_fit(0.000_123_456, 7).as_deref(),
            Some("0.00012")
        );
        assert_eq!(format_number_fit(1e15, 5).as_deref(), Some("1E+15"));
        assert_eq!(format_number_fit(0.1 + 0.2, 3).as_deref(), Some("0.3"));
        assert_eq!(format_number_fit(0.0, 1).as_deref(), Some("0"));
    }

    #[test]
    fn fit_fails_when_nothing_fits() {
        assert_eq!(format_number_fit(-0.5, 2), None);
        assert_eq!(format_number_fit(123_456.0, 0), None);
    }

    #[test]
    fn decimal_inputs_round_trip_through_display() {
        for s in [
            "0.1",
            "19.99",
            "-1234.5678",
            "100000000000000",
            "0.000001",
            "3.14159265358979",
        ] {
            assert_eq!(parsed(s).to_string(), s);
        }
    }
}
