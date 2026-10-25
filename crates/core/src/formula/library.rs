use std::cmp::Ordering;

use super::Expr;
use super::criteria::Criteria;
use super::eval::{Area, Evaluator, Value, compare};
use super::functions::numbers;
use crate::date::{date_serial, from_serial};
use crate::value::round_significant;
use crate::{CellAddr, CellError, CellRange, CellValue, MAX_COLS, MAX_ROWS};

const MANY: usize = 255;
const SECONDS_PER_DAY: f64 = 86_400.0;

pub(super) const FUNCTIONS: &[(&str, usize, usize)] = &[
    ("AVERAGEIF", 2, 3),
    ("AVERAGEIFS", 3, MANY),
    ("CEILING", 2, 2),
    ("CHAR", 1, 1),
    ("CHOOSE", 2, MANY),
    ("CLEAN", 1, 1),
    ("CODE", 1, 1),
    ("COLUMN", 0, 1),
    ("COLUMNS", 1, 1),
    ("COUNTBLANK", 1, 1),
    ("COUNTIFS", 2, MANY),
    ("DATE", 3, 3),
    ("DATEDIF", 3, 3),
    ("DATEVALUE", 1, 1),
    ("DAY", 1, 1),
    ("DAYS", 2, 2),
    ("EDATE", 2, 2),
    ("EOMONTH", 2, 2),
    ("EXACT", 2, 2),
    ("EXP", 1, 1),
    ("FALSE", 0, 0),
    ("FIND", 2, 3),
    ("FLOOR", 2, 2),
    ("HLOOKUP", 3, 4),
    ("HOUR", 1, 1),
    ("IFNA", 2, 2),
    ("IFS", 2, MANY),
    ("INDEX", 2, 3),
    ("INDIRECT", 1, 2),
    ("INT", 1, 1),
    ("ISBLANK", 1, 1),
    ("ISERR", 1, 1),
    ("ISERROR", 1, 1),
    ("ISLOGICAL", 1, 1),
    ("ISNA", 1, 1),
    ("ISNUMBER", 1, 1),
    ("ISTEXT", 1, 1),
    ("LARGE", 2, 2),
    ("LN", 1, 1),
    ("LOG", 1, 2),
    ("LOG10", 1, 1),
    ("LOWER", 1, 1),
    ("MATCH", 2, 3),
    ("MAXIFS", 3, MANY),
    ("MEDIAN", 1, MANY),
    ("MINIFS", 3, MANY),
    ("MINUTE", 1, 1),
    ("MOD", 2, 2),
    ("MONTH", 1, 1),
    ("MROUND", 2, 2),
    ("NA", 0, 0),
    ("NETWORKDAYS", 2, 3),
    ("NOW", 0, 0),
    ("OFFSET", 3, 5),
    ("PI", 0, 0),
    ("POWER", 2, 2),
    ("PRODUCT", 1, MANY),
    ("PROPER", 1, 1),
    ("RAND", 0, 0),
    ("RANDBETWEEN", 2, 2),
    ("RANK", 2, 3),
    ("REPLACE", 4, 4),
    ("REPT", 2, 2),
    ("ROUNDDOWN", 2, 2),
    ("ROUNDUP", 2, 2),
    ("ROW", 0, 1),
    ("ROWS", 1, 1),
    ("SEARCH", 2, 3),
    ("SECOND", 1, 1),
    ("SIGN", 1, 1),
    ("SMALL", 2, 2),
    ("SQRT", 1, 1),
    ("STDEV", 1, MANY),
    ("SUBSTITUTE", 3, 4),
    ("SUBTOTAL", 2, MANY),
    ("SUMIFS", 3, MANY),
    ("SUMPRODUCT", 1, MANY),
    ("SWITCH", 3, MANY),
    ("TEXT", 2, 2),
    ("TEXTJOIN", 3, MANY),
    ("TIME", 3, 3),
    ("TRIM", 1, 1),
    ("TRUE", 0, 0),
    ("TRUNC", 1, 2),
    ("UPPER", 1, 1),
    ("VALUE", 1, 1),
    ("WEEKDAY", 1, 2),
    ("WEEKNUM", 1, 2),
    ("WORKDAY", 2, 3),
    ("XOR", 1, MANY),
    ("YEAR", 1, 1),
];

pub(super) const VOLATILE: [&str; 6] =
    ["TODAY", "NOW", "RAND", "RANDBETWEEN", "OFFSET", "INDIRECT"];

