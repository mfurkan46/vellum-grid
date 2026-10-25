use std::cmp::Ordering;

use super::{BinOp, Expr, functions};
use crate::locale::fold;
use crate::numfmt;
use crate::value::round_significant;
use crate::{CellAddr, CellError, CellRange, CellValue, Locale, Sheet, Workbook};

#[derive(Clone, Copy)]
pub struct Area<'a> {
    pub sheet: &'a Sheet,
    pub range: CellRange,
}

pub enum Value<'a> {
    Scalar(CellValue),
    Range(Area<'a>),
}

pub struct Evaluator<'a> {
    pub book: &'a Workbook,
    pub sheet: usize,
    pub locale: &'a Locale,
    pub at: CellAddr,
    pub today: f64,
    pub now: f64,
}

impl<'a> Evaluator<'a> {
    pub fn evaluate(&self, expr: &Expr) -> CellValue {
        match self.scalar(expr) {
            CellValue::Empty => CellValue::Number(0.0),
            CellValue::Number(n) => number(Ok(n)),
            value => value,
        }
    }

    pub fn infer_format(&self, expr: &Expr) -> Option<String> {
        let is_date = |code: &Option<String>| {
            code.as_deref()
                .is_some_and(|c| numfmt::kind(c) == numfmt::Kind::Date)
        };
        match expr {
            Expr::Paren(inner) | Expr::Neg(inner) | Expr::Plus(inner) => self.infer_format(inner),
            Expr::Reference { sheet, reference } => self
                .resolve(sheet.as_deref())
                .ok()
                .and_then(|s| s.get(reference.range().start()))
                .filter(|cell| !cell.style().has_general_format())
                .map(|cell| cell.number_format().to_owned()),
            Expr::Binary(op @ (BinOp::Add | BinOp::Sub), left, right) => {
                match (self.infer_format(left), self.infer_format(right)) {
                    (Some(f), None) => Some(f),
                    (None, Some(f)) if *op == BinOp::Add => Some(f),
                    (Some(a), Some(b)) if !is_date(&Some(a.clone())) && a == b => Some(a),
                    _ => None,
                }
            }
            Expr::Binary(BinOp::Mul | BinOp::Div, left, _) => Some(self.infer_format(left))
                .filter(|f| !is_date(f))
                .flatten(),
            Expr::Call(name, _)
                if matches!(
                    name.as_str(),
                    "TODAY" | "DATE" | "EDATE" | "EOMONTH" | "WORKDAY" | "DATEVALUE"
                ) =>
            {
                Some(numfmt::DATE.to_owned())
            }
            Expr::Call(name, _) if name == "NOW" => Some(numfmt::DATE_TIME.to_owned()),
            Expr::Call(name, _) if name == "TIME" => Some(numfmt::TIME.to_owned()),
            Expr::Call(name, args)
                if matches!(
                    name.as_str(),
                    "IF" | "IFERROR" | "MIN" | "MAX" | "SUM" | "AVERAGE" | "ROUND" | "ABS"
                ) =>
            {
                args.iter().find_map(|arg| self.infer_format(arg))
            }
            _ => None,
        }
    }

