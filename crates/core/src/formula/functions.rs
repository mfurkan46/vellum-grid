use std::cmp::Ordering;

use super::Expr;
use super::criteria::Criteria;
use super::eval::{Area, Evaluator, Value, compare, number};
use crate::{CellAddr, CellError, CellRange, CellValue, Sheet, round_decimal};

const MANY: usize = 255;

const FUNCTIONS: [(&str, usize, usize); 24] = [
    ("ABS", 1, 1),
    ("AND", 1, MANY),
    ("AVERAGE", 1, MANY),
    ("CONCAT", 1, MANY),
    ("CONCATENATE", 1, MANY),
    ("COUNT", 1, MANY),
    ("COUNTA", 1, MANY),
    ("COUNTIF", 2, 2),
    ("IF", 2, 3),
    ("IFERROR", 2, 2),
    ("LEFT", 1, 2),
    ("LEN", 1, 1),
    ("MAX", 1, MANY),
    ("MID", 3, 3),
    ("MIN", 1, MANY),
    ("NOT", 1, 1),
    ("OR", 1, MANY),
    ("RIGHT", 1, 2),
    ("ROUND", 2, 2),
    ("SUM", 1, MANY),
    ("SUMIF", 2, 3),
    ("TODAY", 0, 0),
    ("VLOOKUP", 3, 4),
    ("XLOOKUP", 3, 6),
];

pub(super) fn arity(name: &str) -> Option<(usize, usize)> {
    FUNCTIONS
        .iter()
        .chain(super::library::FUNCTIONS)
        .find(|(n, _, _)| *n == name)
        .map(|(_, min, max)| (*min, *max))
}

pub fn names() -> Vec<&'static str> {
    let mut names: Vec<&str> = FUNCTIONS
        .iter()
        .chain(super::library::FUNCTIONS)
        .map(|(name, _, _)| *name)
        .collect();
    names.sort_unstable();
    names
}

pub(super) fn call<'a>(ev: &Evaluator<'a>, name: &str, args: &[Expr]) -> Value<'a> {
    let result = match name {
        "ABS" => ev.number(&args[0]).map(|n| CellValue::Number(n.abs())),
        "AND" => logical(ev, args, true),
        "AVERAGE" => aggregate(ev, args).and_then(|(sum, count)| {
            if count == 0 {
                Err(CellError::Div0)
            } else {
                Ok(CellValue::Number(sum / count as f64))
            }
        }),
        "CONCAT" | "CONCATENATE" => concat(ev, args),
        "COUNT" => Ok(count(ev, args)),
        "COUNTA" => Ok(count_all(ev, args)),
        "COUNTIF" => count_if(ev, args),
        "IF" => return branch(ev, args),
        "IFERROR" => Ok(match ev.scalar(&args[0]) {
            CellValue::Error(_) => ev.scalar(&args[1]),
            value => value,
        }),
        "LEFT" => left_right(ev, args, true),
        "LEN" => ev
            .text(&args[0])
            .map(|s| CellValue::Number(s.chars().count() as f64)),
        "MAX" => extreme(ev, args, f64::max),
        "MID" => mid(ev, args),
        "MIN" => extreme(ev, args, f64::min),
        "NOT" => ev.boolean(&args[0]).map(|b| CellValue::Bool(!b)),
        "OR" => logical(ev, args, false),
        "RIGHT" => left_right(ev, args, false),
        "ROUND" => ev.number(&args[0]).and_then(|n| {
            let places = ev.number(&args[1])?.trunc() as i32;
            Ok(CellValue::Number(round_decimal(n, places)))
        }),
        "SUM" => aggregate(ev, args).map(|(sum, _)| CellValue::Number(sum)),
        "SUMIF" => sum_if(ev, args),
        "TODAY" => Ok(CellValue::Number(ev.today)),
        "VLOOKUP" => vlookup(ev, args),
        "XLOOKUP" => xlookup(ev, args),
        _ => match super::library::call(ev, name, args) {
            Some(value) => return value,
            None => Err(CellError::Name),
        },
    };
    Value::Scalar(match result {
        Ok(CellValue::Number(n)) => number(Ok(n)),
        Ok(value) => value,
        Err(e) => CellValue::Error(e),
    })
}