pub(super) fn call<'a>(ev: &Evaluator<'a>, name: &str, args: &[Expr]) -> Option<Value<'a>> {
    let lazy = match name {
        "CHOOSE" => Some(choose(ev, args)),
        "IFS" => Some(ifs(ev, args)),
        "SWITCH" => Some(switch(ev, args)),
        "IFNA" => Some(match ev.scalar(&args[0]) {
            CellValue::Error(CellError::NA) => ev.value(&args[1]),
            value => Value::Scalar(value),
        }),
        "INDEX" => Some(index(ev, args)),
        "OFFSET" => Some(offset(ev, args)),
        "INDIRECT" => Some(indirect(ev, args)),
        _ => None,
    };
    if lazy.is_some() {
        return lazy;
    }
    let result = match name {
        "TRUE" => Ok(CellValue::Bool(true)),
        "FALSE" => Ok(CellValue::Bool(false)),
        "XOR" => xor(ev, args),
        "INT" => num(ev, &args[0], f64::floor),
        "MOD" => modulo(ev, args),
        "POWER" => power(ev, args),
        "SQRT" => ev
            .number(&args[0])
            .and_then(|n| positive(n, true).map(f64::sqrt))
            .map(CellValue::Number),
        "ROUNDUP" => rounded(ev, &args[0], args.get(1), true),
        "ROUNDDOWN" | "TRUNC" => rounded(ev, &args[0], args.get(1), false),
        "PRODUCT" => product(ev, args),
        "SUMPRODUCT" => sum_product(ev, args),
        "SUBTOTAL" => subtotal(ev, args),
        "CEILING" => to_multiple(ev, args, f64::ceil),
        "FLOOR" => to_multiple(ev, args, f64::floor),
        "MROUND" => to_multiple(ev, args, f64::round),
        "SIGN" => num(ev, &args[0], |n| if n == 0.0 { 0.0 } else { n.signum() }),
        "PI" => Ok(CellValue::Number(std::f64::consts::PI)),
        "RAND" => Ok(CellValue::Number(random(ev))),
        "RANDBETWEEN" => random_between(ev, args),
        "EXP" => num(ev, &args[0], f64::exp),
        "LN" => log(ev, &args[0], None),
        "LOG10" => log(ev, &args[0], Some(10.0)),
        "LOG" => match args.get(1).filter(|a| !matches!(a, Expr::Missing)) {
            Some(base) => ev.number(base).and_then(|b| log(ev, &args[0], Some(b))),
            None => log(ev, &args[0], Some(10.0)),
        },
        "SUMIFS" => multi_criteria(ev, Some(&args[0]), &args[1..], Fold::Sum),
        "COUNTIFS" => multi_criteria(ev, None, args, Fold::Count),
        "AVERAGEIFS" => multi_criteria(ev, Some(&args[0]), &args[1..], Fold::Average),
        "MAXIFS" => multi_criteria(ev, Some(&args[0]), &args[1..], Fold::Max),
        "MINIFS" => multi_criteria(ev, Some(&args[0]), &args[1..], Fold::Min),
        "AVERAGEIF" => {
            let target = args.get(2).filter(|a| !matches!(a, Expr::Missing));
            let pairs = [args[0].clone(), args[1].clone()];
            multi_criteria(ev, target.or(Some(&args[0])), &pairs, Fold::Average)
        }
        "COUNTBLANK" => count_blank(ev, &args[0]),
        "MEDIAN" => sorted(ev, args).and_then(|v| median(&v)),
        "LARGE" => kth(ev, args, true),
        "SMALL" => kth(ev, args, false),
        "RANK" => rank(ev, args),
        "STDEV" => stdev(ev, args),
        "TEXT" => text_format(ev, args),
        "VALUE" => value_of(ev, &args[0]),
        "TEXTJOIN" => text_join(ev, args),
        "TRIM" => ev.text(&args[0]).map(|s| {
            CellValue::Text(
                s.split(' ')
                    .filter(|w| !w.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        }),
        "UPPER" => ev
            .text(&args[0])
            .map(|s| CellValue::Text(ev.locale.upper(&s))),
        "LOWER" => ev
            .text(&args[0])
            .map(|s| CellValue::Text(ev.locale.lower(&s))),
        "PROPER" => ev.text(&args[0]).map(|s| CellValue::Text(proper(ev, &s))),
        "SUBSTITUTE" => substitute(ev, args),
        "REPLACE" => replace(ev, args),
        "FIND" => find(ev, args, false),
        "SEARCH" => find(ev, args, true),
        "REPT" => repeat(ev, args),
        "EXACT" => exact_text(ev, args),
        "CHAR" => ev.number(&args[0]).and_then(|n| {
            let code = n.trunc();
            if !(1.0..=255.0).contains(&code) {
                return Err(CellError::Value);
            }
            char::from_u32(code as u32)
                .map(|c| CellValue::Text(c.to_string()))
                .ok_or(CellError::Value)
        }),
        "CODE" => ev.text(&args[0]).and_then(|s| {
            s.chars()
                .next()
                .map(|c| CellValue::Number(f64::from(u32::from(c))))
                .ok_or(CellError::Value)
        }),
        "CLEAN" => ev
            .text(&args[0])
            .map(|s| CellValue::Text(s.chars().filter(|c| !c.is_control()).collect())),
        "DATE" => date(ev, args),
        "NOW" => Ok(CellValue::Number(ev.now)),
        "YEAR" => part(ev, &args[0], |d| f64::from(d.year as i32)),
        "MONTH" => part(ev, &args[0], |d| f64::from(d.month)),
        "DAY" => part(ev, &args[0], |d| f64::from(d.day)),
        "HOUR" => part(ev, &args[0], |d| f64::from(d.hour)),
        "MINUTE" => part(ev, &args[0], |d| f64::from(d.minute)),
        "SECOND" => part(ev, &args[0], |d| f64::from(d.second)),
        "WEEKDAY" => weekday(ev, args),
        "WEEKNUM" => week_number(ev, args),
        "EDATE" => shift_months(ev, args, false),
        "EOMONTH" => shift_months(ev, args, true),
        "NETWORKDAYS" => network_days(ev, args),
        "WORKDAY" => workday(ev, args),
        "DATEDIF" => date_dif(ev, args),
        "DAYS" => days_between(ev, args),
        "TIME" => time(ev, args),
        "DATEVALUE" => ev.text(&args[0]).and_then(|s| {
            ev.locale
                .parse_value(s.trim())
                .map(|n| CellValue::Number(n.floor()))
                .ok_or(CellError::Value)
        }),
        "HLOOKUP" => hlookup(ev, args),
        "MATCH" => match_position(ev, args),
        "ROW" => position(ev, args.first(), true),
        "COLUMN" => position(ev, args.first(), false),
        "ROWS" => ev
            .range(&args[0])
            .map(|a| CellValue::Number(f64::from(a.range.rows()))),
        "COLUMNS" => ev
            .range(&args[0])
            .map(|a| CellValue::Number(f64::from(a.range.cols()))),
        "ISBLANK" => Ok(CellValue::Bool(ev.scalar(&args[0]).is_empty())),
        "ISNUMBER" => Ok(CellValue::Bool(matches!(
            ev.scalar(&args[0]),
            CellValue::Number(_)
        ))),
        "ISTEXT" => Ok(CellValue::Bool(matches!(
            ev.scalar(&args[0]),
            CellValue::Text(_)
        ))),
        "ISLOGICAL" => Ok(CellValue::Bool(matches!(
            ev.scalar(&args[0]),
            CellValue::Bool(_)
        ))),
        "ISERROR" => Ok(CellValue::Bool(matches!(
            ev.scalar(&args[0]),
            CellValue::Error(_)
        ))),
        "ISERR" => Ok(CellValue::Bool(matches!(
            ev.scalar(&args[0]),
            CellValue::Error(e) if e != CellError::NA
        ))),
        "ISNA" => Ok(CellValue::Bool(matches!(
            ev.scalar(&args[0]),
            CellValue::Error(CellError::NA)
        ))),
        "NA" => Err(CellError::NA),
        _ => return None,
    };
    Some(Value::Scalar(match result {
        Ok(value) => value,
        Err(e) => CellValue::Error(e),
    }))
}

fn given(arg: Option<&Expr>) -> Option<&Expr> {
    arg.filter(|a| !matches!(a, Expr::Missing))
}

fn num(ev: &Evaluator, arg: &Expr, f: impl Fn(f64) -> f64) -> Result<CellValue, CellError> {
    ev.number(arg).map(|n| CellValue::Number(f(n)))
}

fn positive(n: f64, allow_zero: bool) -> Result<f64, CellError> {
    if n > 0.0 || (allow_zero && n == 0.0) {
        Ok(n)
    } else {
        Err(CellError::Num)
    }
}

fn choose<'a>(ev: &Evaluator<'a>, args: &[Expr]) -> Value<'a> {
    match ev.number(&args[0]) {
        Ok(i) if i >= 1.0 && (i as usize) < args.len() => ev.value(&args[i as usize]),
        Ok(_) => Value::Scalar(CellValue::Error(CellError::Value)),
        Err(e) => Value::Scalar(CellValue::Error(e)),
    }
}

fn ifs<'a>(ev: &Evaluator<'a>, args: &[Expr]) -> Value<'a> {
    for pair in args.chunks(2) {
        let [condition, value] = pair else {
            return Value::Scalar(CellValue::Error(CellError::NA));
        };
        match ev.boolean(condition) {
            Ok(true) => return ev.value(value),
            Ok(false) => {}
            Err(e) => return Value::Scalar(CellValue::Error(e)),
        }
    }
    Value::Scalar(CellValue::Error(CellError::NA))
}

fn switch<'a>(ev: &Evaluator<'a>, args: &[Expr]) -> Value<'a> {
    let subject = ev.scalar(&args[0]);
    if let CellValue::Error(e) = subject {
        return Value::Scalar(CellValue::Error(e));
    }
    let rest = &args[1..];
    for pair in rest.chunks(2) {
        match pair {
            [case, result] => {
                if compare(&subject, &ev.scalar(case)) == Ok(Ordering::Equal) {
                    return ev.value(result);
                }
            }
            [default] => return ev.value(default),
            _ => {}
        }
    }
    Value::Scalar(CellValue::Error(CellError::NA))
}

fn xor(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let mut trues = 0;
    for arg in args {
        match ev.value(arg) {
            Value::Range(area) => {
                for (_, cell) in area.sheet.cells_in(area.range) {
                    match &cell.value {
                        CellValue::Bool(b) => trues += usize::from(*b),
                        CellValue::Number(n) => trues += usize::from(*n != 0.0),
                        CellValue::Error(e) => return Err(*e),
                        _ => {}
                    }
                }
            }
            Value::Scalar(value) => trues += usize::from(ev.to_bool(&value)?),
        }
    }
    Ok(CellValue::Bool(trues % 2 == 1))
}