    pub(super) fn value(&self, expr: &Expr) -> Value<'a> {
        let scalar = |v| Value::Scalar(v);
        match expr {
            Expr::Number(n) => scalar(CellValue::Number(*n)),
            Expr::Text(s) => scalar(CellValue::Text(s.clone())),
            Expr::Bool(b) => scalar(CellValue::Bool(*b)),
            Expr::Error(e) => scalar(CellValue::Error(*e)),
            Expr::Missing => scalar(CellValue::Empty),
            Expr::Name(_) => scalar(CellValue::Error(CellError::Name)),
            Expr::Structured(_) => scalar(CellValue::Error(CellError::Ref)),
            Expr::Reference { sheet, reference } => match self.resolve(sheet.as_deref()) {
                Ok(sheet) => {
                    let range = reference.range();
                    if range.is_single() {
                        scalar(sheet.value(range.start()).clone())
                    } else {
                        Value::Range(Area { sheet, range })
                    }
                }
                Err(e) => scalar(CellValue::Error(e)),
            },
            Expr::Paren(inner) | Expr::Plus(inner) => self.value(inner),
            Expr::Neg(inner) => scalar(number(self.number(inner).map(|n| -n))),
            Expr::Percent(inner) => scalar(number(self.number(inner).map(|n| n / 100.0))),
            Expr::Binary(op, left, right) => scalar(self.binary(*op, left, right)),
            Expr::Call(name, args) => functions::call(self, name, args),
        }
    }

    fn resolve(&self, sheet: Option<&str>) -> Result<&'a Sheet, CellError> {
        match sheet {
            None => Ok(&self.book.sheets()[self.sheet]),
            Some(name) => self.book.sheet(name).ok_or(CellError::Ref),
        }
    }

    pub(super) fn range(&self, expr: &Expr) -> Result<Area<'a>, CellError> {
        match self.unparen(expr) {
            Expr::Reference { sheet, reference } => Ok(Area {
                sheet: self.resolve(sheet.as_deref())?,
                range: reference.range(),
            }),
            _ => match self.value(expr) {
                Value::Range(area) => Ok(area),
                Value::Scalar(CellValue::Error(e)) => Err(e),
                Value::Scalar(_) => Err(CellError::Value),
            },
        }
    }

    pub(super) fn unparen<'e>(&self, expr: &'e Expr) -> &'e Expr {
        match expr {
            Expr::Paren(inner) => self.unparen(inner),
            other => other,
        }
    }

    pub(super) fn scalar(&self, expr: &Expr) -> CellValue {
        match self.value(expr) {
            Value::Scalar(v) => v,
            Value::Range(area) => self.intersect(area),
        }
    }

    fn intersect(&self, area: Area) -> CellValue {
        let (start, end) = (area.range.start(), area.range.end());
        let addr = if area.range.cols() == 1 && (start.row()..=end.row()).contains(&self.at.row()) {
            CellAddr::new(self.at.row(), start.col()).ok()
        } else if area.range.rows() == 1 && (start.col()..=end.col()).contains(&self.at.col()) {
            CellAddr::new(start.row(), self.at.col()).ok()
        } else {
            None
        };
        addr.map_or(CellValue::Error(CellError::Value), |a| {
            area.sheet.value(a).clone()
        })
    }

    pub(super) fn number(&self, expr: &Expr) -> Result<f64, CellError> {
        self.to_number(&self.scalar(expr))
    }

    pub(super) fn text(&self, expr: &Expr) -> Result<String, CellError> {
        self.to_text(&self.scalar(expr))
    }

    pub(super) fn boolean(&self, expr: &Expr) -> Result<bool, CellError> {
        self.to_bool(&self.scalar(expr))
    }

    pub(super) fn to_number(&self, value: &CellValue) -> Result<f64, CellError> {
        match value {
            CellValue::Empty => Ok(0.0),
            CellValue::Number(n) => Ok(*n),
            CellValue::Bool(b) => Ok(f64::from(u8::from(*b))),
            CellValue::Text(s) => self.locale.parse_value(s.trim()).ok_or(CellError::Value),
            CellValue::Error(e) => Err(*e),
        }
    }

    pub(super) fn to_text(&self, value: &CellValue) -> Result<String, CellError> {
        match value {
            CellValue::Error(e) => Err(*e),
            other => Ok(self.locale.display(other)),
        }
    }

    pub(super) fn to_bool(&self, value: &CellValue) -> Result<bool, CellError> {
        match value {
            CellValue::Empty => Ok(false),
            CellValue::Number(n) => Ok(*n != 0.0),
            CellValue::Bool(b) => Ok(*b),
            CellValue::Text(s) => self.locale.parse_bool(s).ok_or(CellError::Value),
            CellValue::Error(e) => Err(*e),
        }
    }

    fn binary(&self, op: BinOp, left: &Expr, right: &Expr) -> CellValue {
        let a = self.scalar(left);
        let b = self.scalar(right);
        let numbers =
            || -> Result<(f64, f64), CellError> { Ok((self.to_number(&a)?, self.to_number(&b)?)) };
        match op {
            BinOp::Add => number(numbers().map(|(x, y)| x + y)),
            BinOp::Sub => number(numbers().map(|(x, y)| x - y)),
            BinOp::Mul => number(numbers().map(|(x, y)| x * y)),
            BinOp::Div => number(numbers().and_then(|(x, y)| {
                if y == 0.0 {
                    Err(CellError::Div0)
                } else {
                    Ok(x / y)
                }
            })),
            BinOp::Pow => number(numbers().and_then(|(x, y)| {
                if x == 0.0 && y == 0.0 {
                    Err(CellError::Num)
                } else if x == 0.0 && y < 0.0 {
                    Err(CellError::Div0)
                } else {
                    Ok(x.powf(y))
                }
            })),
            BinOp::Concat => match (self.to_text(&a), self.to_text(&b)) {
                (Ok(x), Ok(y)) => CellValue::Text(x + &y),
                (Err(e), _) | (_, Err(e)) => CellValue::Error(e),
            },
            _ => match compare(&a, &b) {
                Ok(ordering) => CellValue::Bool(match op {
                    BinOp::Eq => ordering == Ordering::Equal,
                    BinOp::Ne => ordering != Ordering::Equal,
                    BinOp::Lt => ordering == Ordering::Less,
                    BinOp::Le => ordering != Ordering::Greater,
                    BinOp::Gt => ordering == Ordering::Greater,
                    _ => ordering != Ordering::Less,
                }),
                Err(e) => CellValue::Error(e),
            },
        }
    }
}