fn each(
    ev: &Evaluator,
    args: &[Expr],
    mut f: impl FnMut(&CellValue, bool) -> Result<(), CellError>,
) -> Result<(), CellError> {
    for arg in args {
        let from_reference = matches!(ev.unparen(arg), Expr::Reference { .. });
        match ev.value(arg) {
            Value::Range(area) => {
                for (_, cell) in area.sheet.cells_in(area.range) {
                    f(&cell.value, true)?;
                }
            }
            Value::Scalar(value) => f(&value, from_reference)?,
        }
    }
    Ok(())
}

pub(super) fn numbers(
    ev: &Evaluator,
    args: &[Expr],
    mut f: impl FnMut(f64),
) -> Result<(), CellError> {
    each(ev, args, |value, from_reference| {
        match value {
            CellValue::Number(n) => f(*n),
            CellValue::Error(e) => return Err(*e),
            _ if from_reference => {}
            other => f(ev.to_number(other)?),
        }
        Ok(())
    })
}

fn aggregate(ev: &Evaluator, args: &[Expr]) -> Result<(f64, usize), CellError> {
    let (mut sum, mut count) = (0.0, 0);
    numbers(ev, args, |n| {
        sum += n;
        count += 1;
    })?;
    Ok((sum, count))
}

fn extreme(
    ev: &Evaluator,
    args: &[Expr],
    pick: fn(f64, f64) -> f64,
) -> Result<CellValue, CellError> {
    let mut best: Option<f64> = None;
    numbers(ev, args, |n| best = Some(best.map_or(n, |b| pick(b, n))))?;
    Ok(CellValue::Number(best.unwrap_or(0.0)))
}

fn count(ev: &Evaluator, args: &[Expr]) -> CellValue {
    let mut total = 0;
    let _ = each(ev, args, |value, from_reference| {
        let counts = match value {
            CellValue::Number(_) => true,
            CellValue::Error(_) | CellValue::Empty => false,
            other => !from_reference && ev.to_number(other).is_ok(),
        };
        total += usize::from(counts);
        Ok(())
    });
    CellValue::Number(total as f64)
}

fn count_all(ev: &Evaluator, args: &[Expr]) -> CellValue {
    let mut total = 0;
    let _ = each(ev, args, |value, _| {
        total += usize::from(!value.is_empty());
        Ok(())
    });
    let missing = args.iter().filter(|a| matches!(a, Expr::Missing)).count();
    CellValue::Number((total + missing) as f64)
}

fn logical(ev: &Evaluator, args: &[Expr], all: bool) -> Result<CellValue, CellError> {
    let mut seen = false;
    let mut result = all;
    each(ev, args, |value, from_reference| {
        let b = match value {
            CellValue::Bool(b) => *b,
            CellValue::Number(n) => *n != 0.0,
            CellValue::Error(e) => return Err(*e),
            _ if from_reference => return Ok(()),
            other => ev.to_bool(other)?,
        };
        seen = true;
        result = if all { result && b } else { result || b };
        Ok(())
    })?;
    if seen {
        Ok(CellValue::Bool(result))
    } else {
        Err(CellError::Value)
    }
}

fn branch<'a>(ev: &Evaluator<'a>, args: &[Expr]) -> Value<'a> {
    match ev.boolean(&args[0]) {
        Ok(true) => ev.value(&args[1]),
        Ok(false) => match args.get(2) {
            Some(arg) => ev.value(arg),
            None => Value::Scalar(CellValue::Bool(false)),
        },
        Err(e) => Value::Scalar(CellValue::Error(e)),
    }
}

fn concat(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let mut out = String::new();
    each(ev, args, |value, _| {
        out.push_str(&ev.to_text(value)?);
        Ok(())
    })?;
    Ok(CellValue::Text(out))
}

fn count_arg(ev: &Evaluator, arg: Option<&Expr>, default: f64) -> Result<usize, CellError> {
    let n = match arg {
        Some(arg) => ev.number(arg)?.trunc(),
        None => default,
    };
    if n < 0.0 {
        Err(CellError::Value)
    } else {
        Ok(n as usize)
    }
}