fn modulo(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let n = ev.number(&args[0])?;
    let d = ev.number(&args[1])?;
    if d == 0.0 {
        return Err(CellError::Div0);
    }
    Ok(CellValue::Number(round_significant(
        n - d * (n / d).floor(),
    )))
}

fn power(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let x = ev.number(&args[0])?;
    let y = ev.number(&args[1])?;
    if x == 0.0 && y == 0.0 {
        return Err(CellError::Num);
    }
    if x == 0.0 && y < 0.0 {
        return Err(CellError::Div0);
    }
    Ok(CellValue::Number(x.powf(y)))
}

fn rounded(
    ev: &Evaluator,
    value: &Expr,
    digits: Option<&Expr>,
    up: bool,
) -> Result<CellValue, CellError> {
    let n = ev.number(value)?;
    let places = match given(digits) {
        Some(arg) => ev.number(arg)?.trunc() as i32,
        None => 0,
    };
    let factor = 10f64.powi(places);
    let scaled = round_significant(n.abs() * factor);
    let whole = if up { scaled.ceil() } else { scaled.floor() };
    Ok(CellValue::Number(n.signum() * whole / factor))
}

fn product(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let mut result = 1.0;
    let mut seen = false;
    numbers(ev, args, |n| {
        result *= n;
        seen = true;
    })?;
    Ok(CellValue::Number(if seen { result } else { 0.0 }))
}

fn sum_product(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let areas: Vec<Area> = args.iter().map(|a| ev.range(a)).collect::<Result<_, _>>()?;
    let (rows, cols) = (areas[0].range.rows(), areas[0].range.cols());
    if areas
        .iter()
        .any(|a| a.range.rows() != rows || a.range.cols() != cols)
    {
        return Err(CellError::Value);
    }
    let used = |area: &Area| area.sheet.extent();
    let limit_rows = areas
        .iter()
        .map(|a| used(a).0.saturating_sub(a.range.start().row()))
        .max()
        .unwrap_or(0)
        .min(rows);
    let limit_cols = areas
        .iter()
        .map(|a| used(a).1.saturating_sub(a.range.start().col()))
        .max()
        .unwrap_or(0)
        .min(cols);
    let mut sum = 0.0;
    for r in 0..limit_rows {
        for c in 0..limit_cols {
            let mut product = 1.0;
            for area in &areas {
                let addr = area.range.start().offset(i64::from(r), i64::from(c));
                product *= match area.sheet.value(addr) {
                    CellValue::Number(n) => *n,
                    CellValue::Error(e) => return Err(*e),
                    _ => 0.0,
                };
            }
            sum += product;
        }
    }
    Ok(CellValue::Number(sum))
}

fn subtotal(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let code = ev.number(&args[0])?.trunc() as i32;
    let kind = code % 100;
    if !(1..=11).contains(&kind) || !(1..=111).contains(&code) {
        return Err(CellError::Value);
    }
    let mut values = Vec::new();
    let mut nonblank = 0usize;
    for arg in &args[1..] {
        let area = ev.range(arg)?;
        let layout = area.sheet.layout();
        let filtered = |row: u32| {
            layout.filter.as_ref().is_some_and(|filter| {
                row > filter.range.start().row() && row <= filter.range.end().row()
            })
        };
        for (addr, cell) in area.sheet.cells_in(area.range) {
            let row = addr.row();
            if layout.hidden_rows.contains(&row) && (code > 100 || filtered(row)) {
                continue;
            }
            if cell
                .formula
                .as_deref()
                .is_some_and(|f| f.contains("SUBTOTAL("))
            {
                continue;
            }
            match &cell.value {
                CellValue::Number(n) => {
                    values.push(*n);
                    nonblank += 1;
                }
                CellValue::Error(e) => return Err(*e),
                CellValue::Empty => {}
                _ => nonblank += 1,
            }
        }
    }
    let count = values.len() as f64;
    let sum: f64 = values.iter().sum();
    let variance = |sample: bool| -> Result<f64, CellError> {
        let n = if sample { count - 1.0 } else { count };
        if n <= 0.0 {
            return Err(CellError::Div0);
        }
        let mean = sum / count;
        Ok(values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n)
    };
    let result = match kind {
        1 if count == 0.0 => return Err(CellError::Div0),
        1 => sum / count,
        2 => count,
        3 => nonblank as f64,
        4 => values
            .iter()
            .copied()
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
            .unwrap_or(0.0),
        5 => values
            .iter()
            .copied()
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
            .unwrap_or(0.0),
        6 => values.iter().product(),
        7 => variance(true)?.sqrt(),
        8 => variance(false)?.sqrt(),
        9 => sum,
        10 => variance(true)?,
        _ => variance(false)?,
    };
    Ok(CellValue::Number(result))
}

fn to_multiple(
    ev: &Evaluator,
    args: &[Expr],
    round: fn(f64) -> f64,
) -> Result<CellValue, CellError> {
    let n = ev.number(&args[0])?;
    let m = ev.number(&args[1])?;
    if m == 0.0 {
        return Ok(CellValue::Number(0.0));
    }
    if n > 0.0 && m < 0.0 {
        return Err(CellError::Num);
    }
    Ok(CellValue::Number(round(round_significant(n / m)) * m))
}