pub(super) fn number(result: Result<f64, CellError>) -> CellValue {
    match result {
        Ok(n) if !n.is_finite() => CellValue::Error(CellError::Num),
        Ok(0.0) => CellValue::Number(0.0),
        Ok(n) => CellValue::Number(n),
        Err(e) => CellValue::Error(e),
    }
}

fn rank(value: &CellValue) -> u8 {
    match value {
        CellValue::Empty | CellValue::Number(_) | CellValue::Error(_) => 0,
        CellValue::Text(_) => 1,
        CellValue::Bool(_) => 2,
    }
}

pub(super) fn compare(a: &CellValue, b: &CellValue) -> Result<Ordering, CellError> {
    let blank_like = |other: &CellValue| match other {
        CellValue::Text(_) => CellValue::Text(String::new()),
        CellValue::Bool(_) => CellValue::Bool(false),
        _ => CellValue::Number(0.0),
    };
    let a = if a.is_empty() {
        blank_like(b)
    } else {
        a.clone()
    };
    let b = if b.is_empty() {
        blank_like(&a)
    } else {
        b.clone()
    };
    match (&a, &b) {
        (CellValue::Error(e), _) | (_, CellValue::Error(e)) => Err(*e),
        (CellValue::Number(x), CellValue::Number(y)) => Ok(round_significant(*x)
            .partial_cmp(&round_significant(*y))
            .unwrap_or(Ordering::Equal)),
        (CellValue::Text(x), CellValue::Text(y)) => Ok(fold(x).cmp(&fold(y))),
        (CellValue::Bool(x), CellValue::Bool(y)) => Ok(x.cmp(y)),
        _ => Ok(rank(&a).cmp(&rank(&b))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::parse;
    use crate::{EN, TR};

    fn sheet() -> Sheet {
        let mut sheet = Sheet::new("Sayfa1");
        for (addr, input) in [
            ("A1", "10"),
            ("A2", "20"),
            ("A3", "elma"),
            ("A4", "TRUE"),
            ("B1", "3"),
            ("C1", "#DIV/0!"),
        ] {
            sheet.set_input(addr.parse().unwrap(), input);
        }
        sheet
    }

    fn eval_in(src: &str, locale: &Locale) -> CellValue {
        let book = Workbook::from_sheets(vec![sheet()]).unwrap();
        let evaluator = Evaluator {
            book: &book,
            sheet: 0,
            locale,
            at: "D2".parse().unwrap(),
            today: 46000.0,
            now: 46000.5,
        };
        evaluator.evaluate(&parse(src, locale).unwrap())
    }

    fn eval(src: &str) -> CellValue {
        eval_in(src, &EN)
    }

    fn n(x: f64) -> CellValue {
        CellValue::Number(x)
    }

    fn err(e: CellError) -> CellValue {
        CellValue::Error(e)
    }

    #[test]
    fn arithmetic_rules() {
        assert_eq!(eval("1+2*3"), n(7.0));
        assert_eq!(eval("-2^2"), n(4.0));
        assert_eq!(eval("2^3^2"), n(64.0));
        assert_eq!(eval("50%*A1"), n(5.0));
        assert_eq!(eval("A1/B1*B1"), n(10.0));
        assert_eq!(eval("1/0"), err(CellError::Div0));
        assert_eq!(eval("0^0"), err(CellError::Num));
        assert_eq!(eval("(-8)^0.5"), err(CellError::Num));
        assert_eq!(eval("A1+A3"), err(CellError::Value));
        assert_eq!(eval("C1+1"), err(CellError::Div0));
        assert_eq!(eval("A4+1"), n(2.0));
        assert_eq!(eval("Z99+1"), n(1.0));
        assert_eq!(eval("\"3\"+1"), n(4.0));
    }

    #[test]
    fn empty_reference_is_zero() {
        assert_eq!(eval("Z99"), n(0.0));
    }

    #[test]
    fn comparison_rules() {
        assert_eq!(eval("0.1+0.2=0.3"), CellValue::Bool(true));
        assert_eq!(eval("\"Elma\"=A3"), CellValue::Bool(true));
        assert_eq!(eval("\"a\"<\"B\""), CellValue::Bool(true));
        assert_eq!(eval("1<\"a\""), CellValue::Bool(true));
        assert_eq!(eval("\"z\"<TRUE"), CellValue::Bool(true));
        assert_eq!(eval("Z99=0"), CellValue::Bool(true));
        assert_eq!(eval("Z99=\"\""), CellValue::Bool(true));
        assert_eq!(eval("A1<>10"), CellValue::Bool(false));
    }

    #[test]
    fn concatenation_uses_locale() {
        assert_eq!(eval("1.5&\"x\""), CellValue::Text("1.5x".into()));
        assert_eq!(eval_in("1,5&\"x\"", &TR), CellValue::Text("1,5x".into()));
        assert_eq!(eval_in("A4&\"\"", &TR), CellValue::Text("DOĞRU".into()));
        assert_eq!(eval_in("\"2,5\"*2", &TR), n(5.0));
    }

    #[test]
    fn ranges_use_implicit_intersection() {
        assert_eq!(eval("A1:A3"), n(20.0));
        assert_eq!(eval("A1:C1"), err(CellError::Value));
        assert_eq!(eval("B1:E1"), n(0.0));
        assert_eq!(eval("A1:C2"), err(CellError::Value));
    }

    #[test]
    fn other_sheets_and_names_are_errors() {
        assert_eq!(eval("Other!A1"), err(CellError::Ref));
        assert_eq!(eval("sayfa1!A1"), n(10.0));
        assert_eq!(eval("unknown"), err(CellError::Name));
        assert_eq!(eval("NOSUCH(1)"), err(CellError::Name));
    }
}