fn left_right(ev: &Evaluator, args: &[Expr], left: bool) -> Result<CellValue, CellError> {
    let text: Vec<char> = ev.text(&args[0])?.chars().collect();
    let n = count_arg(ev, args.get(1), 1.0)?.min(text.len());
    let part = if left {
        &text[..n]
    } else {
        &text[text.len() - n..]
    };
    Ok(CellValue::Text(part.iter().collect()))
}

fn mid(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let text = ev.text(&args[0])?;
    let start = ev.number(&args[1])?.trunc();
    if start < 1.0 {
        return Err(CellError::Value);
    }
    let len = count_arg(ev, args.get(2), 0.0)?;
    Ok(CellValue::Text(
        text.chars().skip(start as usize - 1).take(len).collect(),
    ))
}

fn at(sheet: &Sheet, row: u32, col: u32) -> CellValue {
    match CellAddr::new(row, col).map(|addr| sheet.value(addr)) {
        Ok(CellValue::Empty) => CellValue::Number(0.0),
        Ok(value) => value.clone(),
        Err(_) => CellValue::Error(CellError::Ref),
    }
}

fn lookup_target(ev: &Evaluator, arg: &Expr) -> Result<CellValue, CellError> {
    match ev.scalar(arg) {
        CellValue::Error(e) => Err(e),
        value => Ok(value),
    }
}

fn same_kind(a: &CellValue, b: &CellValue) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn exact(ev: &Evaluator, target: &CellValue, value: &CellValue) -> bool {
    match target {
        CellValue::Text(_) => Criteria::new(target, ev.locale).matches(value) && !value.is_empty(),
        _ => same_kind(target, value) && compare(target, value) == Ok(Ordering::Equal),
    }
}

fn vlookup(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let target = lookup_target(ev, &args[0])?;
    let Area {
        sheet,
        range: table,
    } = ev.range(&args[1])?;
    let col = ev.number(&args[2])?.trunc();
    if col < 1.0 {
        return Err(CellError::Value);
    }
    if col > f64::from(table.cols()) {
        return Err(CellError::Ref);
    }
    let approximate = match args.get(3) {
        Some(Expr::Missing) | None => true,
        Some(arg) => ev.boolean(arg)?,
    };
    let first = CellRange::new(
        table.start(),
        table.start().offset(i64::from(table.rows()) - 1, 0),
    );
    let mut found = None;
    for (addr, cell) in sheet.cells_in(first) {
        let value = &cell.value;
        if approximate {
            if !same_kind(&target, value) {
                continue;
            }
            match compare(value, &target) {
                Ok(Ordering::Greater) => break,
                Ok(_) => found = Some(addr.row()),
                Err(_) => {}
            }
        } else if exact(ev, &target, value) {
            found = Some(addr.row());
            break;
        }
    }
    let row = found.ok_or(CellError::NA)?;
    Ok(at(sheet, row, table.start().col() + col as u32 - 1))
}

fn vector(range: CellRange) -> Result<bool, CellError> {
    if range.cols() == 1 {
        Ok(true)
    } else if range.rows() == 1 {
        Ok(false)
    } else {
        Err(CellError::Value)
    }
}