fn random(ev: &Evaluator) -> f64 {
    let mut x = ev.now.to_bits()
        ^ (u64::from(ev.at.row()) << 20)
        ^ u64::from(ev.at.col())
        ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn random_between(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let low = ev.number(&args[0])?.ceil();
    let high = ev.number(&args[1])?.floor();
    if low > high {
        return Err(CellError::Num);
    }
    Ok(CellValue::Number(
        low + (random(ev) * (high - low + 1.0)).floor(),
    ))
}

fn log(ev: &Evaluator, arg: &Expr, base: Option<f64>) -> Result<CellValue, CellError> {
    let n = positive(ev.number(arg)?, false)?;
    match base {
        None => Ok(CellValue::Number(n.ln())),
        Some(b) if b <= 0.0 || b == 1.0 => Err(CellError::Num),
        Some(b) => Ok(CellValue::Number(n.ln() / b.ln())),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Fold {
    Sum,
    Count,
    Average,
    Max,
    Min,
}

fn multi_criteria(
    ev: &Evaluator,
    target: Option<&Expr>,
    pairs: &[Expr],
    fold: Fold,
) -> Result<CellValue, CellError> {
    if pairs.is_empty() || !pairs.len().is_multiple_of(2) {
        return Err(CellError::Value);
    }
    let mut tests = Vec::new();
    for pair in pairs.chunks(2) {
        let area = ev.range(&pair[0])?;
        let criteria = match ev.scalar(&pair[1]) {
            CellValue::Error(e) => return Err(e),
            value => Criteria::new(&value, ev.locale),
        };
        tests.push((area, criteria));
    }
    let (rows, cols) = (tests[0].0.range.rows(), tests[0].0.range.cols());
    if tests
        .iter()
        .any(|(a, _)| a.range.rows() != rows || a.range.cols() != cols)
    {
        return Err(CellError::Value);
    }
    let target = match target {
        Some(arg) => Some(ev.range(arg)?),
        None => None,
    };
    let extent = |area: &Area| {
        let (r, c) = area.sheet.extent();
        (
            r.saturating_sub(area.range.start().row()),
            c.saturating_sub(area.range.start().col()),
        )
    };
    let every_blank_matches = tests.iter().all(|(_, c)| c.matches(&CellValue::Empty));
    let (limit_rows, limit_cols) = if every_blank_matches {
        (rows, cols)
    } else {
        tests
            .iter()
            .map(|(a, _)| extent(a))
            .fold((0, 0), |(r, c), (er, ec)| (r.max(er), c.max(ec)))
    };
    let (limit_rows, limit_cols) = (limit_rows.min(rows), limit_cols.min(cols));
    let mut hits = 0usize;
    let mut values = Vec::new();
    for r in 0..limit_rows {
        for c in 0..limit_cols {
            let all = tests.iter().all(|(area, criteria)| {
                let addr = area.range.start().offset(i64::from(r), i64::from(c));
                criteria.matches(area.sheet.value(addr))
            });
            if !all {
                continue;
            }
            hits += 1;
            if let Some(area) = target {
                let addr = area.range.start().offset(i64::from(r), i64::from(c));
                match area.sheet.value(addr) {
                    CellValue::Number(n) => values.push(*n),
                    CellValue::Error(e) => return Err(*e),
                    _ => {}
                }
            }
        }
    }
    if fold == Fold::Count && every_blank_matches {
        let outside =
            u64::from(rows) * u64::from(cols) - u64::from(limit_rows) * u64::from(limit_cols);
        hits += outside as usize;
    }
    Ok(CellValue::Number(match fold {
        Fold::Count => hits as f64,
        Fold::Sum => values.iter().sum(),
        Fold::Average if values.is_empty() => return Err(CellError::Div0),
        Fold::Average => values.iter().sum::<f64>() / values.len() as f64,
        Fold::Max => values
            .iter()
            .copied()
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
            .unwrap_or(0.0),
        Fold::Min => values
            .iter()
            .copied()
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
            .unwrap_or(0.0),
    }))
}

fn count_blank(ev: &Evaluator, arg: &Expr) -> Result<CellValue, CellError> {
    let area = ev.range(arg)?;
    let size = u64::from(area.range.rows()) * u64::from(area.range.cols());
    let filled = area
        .sheet
        .cells_in(area.range)
        .filter(|(_, cell)| {
            !matches!(&cell.value, CellValue::Empty) && cell.value != CellValue::Text(String::new())
        })
        .count() as u64;
    Ok(CellValue::Number((size - filled) as f64))
}

fn sorted(ev: &Evaluator, args: &[Expr]) -> Result<Vec<f64>, CellError> {
    let mut values = Vec::new();
    numbers(ev, args, |n| values.push(n))?;
    values.sort_by(f64::total_cmp);
    Ok(values)
}

fn median(values: &[f64]) -> Result<CellValue, CellError> {
    let n = values.len();
    if n == 0 {
        return Err(CellError::Num);
    }
    Ok(CellValue::Number(if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }))
}

fn kth(ev: &Evaluator, args: &[Expr], largest: bool) -> Result<CellValue, CellError> {
    let values = sorted(ev, &args[..1])?;
    let k = ev.number(&args[1])?.ceil();
    if k < 1.0 || k as usize > values.len() {
        return Err(CellError::Num);
    }
    let k = k as usize;
    Ok(CellValue::Number(if largest {
        values[values.len() - k]
    } else {
        values[k - 1]
    }))
}

fn rank(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let n = ev.number(&args[0])?;
    let values = sorted(ev, &args[1..2])?;
    let ascending = match given(args.get(2)) {
        Some(arg) => ev.number(arg)? != 0.0,
        None => false,
    };
    if !values.contains(&n) {
        return Err(CellError::NA);
    }
    let better = values
        .iter()
        .filter(|v| if ascending { **v < n } else { **v > n })
        .count();
    Ok(CellValue::Number((better + 1) as f64))
}

fn stdev(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let values = sorted(ev, args)?;
    let n = values.len() as f64;
    if n < 2.0 {
        return Err(CellError::Div0);
    }
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    Ok(CellValue::Number(variance.sqrt()))
}

fn text_format(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let value = ev.scalar(&args[0]);
    if let CellValue::Error(e) = value {
        return Err(e);
    }
    let code = ev.locale.format_code(&ev.text(&args[1])?);
    let value = match &value {
        CellValue::Text(s) => ev
            .locale
            .parse_value(s.trim())
            .map_or(value.clone(), CellValue::Number),
        other => other.clone(),
    };
    Ok(CellValue::Text(ev.locale.format_value(&value, &code)))
}

fn value_of(ev: &Evaluator, arg: &Expr) -> Result<CellValue, CellError> {
    match ev.scalar(arg) {
        CellValue::Number(n) => Ok(CellValue::Number(n)),
        CellValue::Empty => Ok(CellValue::Number(0.0)),
        CellValue::Text(s) => ev
            .locale
            .parse_value(s.trim())
            .map(CellValue::Number)
            .ok_or(CellError::Value),
        CellValue::Error(e) => Err(e),
        CellValue::Bool(_) => Err(CellError::Value),
    }
}

fn text_join(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let delimiter = ev.text(&args[0])?;
    let skip_empty = ev.boolean(&args[1])?;
    let mut parts = Vec::new();
    for arg in &args[2..] {
        match ev.value(arg) {
            Value::Range(area) => {
                let (start, end) = (area.range.start(), area.range.end());
                let (rows, cols) = area.sheet.extent();
                for row in start.row()..=end.row().min(rows.saturating_sub(1)) {
                    for col in start.col()..=end.col().min(cols.saturating_sub(1)) {
                        let addr = CellAddr::new(row, col).expect("inside the range");
                        parts.push(ev.to_text(area.sheet.value(addr))?);
                    }
                }
            }
            Value::Scalar(value) => parts.push(ev.to_text(&value)?),
        }
    }
    if skip_empty {
        parts.retain(|p| !p.is_empty());
    }
    Ok(CellValue::Text(parts.join(&delimiter)))
}

fn proper(ev: &Evaluator, text: &str) -> String {
    let mut out = String::new();
    let mut start = true;
    for c in text.chars() {
        let s = c.to_string();
        if c.is_alphabetic() {
            out.push_str(&if start {
                ev.locale.upper(&s)
            } else {
                ev.locale.lower(&s)
            });
            start = false;
        } else {
            out.push(c);
            start = true;
        }
    }
    out
}

fn substitute(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let text = ev.text(&args[0])?;
    let old = ev.text(&args[1])?;
    let new = ev.text(&args[2])?;
    if old.is_empty() {
        return Ok(CellValue::Text(text));
    }
    let Some(arg) = given(args.get(3)) else {
        return Ok(CellValue::Text(text.replace(&old, &new)));
    };
    let instance = ev.number(arg)?.trunc();
    if instance < 1.0 {
        return Err(CellError::Value);
    }
    let found = text.match_indices(&old).nth(instance as usize - 1);
    Ok(CellValue::Text(match found {
        Some((at, _)) => format!("{}{new}{}", &text[..at], &text[at + old.len()..]),
        None => text,
    }))
}

fn replace(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let text: Vec<char> = ev.text(&args[0])?.chars().collect();
    let start = ev.number(&args[1])?.trunc();
    let count = ev.number(&args[2])?.trunc();
    if start < 1.0 || count < 0.0 {
        return Err(CellError::Value);
    }
    let from = (start as usize - 1).min(text.len());
    let to = (from + count as usize).min(text.len());
    let mut out: String = text[..from].iter().collect();
    out.push_str(&ev.text(&args[3])?);
    out.extend(&text[to..]);
    Ok(CellValue::Text(out))
}

fn find(ev: &Evaluator, args: &[Expr], fold_case: bool) -> Result<CellValue, CellError> {
    let needle = ev.text(&args[0])?;
    let haystack = ev.text(&args[1])?;
    let start = match given(args.get(2)) {
        Some(arg) => ev.number(arg)?.trunc(),
        None => 1.0,
    };
    let chars: Vec<char> = haystack.chars().collect();
    if start < 1.0 || start as usize > chars.len() + 1 {
        return Err(CellError::Value);
    }
    let prepare = |s: &str| {
        if fold_case {
            ev.locale.lower(s)
        } else {
            s.to_owned()
        }
    };
    let needle: Vec<char> = prepare(&needle).chars().collect();
    let body: Vec<char> = prepare(&haystack).chars().collect();
    let first = start as usize - 1;
    if needle.is_empty() {
        return Ok(CellValue::Number(start));
    }
    (first..=body.len().saturating_sub(needle.len()))
        .find(|&i| body[i..].starts_with(&needle))
        .map(|i| CellValue::Number((i + 1) as f64))
        .ok_or(CellError::Value)
}

fn repeat(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let text = ev.text(&args[0])?;
    let times = ev.number(&args[1])?.trunc();
    if times < 0.0 || text.chars().count() as f64 * times > 32_767.0 {
        return Err(CellError::Value);
    }
    Ok(CellValue::Text(text.repeat(times as usize)))
}

fn exact_text(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    Ok(CellValue::Bool(ev.text(&args[0])? == ev.text(&args[1])?))
}

fn days_between(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    Ok(CellValue::Number(
        day_number(ev, &args[0])? - day_number(ev, &args[1])?,
    ))
}

fn day_number(ev: &Evaluator, arg: &Expr) -> Result<f64, CellError> {
    let n = ev.number(arg)?;
    if n < 0.0 {
        return Err(CellError::Num);
    }
    Ok(n.floor())
}

fn serial_of(year: i64, month: i64, day: f64) -> Result<f64, CellError> {
    let months = year * 12 + month - 1;
    let (year, month) = (months.div_euclid(12), months.rem_euclid(12) + 1);
    let first = date_serial(year, month as u32, 1).ok_or(CellError::Num)?;
    Ok(first + day - 1.0)
}

fn date(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let mut year = ev.number(&args[0])?.trunc() as i64;
    if (0..1900).contains(&year) {
        year += 1900;
    }
    let month = ev.number(&args[1])?.trunc() as i64;
    let day = ev.number(&args[2])?.trunc();
    let serial = serial_of(year, month, day)?;
    if serial < 0.0 {
        return Err(CellError::Num);
    }
    Ok(CellValue::Number(serial))
}

fn part(
    ev: &Evaluator,
    arg: &Expr,
    f: impl Fn(&crate::date::DateTime) -> f64,
) -> Result<CellValue, CellError> {
    let n = ev.number(arg)?;
    let parts = from_serial(n).ok_or(CellError::Num)?;
    Ok(CellValue::Number(f(&parts)))
}

fn weekday_index(serial: f64) -> i64 {
    (serial.floor() as i64 + 6).rem_euclid(7)
}

fn weekday(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let serial = day_number(ev, &args[0])?;
    let kind = match given(args.get(1)) {
        Some(arg) => ev.number(arg)?.trunc() as i64,
        None => 1,
    };
    let sunday_based = weekday_index(serial);
    let value = match kind {
        1 => sunday_based + 1,
        2 => (sunday_based + 6) % 7 + 1,
        3 => (sunday_based + 6) % 7,
        _ => return Err(CellError::Num),
    };
    Ok(CellValue::Number(value as f64))
}

fn week_number(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let serial = day_number(ev, &args[0])?;
    let monday = match given(args.get(1)) {
        Some(arg) => match ev.number(arg)?.trunc() as i64 {
            1 => false,
            2 => true,
            _ => return Err(CellError::Num),
        },
        None => false,
    };
    let year = from_serial(serial).ok_or(CellError::Num)?.year;
    let first = date_serial(year, 1, 1).ok_or(CellError::Num)?;
    let mut offset = weekday_index(first);
    if monday {
        offset = (offset + 6) % 7;
    }
    Ok(CellValue::Number(
        ((serial - first) as i64 + offset).div_euclid(7) as f64 + 1.0,
    ))
}

fn shift_months(ev: &Evaluator, args: &[Expr], end_of_month: bool) -> Result<CellValue, CellError> {
    let start = from_serial(day_number(ev, &args[0])?).ok_or(CellError::Num)?;
    let months = ev.number(&args[1])?.trunc() as i64;
    let total = start.year * 12 + i64::from(start.month) - 1 + months;
    let (year, month) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let next = serial_of(year, i64::from(month) + 1, 1.0)?;
    let last_day = next - date_serial(year, month, 1).ok_or(CellError::Num)?;
    let day = if end_of_month {
        last_day
    } else {
        f64::from(start.day).min(last_day)
    };
    Ok(CellValue::Number(
        date_serial(year, month, 1).ok_or(CellError::Num)? + day - 1.0,
    ))
}

fn holidays(ev: &Evaluator, arg: Option<&Expr>) -> Result<Vec<f64>, CellError> {
    let mut out = Vec::new();
    if let Some(arg) = given(arg) {
        numbers(ev, std::slice::from_ref(arg), |n| out.push(n.floor()))?;
    }
    Ok(out)
}

fn workday_at(serial: f64, holidays: &[f64]) -> bool {
    let day = weekday_index(serial);
    day != 0 && day != 6 && !holidays.contains(&serial)
}

fn network_days(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let start = day_number(ev, &args[0])?;
    let end = day_number(ev, &args[1])?;
    let holidays = holidays(ev, args.get(2))?;
    let (low, high, sign) = if start <= end {
        (start, end, 1.0)
    } else {
        (end, start, -1.0)
    };
    let mut count = 0.0;
    let mut day = low;
    while day <= high {
        if workday_at(day, &holidays) {
            count += 1.0;
        }
        day += 1.0;
    }
    Ok(CellValue::Number(sign * count))
}

fn workday(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let mut day = day_number(ev, &args[0])?;
    let days = ev.number(&args[1])?.trunc();
    let holidays = holidays(ev, args.get(2))?;
    let step = if days < 0.0 { -1.0 } else { 1.0 };
    let mut left = days.abs();
    while left > 0.0 {
        day += step;
        if workday_at(day, &holidays) {
            left -= 1.0;
        }
    }
    Ok(CellValue::Number(day))
}

fn date_dif(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let start_serial = day_number(ev, &args[0])?;
    let end_serial = day_number(ev, &args[1])?;
    if start_serial > end_serial {
        return Err(CellError::Num);
    }
    let start = from_serial(start_serial).ok_or(CellError::Num)?;
    let end = from_serial(end_serial).ok_or(CellError::Num)?;
    let unit = ev.text(&args[2])?.to_uppercase();
    let mut months = (end.year - start.year) * 12 + i64::from(end.month) - i64::from(start.month);
    if end.day < start.day {
        months -= 1;
    }
    let value = match unit.as_str() {
        "D" => end_serial - start_serial,
        "M" => months as f64,
        "Y" => (months / 12) as f64,
        "YM" => (months % 12) as f64,
        "MD" => {
            let anchor = serial_of(
                end.year,
                i64::from(end.month) - i64::from(end.day < start.day),
                f64::from(start.day),
            )?;
            end_serial - anchor
        }
        "YD" => {
            let year = if (end.month, end.day) >= (start.month, start.day) {
                end.year
            } else {
                end.year - 1
            };
            end_serial - serial_of(year, i64::from(start.month), f64::from(start.day))?
        }
        _ => return Err(CellError::Num),
    };
    Ok(CellValue::Number(value))
}

fn time(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let h = ev.number(&args[0])?.trunc();
    let m = ev.number(&args[1])?.trunc();
    let s = ev.number(&args[2])?.trunc();
    let seconds = h * 3600.0 + m * 60.0 + s;
    if seconds < 0.0 {
        return Err(CellError::Num);
    }
    Ok(CellValue::Number((seconds / SECONDS_PER_DAY).fract()))
}

fn hlookup(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let target = match ev.scalar(&args[0]) {
        CellValue::Error(e) => return Err(e),
        value => value,
    };
    let Area { sheet, range } = ev.range(&args[1])?;
    let row = ev.number(&args[2])?.trunc();
    if row < 1.0 {
        return Err(CellError::Value);
    }
    if row > f64::from(range.rows()) {
        return Err(CellError::Ref);
    }
    let approximate = match given(args.get(3)) {
        Some(arg) => ev.boolean(arg)?,
        None => true,
    };
    let first = CellRange::new(
        range.start(),
        range.start().offset(0, i64::from(range.cols()) - 1),
    );
    let col = search(
        ev,
        sheet,
        first,
        &target,
        if approximate { 1 } else { 0 },
        false,
    )?;
    let addr = CellAddr::new(
        range.start().row() + row as u32 - 1,
        range.start().col() + col,
    )
    .map_err(|_| CellError::Ref)?;
    Ok(match sheet.value(addr) {
        CellValue::Empty => CellValue::Number(0.0),
        value => value.clone(),
    })
}

fn search(
    ev: &Evaluator,
    sheet: &crate::Sheet,
    range: CellRange,
    target: &CellValue,
    mode: i32,
    vertical: bool,
) -> Result<u32, CellError> {
    let index = |addr: CellAddr| {
        if vertical {
            addr.row() - range.start().row()
        } else {
            addr.col() - range.start().col()
        }
    };
    let same_kind = |v: &CellValue| std::mem::discriminant(v) == std::mem::discriminant(target);
    let mut found = None;
    for (addr, cell) in sheet.cells_in(range) {
        let value = &cell.value;
        match mode {
            0 => {
                let hit = match target {
                    CellValue::Text(_) => {
                        !value.is_empty() && Criteria::new(target, ev.locale).matches(value)
                    }
                    _ => same_kind(value) && compare(value, target) == Ok(Ordering::Equal),
                };
                if hit {
                    return Ok(index(addr));
                }
            }
            1 => {
                if !same_kind(value) {
                    continue;
                }
                match compare(value, target) {
                    Ok(Ordering::Greater) => break,
                    Ok(_) => found = Some(index(addr)),
                    Err(_) => {}
                }
            }
            _ => {
                if !same_kind(value) {
                    continue;
                }
                match compare(value, target) {
                    Ok(Ordering::Less) => break,
                    Ok(_) => found = Some(index(addr)),
                    Err(_) => {}
                }
            }
        }
    }
    found.ok_or(CellError::NA)
}

fn match_position(ev: &Evaluator, args: &[Expr]) -> Result<CellValue, CellError> {
    let target = match ev.scalar(&args[0]) {
        CellValue::Error(e) => return Err(e),
        value => value,
    };
    let Area { sheet, range } = ev.range(&args[1])?;
    let vertical = if range.cols() == 1 {
        true
    } else if range.rows() == 1 {
        false
    } else {
        return Err(CellError::NA);
    };
    let mode = match given(args.get(2)) {
        Some(arg) => {
            let n = ev.number(arg)?;
            if n > 0.0 {
                1
            } else if n < 0.0 {
                -1
            } else {
                0
            }
        }
        None => 1,
    };
    let i = search(ev, sheet, range, &target, mode, vertical)?;
    Ok(CellValue::Number(f64::from(i) + 1.0))
}

fn position(ev: &Evaluator, arg: Option<&Expr>, row: bool) -> Result<CellValue, CellError> {
    let addr = match given(arg) {
        Some(arg) => ev.range(arg)?.range.start(),
        None => ev.at,
    };
    let index = if row { addr.row() } else { addr.col() };
    Ok(CellValue::Number(f64::from(index) + 1.0))
}

fn area_value<'a>(area: Area<'a>) -> Value<'a> {
    if area.range.is_single() {
        Value::Scalar(area.sheet.value(area.range.start()).clone())
    } else {
        Value::Range(area)
    }
}