fn xlookup(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let target = lookup_target(ev, &args[0])?;
    let Area {
        sheet: lookup_sheet,
        range: lookup,
    } = ev.range(&args[1])?;
    let Area {
        sheet: result_sheet,
        range: result,
    } = ev.range(&args[2])?;
    let vertical = vector(lookup)?;
    let length = if vertical {
        lookup.rows()
    } else {
        lookup.cols()
    };
    let optional = |i: usize| args.get(i).filter(|a| !matches!(a, Expr::Missing));
    let mode = optional(4).map_or(Ok(0.0), |a| ev.number(a))?;
    let search = optional(5).map_or(Ok(1.0), |a| ev.number(a))?;

    let mut cells: Vec<(u32, &CellValue)> = lookup_sheet
        .cells_in(lookup)
        .map(|(addr, cell)| {
            let index = if vertical {
                addr.row() - lookup.start().row()
            } else {
                addr.col() - lookup.start().col()
            };
            (index, &cell.value)
        })
        .collect();
    if search < 0.0 {
        cells.reverse();
    }

    let exact_match = cells.iter().find(|(_, v)| match mode as i32 {
        2 => exact(ev, &target, v),
        _ => same_kind(&target, v) && compare(v, &target) == Ok(Ordering::Equal),
    });
    let index = match (exact_match, mode as i32) {
        (Some((i, _)), _) => Some(*i),
        (None, m @ (-1 | 1)) => {
            let wanted = if m < 0 {
                Ordering::Less
            } else {
                Ordering::Greater
            };
            cells
                .iter()
                .filter(|(_, v)| same_kind(&target, v) && compare(v, &target) == Ok(wanted))
                .reduce(|best, candidate| {
                    if compare(candidate.1, best.1) == Ok(wanted.reverse()) {
                        candidate
                    } else {
                        best
                    }
                })
                .map(|(i, _)| *i)
        }
        _ => None,
    };
    let Some(index) = index else {
        return match optional(3) {
            Some(arg) => Ok(ev.scalar(arg)),
            None => Err(CellError::NA),
        };
    };
    let result_length = if vertical {
        result.rows()
    } else {
        result.cols()
    };
    if result_length != length {
        return Err(CellError::Value);
    }
    let start = result.start();
    Ok(if vertical {
        at(result_sheet, start.row() + index, start.col())
    } else {
        at(result_sheet, start.row(), start.col() + index)
    })
}

fn criteria_args<'a>(
    ev: &Evaluator<'a>,
    range: &Expr,
    criteria: &Expr,
) -> Result<(Area<'a>, Criteria), CellError> {
    let range = ev.range(range)?;
    let criteria = match ev.scalar(criteria) {
        CellValue::Error(e) => return Err(e),
        value => Criteria::new(&value, ev.locale),
    };
    Ok((range, criteria))
}

fn count_if(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let (Area { sheet, range }, criteria) = criteria_args(ev, &args[0], &args[1])?;
    let mut filled = 0u64;
    let mut hits = 0u64;
    for (_, cell) in sheet.cells_in(range) {
        filled += 1;
        hits += u64::from(criteria.matches(&cell.value));
    }
    if criteria.matches(&CellValue::Empty) {
        hits += u64::from(range.rows()) * u64::from(range.cols()) - filled;
    }
    Ok(CellValue::Number(hits as f64))
}