fn index<'a>(ev: &Evaluator<'a>, args: &[Expr]) -> Value<'a> {
    let run = || -> Result<Value<'a>, CellError> {
        let area = ev.range(&args[0])?;
        let first = ev.number(&args[1])?.trunc();
        let second = match given(args.get(2)) {
            Some(arg) => Some(ev.number(arg)?.trunc()),
            None => None,
        };
        let (rows, cols) = (area.range.rows(), area.range.cols());
        let (row, col) = match second {
            Some(col) => (first, col),
            None if rows == 1 => (1.0, first),
            None => (first, if cols == 1 { 1.0 } else { 0.0 }),
        };
        if row < 0.0 || col < 0.0 || row > f64::from(rows) || col > f64::from(cols) {
            return Err(CellError::Ref);
        }
        let start = area.range.start();
        let (top, bottom) = if row == 0.0 {
            (start.row(), area.range.end().row())
        } else {
            let r = start.row() + row as u32 - 1;
            (r, r)
        };
        let (left, right) = if col == 0.0 {
            (start.col(), area.range.end().col())
        } else {
            let c = start.col() + col as u32 - 1;
            (c, c)
        };
        let range = CellRange::new(
            CellAddr::new(top, left).map_err(|_| CellError::Ref)?,
            CellAddr::new(bottom, right).map_err(|_| CellError::Ref)?,
        );
        Ok(area_value(Area {
            sheet: area.sheet,
            range,
        }))
    };
    run().unwrap_or_else(|e| Value::Scalar(CellValue::Error(e)))
}