fn sum_if(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let (Area { sheet, range }, criteria) = criteria_args(ev, &args[0], &args[1])?;
    let (sum_sheet, sum_start) = match args.get(2).filter(|a| !matches!(a, Expr::Missing)) {
        Some(arg) => {
            let area = ev.range(arg)?;
            (area.sheet, area.range.start())
        }
        None => (sheet, range.start()),
    };
    let row_offset = i64::from(sum_start.row()) - i64::from(range.start().row());
    let col_offset = i64::from(sum_start.col()) - i64::from(range.start().col());
    let sum_range = CellRange::new(sum_start, range.end().offset(row_offset, col_offset));
    let mut sum = 0.0;
    let mut add = |value: &CellValue| -> Result<(), CellError> {
        match value {
            CellValue::Number(n) => sum += n,
            CellValue::Error(e) => return Err(*e),
            _ => {}
        }
        Ok(())
    };
    for (addr, cell) in sheet.cells_in(range) {
        if criteria.matches(&cell.value) {
            add(sum_sheet.value(addr.offset(row_offset, col_offset)))?;
        }
    }
    if criteria.matches(&CellValue::Empty) {
        for (addr, cell) in sum_sheet.cells_in(sum_range) {
            let source = addr.offset(-row_offset, -col_offset);
            if sheet.get(source).is_none() {
                add(&cell.value)?;
            }
        }
    }
    Ok(CellValue::Number(sum))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::parse;
    use crate::{EN, Locale, TR, Workbook};

    fn sheet() -> Sheet {
        let mut sheet = Sheet::new("S");
        let rows = [
            ["Ürün", "Adet", "Fiyat", "Aktif"],
            ["Elma", "10", "2.5", "TRUE"],
            ["Armut", "", "4", "FALSE"],
            ["Kiraz", "5", "12", "TRUE"],
            ["elmas", "x", "100", ""],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                let addr = CellAddr::new(r as u32, c as u32).unwrap();
                sheet.set_input(addr, value);
            }
        }
        sheet.set_input("F1".parse().unwrap(), "#N/A");
        sheet
    }

    fn eval_in(src: &str, locale: &Locale) -> CellValue {
        let book = Workbook::from_sheets(vec![sheet()]).unwrap();
        Evaluator {
            book: &book,
            sheet: 0,
            locale,
            at: "H1".parse().unwrap(),
            today: 46000.0,
            now: 46000.5,
        }
        .evaluate(&parse(src, locale).unwrap())
    }

    fn eval(src: &str) -> CellValue {
        eval_in(src, &EN)
    }

    fn n(x: f64) -> CellValue {
        CellValue::Number(x)
    }

    fn t(s: &str) -> CellValue {
        CellValue::Text(s.into())
    }

    fn err(e: CellError) -> CellValue {
        CellValue::Error(e)
    }

    #[test]
    fn aggregates_ignore_text_in_references() {
        assert_eq!(eval("SUM(B2:B5)"), n(15.0));
        assert_eq!(eval("SUM(B:B)"), n(15.0));
        assert_eq!(eval("SUM(B2,C2,1,\"2\",TRUE)"), n(16.5));
        assert_eq!(eval("SUM(B5)"), n(0.0));
        assert_eq!(eval("SUM(\"x\")"), err(CellError::Value));
        assert_eq!(eval("SUM(A1:F1)"), err(CellError::NA));
        assert_eq!(eval("AVERAGE(B2:B5)"), n(7.5));
        assert_eq!(eval("AVERAGE(A1:A5)"), err(CellError::Div0));
        assert_eq!(eval("MIN(C2:C5)"), n(2.5));
        assert_eq!(eval("MAX(C2:C5,500)"), n(500.0));
        assert_eq!(eval("MAX(A1:A5)"), n(0.0));
    }

    #[test]
    fn counting() {
        assert_eq!(eval("COUNT(A1:D5)"), n(6.0));
        assert_eq!(eval("COUNT(1,\"2\",\"x\")"), n(2.0));
        assert_eq!(eval("COUNTA(A1:D5)"), n(18.0));
        assert_eq!(eval("COUNTIF(A2:A5,\"el*\")"), n(2.0));
        assert_eq!(eval("COUNTIF(B2:B5,\">=5\")"), n(2.0));
        assert_eq!(eval("COUNTIF(B2:B5,\"\")"), n(1.0));
        assert_eq!(eval("COUNTIF(B1:B1000,\"\")"), n(996.0));
        assert_eq!(eval_in("EĞERSAY(D2:D5;DOĞRU)", &TR), n(2.0));
    }

    #[test]
    fn conditional_sums() {
        assert_eq!(eval("SUMIF(B2:B5,\">5\",C2:C5)"), n(2.5));
        assert_eq!(eval("SUMIF(A2:A5,\"elma\",C2:C5)"), n(2.5));
        assert_eq!(eval("SUMIF(C2:C5,\"<10\")"), n(6.5));
        assert_eq!(eval("SUMIF(B2:B5,\"\",C2)"), n(4.0));
        assert_eq!(eval_in("ETOPLA(D2:D5;DOĞRU;C2:C5)", &TR), n(14.5));
    }

    #[test]
    fn logic() {
        assert_eq!(eval("IF(B2>5,\"çok\",\"az\")"), t("çok"));
        assert_eq!(eval("IF(B4>5,\"çok\")"), CellValue::Bool(false));
        assert_eq!(eval("IF(B4>5,1,)"), n(0.0));
        assert_eq!(eval("IF(A2,1,2)"), err(CellError::Value));
        assert_eq!(eval("SUM(IF(TRUE,B2:B5,0))"), n(15.0));
        assert_eq!(eval("AND(D2,D4)"), CellValue::Bool(true));
        assert_eq!(eval("AND(D2:D5)"), CellValue::Bool(false));
        assert_eq!(eval("OR(D3,0)"), CellValue::Bool(false));
        assert_eq!(eval("AND(A1:A2)"), err(CellError::Value));
        assert_eq!(eval("NOT(0)"), CellValue::Bool(true));
        assert_eq!(eval("IFERROR(1/0,\"yok\")"), t("yok"));
        assert_eq!(eval("IFERROR(5,0)"), n(5.0));
        assert_eq!(
            eval_in("EĞER(VE(DOĞRU;1);\"evet\";\"hayır\")", &TR),
            t("evet")
        );
    }

    #[test]
    fn math() {
        assert_eq!(eval("ROUND(2.675,2)"), n(2.68));
        assert_eq!(eval("ROUND(-2.5,0)"), n(-3.0));
        assert_eq!(eval("ROUND(1234.5,-2)"), n(1200.0));
        assert_eq!(eval("ROUND(0.004,2)"), n(0.0));
        assert_eq!(eval("ABS(-3)"), n(3.0));
        assert_eq!(eval_in("YUVARLA(1,005;2)", &TR), n(1.01));
    }

    #[test]
    fn text_functions() {
        assert_eq!(eval("LEN(A2)"), n(4.0));
        assert_eq!(eval("LEN(\"İğne\")"), n(4.0));
        assert_eq!(eval("LEFT(A2)"), t("E"));
        assert_eq!(eval("LEFT(A2,10)"), t("Elma"));
        assert_eq!(eval("RIGHT(A4,3)"), t("raz"));
        assert_eq!(eval("MID(\"Şeftali\",2,3)"), t("eft"));
        assert_eq!(eval("MID(\"abc\",0,1)"), err(CellError::Value));
        assert_eq!(eval("LEFT(\"abc\",-1)"), err(CellError::Value));
        assert_eq!(eval("CONCAT(A2:A3,\"-\",B2)"), t("ElmaArmut-10"));
        assert_eq!(eval_in("BİRLEŞTİR(\"a\";1,5;D2)", &TR), t("a1,5DOĞRU"));
    }

    #[test]
    fn today_comes_from_context() {
        assert_eq!(eval("TODAY()"), n(46000.0));
        assert_eq!(eval_in("BUGÜN()+1", &TR), n(46001.0));
    }

    #[test]
    fn vertical_lookup() {
        assert_eq!(eval("VLOOKUP(\"kiraz\",A2:C5,3,FALSE)"), n(12.0));
        assert_eq!(eval("VLOOKUP(\"arm*\",A2:C5,2,FALSE)"), n(0.0));
        assert_eq!(eval("VLOOKUP(\"yok\",A2:C5,2,FALSE)"), err(CellError::NA));
        assert_eq!(eval("VLOOKUP(\"Elma\",A2:C5,4,FALSE)"), err(CellError::Ref));
        assert_eq!(
            eval("VLOOKUP(\"Elma\",A2:C5,0,FALSE)"),
            err(CellError::Value)
        );
        assert_eq!(eval("VLOOKUP(7,C2:D5,2)"), CellValue::Bool(false));
        assert_eq!(eval("VLOOKUP(1,C2:D5,2)"), err(CellError::NA));
        assert_eq!(eval_in("DÜŞEYARA(\"Elma\";A:D;3;YANLIŞ)", &TR), n(2.5));
    }

    #[test]
    fn cross_lookup() {
        assert_eq!(eval("XLOOKUP(\"Kiraz\",A2:A5,C2:C5)"), n(12.0));
        assert_eq!(eval("XLOOKUP(\"yok\",A2:A5,C2:C5,\"-\")"), t("-"));
        assert_eq!(eval("XLOOKUP(\"yok\",A2:A5,C2:C5)"), err(CellError::NA));
        assert_eq!(eval("XLOOKUP(5,C2:C5,A2:A5,,-1)"), t("Armut"));
        assert_eq!(eval("XLOOKUP(5,C2:C5,A2:A5,,1)"), t("Kiraz"));
        assert_eq!(eval("XLOOKUP(\"el*\",A2:A5,C2:C5,,2,-1)"), n(100.0));
        assert_eq!(eval("XLOOKUP(\"Fiyat\",A1:D1,A4:D4)"), n(12.0));
        assert_eq!(eval("XLOOKUP(1,A1:B2,A1:A2)"), err(CellError::Value));
        assert_eq!(eval("XLOOKUP(\"Elma\",A2:A5,C2:C4)"), err(CellError::Value));
        assert_eq!(eval_in("ÇAPRAZARA(\"Armut\";A2:A5;C2:C5)", &TR), n(4.0));
    }
}