fn offset<'a>(ev: &Evaluator<'a>, args: &[Expr]) -> Value<'a> {
    let run = || -> Result<Value<'a>, CellError> {
        let area = ev.range(&args[0])?;
        let rows = ev.number(&args[1])?.trunc() as i64;
        let cols = ev.number(&args[2])?.trunc() as i64;
        let height = match given(args.get(3)) {
            Some(arg) => ev.number(arg)?.trunc() as i64,
            None => i64::from(area.range.rows()),
        };
        let width = match given(args.get(4)) {
            Some(arg) => ev.number(arg)?.trunc() as i64,
            None => i64::from(area.range.cols()),
        };
        if height < 1 || width < 1 {
            return Err(CellError::Ref);
        }
        let top = i64::from(area.range.start().row()) + rows;
        let left = i64::from(area.range.start().col()) + cols;
        let bottom = top + height - 1;
        let right = left + width - 1;
        let inside = |v: i64, max: u32| (0..i64::from(max)).contains(&v);
        if !(inside(top, MAX_ROWS)
            && inside(bottom, MAX_ROWS)
            && inside(left, MAX_COLS)
            && inside(right, MAX_COLS))
        {
            return Err(CellError::Ref);
        }
        let range = CellRange::new(
            CellAddr::new(top as u32, left as u32).map_err(|_| CellError::Ref)?,
            CellAddr::new(bottom as u32, right as u32).map_err(|_| CellError::Ref)?,
        );
        Ok(area_value(Area {
            sheet: area.sheet,
            range,
        }))
    };
    run().unwrap_or_else(|e| Value::Scalar(CellValue::Error(e)))
}

fn indirect<'a>(ev: &Evaluator<'a>, args: &[Expr]) -> Value<'a> {
    let run = || -> Result<Value<'a>, CellError> {
        if let Some(arg) = given(args.get(1))
            && !ev.boolean(arg)?
        {
            return Err(CellError::Ref);
        }
        let text = ev.text(&args[0])?;
        let expr = super::parse(text.trim(), ev.locale).map_err(|_| CellError::Ref)?;
        match ev.unparen(&expr) {
            Expr::Reference { .. } => Ok(match ev.range(&expr) {
                Ok(area) => area_value(area),
                Err(e) => Value::Scalar(CellValue::Error(e)),
            }),
            _ => Err(CellError::Ref),
        }
    };
    run().unwrap_or_else(|e| Value::Scalar(CellValue::Error(e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::parse;
    use crate::{EN, Locale, Sheet, TR, Workbook};

    fn sheet() -> Sheet {
        let mut sheet = Sheet::new("S");
        let rows = [
            ["Bölge", "Ürün", "Adet", "Tutar"],
            ["Kuzey", "Elma", "10", "25"],
            ["Güney", "Armut", "4", "16"],
            ["Kuzey", "Kiraz", "5", "60"],
            ["Güney", "Elma", "8", "20"],
        ];
        for (r, row) in rows.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                sheet.set_input(CellAddr::new(r as u32, c as u32).unwrap(), value);
            }
        }
        sheet.set_input("F1".parse().unwrap(), "45366");
        sheet.set_input("F2".parse().unwrap(), "  çok   boşluklu  metin ");
        sheet
    }

    fn eval_in(src: &str, locale: &Locale) -> CellValue {
        let book = Workbook::from_sheets(vec![sheet()]).unwrap();
        Evaluator {
            book: &book,
            sheet: 0,
            locale,
            at: "H3".parse().unwrap(),
            today: 46000.0,
            now: 46000.75,
        }
        .evaluate(&parse(src, locale).unwrap())
    }

    fn eval(src: &str) -> CellValue {
        eval_in(src, &EN)
    }

    fn tr(src: &str) -> CellValue {
        eval_in(src, &TR)
    }

    fn n(x: f64) -> CellValue {
        CellValue::Number(x)
    }

    fn t(s: &str) -> CellValue {
        CellValue::Text(s.into())
    }

    fn b(x: bool) -> CellValue {
        CellValue::Bool(x)
    }

    fn err(e: CellError) -> CellValue {
        CellValue::Error(e)
    }

    #[test]
    fn logical_functions() {
        assert_eq!(eval("IFS(C2>20,\"a\",C2>5,\"b\")"), t("b"));
        assert_eq!(eval("IFS(FALSE,1)"), err(CellError::NA));
        assert_eq!(eval("SWITCH(B3,\"Elma\",1,\"Armut\",2,0)"), n(2.0));
        assert_eq!(eval("SWITCH(B3,\"x\",1,0)"), n(0.0));
        assert_eq!(eval("IFNA(VLOOKUP(\"yok\",B2:C5,2,FALSE),\"-\")"), t("-"));
        assert_eq!(eval("XOR(TRUE,TRUE,TRUE)"), b(true));
        assert_eq!(tr("ÇOKEĞER(C2>20;\"a\";DOĞRU();\"c\")"), t("c"));
        assert_eq!(eval("CHOOSE(2,\"a\",\"b\",\"c\")"), t("b"));
        assert_eq!(eval("SUM(CHOOSE(1,C2:C5,D2:D5))"), n(27.0));
    }

    #[test]
    fn math_functions() {
        assert_eq!(eval("INT(-2.5)"), n(-3.0));
        assert_eq!(eval("MOD(-3,2)"), n(1.0));
        assert_eq!(eval("MOD(5,0)"), err(CellError::Div0));
        assert_eq!(eval("POWER(2,10)"), n(1024.0));
        assert_eq!(eval("SQRT(-1)"), err(CellError::Num));
        assert_eq!(eval("ROUNDUP(2.31,1)"), n(2.4));
        assert_eq!(eval("ROUNDUP(-2.31,1)"), n(-2.4));
        assert_eq!(eval("ROUNDDOWN(2.39,1)"), n(2.3));
        assert_eq!(eval("TRUNC(-8.9)"), n(-8.0));
        assert_eq!(eval("PRODUCT(C2:C3,2)"), n(80.0));
        assert_eq!(eval("SUMPRODUCT(C2:C5,D2:D5)"), n(774.0));
        assert_eq!(eval("CEILING(2.1,0.5)"), n(2.5));
        assert_eq!(eval("FLOOR(7,3)"), n(6.0));
        assert_eq!(eval("MROUND(10,3)"), n(9.0));
        assert_eq!(eval("SIGN(-4)"), n(-1.0));
        assert_eq!(eval("LOG(100)"), n(2.0));
        assert_eq!(eval("LN(0)"), err(CellError::Num));
        assert_eq!(tr("TOPLA.ÇARPIM(C2:C5;D2:D5)"), n(774.0));
        let r = eval("RAND()");
        assert!(matches!(r, CellValue::Number(x) if (0.0..1.0).contains(&x)));
        let r = eval("RANDBETWEEN(1,6)");
        assert!(matches!(r, CellValue::Number(x) if (1.0..=6.0).contains(&x) && x.fract() == 0.0));
    }

    #[test]
    fn multi_criteria_functions() {
        assert_eq!(eval("SUMIFS(D2:D5,A2:A5,\"Kuzey\",C2:C5,\">5\")"), n(25.0));
        assert_eq!(eval("COUNTIFS(A2:A5,\"Güney\")"), n(2.0));
        assert_eq!(eval("COUNTIFS(A:A,\"Kuzey\",B:B,\"Elma\")"), n(1.0));
        assert_eq!(eval("AVERAGEIF(B2:B5,\"Elma\",D2:D5)"), n(22.5));
        assert_eq!(
            eval("AVERAGEIFS(D2:D5,A2:A5,\"yok\")"),
            err(CellError::Div0)
        );
        assert_eq!(eval("MAXIFS(D2:D5,A2:A5,\"Kuzey\")"), n(60.0));
        assert_eq!(eval("MINIFS(D2:D5,A2:A5,\"Güney\")"), n(16.0));
        assert_eq!(eval("COUNTBLANK(A1:E5)"), n(5.0));
        assert_eq!(tr("ÇOKETOPLA(D2:D5;B2:B5;\"elma\")"), n(45.0));
    }

    #[test]
    fn statistics() {
        assert_eq!(eval("MEDIAN(C2:C5)"), n(6.5));
        assert_eq!(eval("LARGE(D2:D5,2)"), n(25.0));
        assert_eq!(eval("SMALL(D2:D5,1)"), n(16.0));
        assert_eq!(eval("RANK(D3,D2:D5)"), n(4.0));
        assert_eq!(eval("RANK(D3,D2:D5,1)"), n(1.0));
        assert_eq!(eval("ROUND(STDEV(C2:C5),4)"), n(2.7538));
    }

    #[test]
    fn subtotal_skips_hidden_rows() {
        let mut first = sheet();
        first.layout_mut().hidden_rows.insert(2);
        first.set_input("C7".parse().unwrap(), "=SUBTOTAL(9,C2:C5)");
        let book = Workbook::from_sheets(vec![first]).unwrap();
        let ev = Evaluator {
            book: &book,
            sheet: 0,
            locale: &EN,
            at: "H1".parse().unwrap(),
            today: 0.0,
            now: 0.0,
        };
        let run = |src: &str| ev.evaluate(&parse(src, &EN).unwrap());
        assert_eq!(run("SUBTOTAL(9,C2:C5)"), run("SUM(C2:C5)"));
        assert_eq!(run("SUBTOTAL(109,C2:C7)"), n(23.0));
        assert_eq!(run("SUBTOTAL(101,C2:C5)"), n(23.0 / 3.0));
        assert_eq!(run("SUBTOTAL(103,A1:A5)"), n(4.0));
        assert_eq!(run("SUBTOTAL(12,C2:C5)"), err(CellError::Value));
        let mut filtered = sheet();
        filtered.layout_mut().hidden_rows.insert(2);
        filtered.layout_mut().filter = Some(crate::AutoFilter {
            range: "A1:C5".parse().unwrap(),
            criteria: Default::default(),
        });
        let book = Workbook::from_sheets(vec![filtered]).unwrap();
        let ev = Evaluator { book: &book, ..ev };
        assert_eq!(
            ev.evaluate(&parse("SUBTOTAL(9,C2:C5)", &EN).unwrap()),
            n(23.0)
        );
    }

    #[test]
    fn text_functions() {
        assert_eq!(eval("TRIM(F2)"), t("çok boşluklu metin"));
        assert_eq!(tr("BÜYÜKHARF(\"istanbul ılık\")"), t("İSTANBUL ILIK"));
        assert_eq!(tr("KÜÇÜKHARF(\"İZMİR IŞIK\")"), t("izmir ışık"));
        assert_eq!(tr("YAZIM.DÜZENİ(\"iSTANBUL izmir\")"), t("İstanbul İzmir"));
        assert_eq!(eval("SUBSTITUTE(\"a-b-c\",\"-\",\"+\")"), t("a+b+c"));
        assert_eq!(eval("SUBSTITUTE(\"a-b-c\",\"-\",\"+\",2)"), t("a-b+c"));
        assert_eq!(eval("REPLACE(\"abcdef\",2,3,\"X\")"), t("aXef"));
        assert_eq!(eval("FIND(\"b\",\"abcb\",3)"), n(4.0));
        assert_eq!(eval("FIND(\"B\",\"abc\")"), err(CellError::Value));
        assert_eq!(eval("SEARCH(\"B\",\"abc\")"), n(2.0));
        assert_eq!(eval("REPT(\"ab\",3)"), t("ababab"));
        assert_eq!(eval("EXACT(\"a\",\"A\")"), b(false));
        assert_eq!(eval("CHAR(65)&CODE(\"a\")"), t("A97"));
        assert_eq!(
            eval("TEXTJOIN(\", \",TRUE,B2:B3,\"\",\"x\")"),
            t("Elma, Armut, x")
        );
        assert_eq!(eval("VALUE(\"12.5\")"), n(12.5));
        assert_eq!(tr("SAYIYAÇEVİR(\"1.234,5\")"), n(1234.5));
        assert_eq!(tr("METNEÇEVİR(1234,5;\"#.##0,00\")"), t("1.234,50"));
        assert_eq!(tr("METNEÇEVİR(F1;\"gg.aa.yyyy\")"), t("15.03.2024"));
        assert_eq!(tr("METNEÇEVİR(0,25;\"0%\")"), t("25%"));
    }

    #[test]
    fn date_functions() {
        let d = |y, m, d| n(date_serial(y, m, d).unwrap());
        assert_eq!(eval("DATE(2024,3,15)"), d(2024, 3, 15));
        assert_eq!(eval("DATE(2024,14,1)"), d(2025, 2, 1));
        assert_eq!(eval("DATE(2024,3,0)"), d(2024, 2, 29));
        assert_eq!(
            eval("YEAR(F1)&\"-\"&MONTH(F1)&\"-\"&DAY(F1)"),
            t("2024-3-15")
        );
        assert_eq!(eval("HOUR(NOW())"), n(18.0));
        assert_eq!(eval("MINUTE(0.75+1/1440)"), n(1.0));
        assert_eq!(eval("WEEKDAY(DATE(2024,3,15))"), n(6.0));
        assert_eq!(eval("WEEKDAY(DATE(2024,3,15),2)"), n(5.0));
        assert_eq!(eval("WEEKNUM(DATE(2024,1,7))"), n(2.0));
        assert_eq!(eval("WEEKNUM(DATE(2024,1,7),2)"), n(1.0));
        assert_eq!(eval("EDATE(DATE(2024,1,31),1)"), d(2024, 2, 29));
        assert_eq!(eval("EOMONTH(DATE(2024,1,15),-1)"), d(2023, 12, 31));
        assert_eq!(
            eval("NETWORKDAYS(DATE(2024,3,11),DATE(2024,3,22))"),
            n(10.0)
        );
        assert_eq!(
            eval("NETWORKDAYS(DATE(2024,3,11),DATE(2024,3,15),DATE(2024,3,13))"),
            n(4.0)
        );
        assert_eq!(eval("WORKDAY(DATE(2024,3,15),1)"), d(2024, 3, 18));
        assert_eq!(eval("WORKDAY(DATE(2024,3,18),-1)"), d(2024, 3, 15));
        assert_eq!(
            eval("DATEDIF(DATE(2020,5,20),DATE(2024,3,15),\"Y\")"),
            n(3.0)
        );
        assert_eq!(
            eval("DATEDIF(DATE(2020,5,20),DATE(2024,3,15),\"M\")"),
            n(45.0)
        );
        assert_eq!(
            eval("DATEDIF(DATE(2020,5,20),DATE(2024,3,15),\"YM\")"),
            n(9.0)
        );
        assert_eq!(
            eval("DATEDIF(DATE(2020,5,20),DATE(2024,3,15),\"MD\")"),
            n(24.0)
        );
        assert_eq!(eval("DAYS(DATE(2024,3,15),DATE(2024,3,1))"), n(14.0));
        assert_eq!(eval("TIME(18,0,0)"), n(0.75));
        assert_eq!(tr("TARİHSAYISI(\"15.03.2024\")"), d(2024, 3, 15));
    }

    #[test]
    fn lookup_functions() {
        assert_eq!(eval("HLOOKUP(\"Adet\",A1:D5,3,FALSE)"), n(4.0));
        assert_eq!(eval("INDEX(A1:D5,3,2)"), t("Armut"));
        assert_eq!(eval("INDEX(B2:B5,2)"), t("Armut"));
        assert_eq!(eval("INDEX(A1:D1,3)"), t("Adet"));
        assert_eq!(eval("SUM(INDEX(C2:D5,0,2))"), n(121.0));
        assert_eq!(eval("MATCH(\"Kiraz\",B1:B5,0)"), n(4.0));
        assert_eq!(eval("MATCH(\"ki*\",B1:B5,0)"), n(4.0));
        assert_eq!(eval("MATCH(7,C3:C4,1)"), n(2.0));
        assert_eq!(eval("INDEX(D2:D5,MATCH(\"Kiraz\",B2:B5,0))"), n(60.0));
        assert_eq!(eval("SUM(OFFSET(C1,1,0,4,1))"), n(27.0));
        assert_eq!(eval("OFFSET(A1,-1,0)"), err(CellError::Ref));
        assert_eq!(eval("INDIRECT(\"B\"&3)"), t("Armut"));
        assert_eq!(eval("SUM(INDIRECT(\"S!C2:C5\"))"), n(27.0));
        assert_eq!(eval("ROW()+COLUMN(D2)"), n(7.0));
        assert_eq!(eval("ROWS(A1:D5)*COLUMNS(A1:D5)"), n(20.0));
    }

    #[test]
    fn information_functions() {
        assert_eq!(eval("ISBLANK(Z1)"), b(true));
        assert_eq!(eval("ISNUMBER(C2)"), b(true));
        assert_eq!(eval("ISTEXT(C2)"), b(false));
        assert_eq!(eval("ISERROR(1/0)"), b(true));
        assert_eq!(eval("ISNA(NA())"), b(true));
        assert_eq!(eval("ISERR(NA())"), b(false));
        assert_eq!(eval("ISLOGICAL(TRUE)"), b(true));
        assert_eq!(tr("EHATALIYSA(1/0)"), b(true));
    }
}
