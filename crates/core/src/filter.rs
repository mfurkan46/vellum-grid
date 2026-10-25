use std::collections::BTreeSet;

use crate::locale::{EN, fold};
use crate::{CellValue, Color, DateTime, Locale, date_serial, from_serial};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    BeginsWith,
    NotBeginsWith,
    EndsWith,
    NotEndsWith,
    Contains,
    NotContains,
}

impl Operator {
    pub const ALL: [Self; 12] = [
        Self::Equal,
        Self::NotEqual,
        Self::Greater,
        Self::GreaterOrEqual,
        Self::Less,
        Self::LessOrEqual,
        Self::BeginsWith,
        Self::NotBeginsWith,
        Self::EndsWith,
        Self::NotEndsWith,
        Self::Contains,
        Self::NotContains,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub op: Operator,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dynamic {
    AboveAverage,
    BelowAverage,
    Tomorrow,
    Today,
    Yesterday,
    NextWeek,
    ThisWeek,
    LastWeek,
    NextMonth,
    ThisMonth,
    LastMonth,
    NextQuarter,
    ThisQuarter,
    LastQuarter,
    NextYear,
    ThisYear,
    LastYear,
    YearToDate,
}

impl Dynamic {
    pub const DATES: [Self; 16] = [
        Self::Tomorrow,
        Self::Today,
        Self::Yesterday,
        Self::NextWeek,
        Self::ThisWeek,
        Self::LastWeek,
        Self::NextMonth,
        Self::ThisMonth,
        Self::LastMonth,
        Self::NextQuarter,
        Self::ThisQuarter,
        Self::LastQuarter,
        Self::NextYear,
        Self::ThisYear,
        Self::LastYear,
        Self::YearToDate,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnFilter {
    Values(BTreeSet<String>),
    Custom {
        first: Condition,
        second: Option<Condition>,
        and: bool,
    },
    Top {
        count: f64,
        percent: bool,
        bottom: bool,
    },
    Dynamic(Dynamic),
    Fill(Option<Color>),
    FontColor(Option<Color>),
}

pub struct Entry<'a> {
    pub value: &'a CellValue,
    pub text: &'a str,
    pub fill: Option<Color>,
    pub color: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Context {
    pub threshold: Option<f64>,
    pub average: Option<f64>,
    pub today: f64,
}

pub fn context(filter: &ColumnFilter, numbers: &[f64], today: f64) -> Context {
    let average = (!numbers.is_empty()).then(|| numbers.iter().sum::<f64>() / numbers.len() as f64);
    let threshold = match filter {
        ColumnFilter::Top {
            count,
            percent,
            bottom,
        } if !numbers.is_empty() => {
            let mut sorted = numbers.to_vec();
            sorted.sort_by(|a, b| a.total_cmp(b));
            if !bottom {
                sorted.reverse();
            }
            let wanted = if *percent {
                (sorted.len() as f64 * count / 100.0).floor().max(1.0)
            } else {
                count.floor().max(1.0)
            } as usize;
            sorted.get(wanted.min(sorted.len()) - 1).copied()
        }
        _ => None,
    };
    Context {
        threshold,
        average,
        today,
    }
}

fn wildcard(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|skip| wildcard(rest, &text[skip..])),
        Some(('?', rest)) => !text.is_empty() && wildcard(rest, &text[1..]),
        Some(('~', rest)) if !rest.is_empty() => {
            text.first() == Some(&rest[0]) && wildcard(&rest[1..], &text[1..])
        }
        Some((c, rest)) => text.first() == Some(c) && wildcard(rest, &text[1..]),
    }
}

fn like(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = fold(pattern).chars().collect();
    let text: Vec<char> = fold(text).chars().collect();
    wildcard(&pattern, &text)
}

fn number_of(value: &CellValue, text: &str, locale: &Locale) -> Option<f64> {
    match value {
        CellValue::Number(n) => Some(*n),
        _ => locale.parse_value(text.trim()),
    }
}

fn condition(condition: &Condition, entry: &Entry, locale: &Locale) -> bool {
    let target = condition.value.trim();
    let numeric = EN.parse_number(target).filter(|_| !target.is_empty());
    let subject = number_of(entry.value, entry.text, locale).filter(|_| !entry.value.is_empty());
    if let (Some(a), Some(b)) = (subject, numeric) {
        let compared = match condition.op {
            Operator::Equal => Some(a == b),
            Operator::NotEqual => Some(a != b),
            Operator::Greater => Some(a > b),
            Operator::GreaterOrEqual => Some(a >= b),
            Operator::Less => Some(a < b),
            Operator::LessOrEqual => Some(a <= b),
            _ => None,
        };
        if let Some(result) = compared {
            return result;
        }
    }
    let text = entry.text;
    match condition.op {
        Operator::Equal => like(target, text),
        Operator::NotEqual => !like(target, text),
        Operator::BeginsWith => like(&format!("{target}*"), text),
        Operator::NotBeginsWith => !like(&format!("{target}*"), text),
        Operator::EndsWith => like(&format!("*{target}"), text),
        Operator::NotEndsWith => !like(&format!("*{target}"), text),
        Operator::Contains => like(&format!("*{target}*"), text),
        Operator::NotContains => !like(&format!("*{target}*"), text),
        Operator::Greater => numeric.is_none() && fold(text) > fold(target),
        Operator::GreaterOrEqual => numeric.is_none() && fold(text) >= fold(target),
        Operator::Less => numeric.is_none() && !text.is_empty() && fold(text) < fold(target),
        Operator::LessOrEqual => {
            numeric.is_none() && !text.is_empty() && fold(text) <= fold(target)
        }
    }
}

fn quarter(month: u32) -> u32 {
    (month - 1) / 3
}

fn date_range(dynamic: Dynamic, today: f64) -> Option<(f64, f64)> {
    let DateTime { year, month, .. } = from_serial(today)?;
    let weekday = (today as i64 + 6).rem_euclid(7) as f64;
    let week = today - weekday;
    let month_start = |year: i64, month: i64| {
        let total = year * 12 + month - 1;
        date_serial(total.div_euclid(12), (total.rem_euclid(12) + 1) as u32, 1)
    };
    let month = i64::from(month);
    let quarter_month = i64::from(quarter(month as u32) * 3 + 1);
    Some(match dynamic {
        Dynamic::Tomorrow => (today + 1.0, today + 2.0),
        Dynamic::Today => (today, today + 1.0),
        Dynamic::Yesterday => (today - 1.0, today),
        Dynamic::NextWeek => (week + 7.0, week + 14.0),
        Dynamic::ThisWeek => (week, week + 7.0),
        Dynamic::LastWeek => (week - 7.0, week),
        Dynamic::NextMonth => (month_start(year, month + 1)?, month_start(year, month + 2)?),
        Dynamic::ThisMonth => (month_start(year, month)?, month_start(year, month + 1)?),
        Dynamic::LastMonth => (month_start(year, month - 1)?, month_start(year, month)?),
        Dynamic::NextQuarter => (
            month_start(year, quarter_month + 3)?,
            month_start(year, quarter_month + 6)?,
        ),
        Dynamic::ThisQuarter => (
            month_start(year, quarter_month)?,
            month_start(year, quarter_month + 3)?,
        ),
        Dynamic::LastQuarter => (
            month_start(year, quarter_month - 3)?,
            month_start(year, quarter_month)?,
        ),
        Dynamic::NextYear => (month_start(year + 1, 1)?, month_start(year + 2, 1)?),
        Dynamic::ThisYear => (month_start(year, 1)?, month_start(year + 1, 1)?),
        Dynamic::LastYear => (month_start(year - 1, 1)?, month_start(year, 1)?),
        Dynamic::YearToDate => (month_start(year, 1)?, today + 1.0),
        Dynamic::AboveAverage | Dynamic::BelowAverage => return None,
    })
}

pub fn matches(filter: &ColumnFilter, entry: &Entry, context: &Context, locale: &Locale) -> bool {
    match filter {
        ColumnFilter::Values(shown) => shown.contains(entry.text),
        ColumnFilter::Custom { first, second, and } => {
            let a = condition(first, entry, locale);
            match second {
                None => a,
                Some(second) if *and => a && condition(second, entry, locale),
                Some(second) => a || condition(second, entry, locale),
            }
        }
        ColumnFilter::Top { bottom, .. } => match (entry.value, context.threshold) {
            (CellValue::Number(n), Some(limit)) => {
                if *bottom {
                    *n <= limit
                } else {
                    *n >= limit
                }
            }
            _ => false,
        },
        ColumnFilter::Dynamic(dynamic) => {
            let CellValue::Number(n) = entry.value else {
                return false;
            };
            match dynamic {
                Dynamic::AboveAverage => context.average.is_some_and(|avg| *n > avg),
                Dynamic::BelowAverage => context.average.is_some_and(|avg| *n < avg),
                other => date_range(*other, context.today.floor())
                    .is_some_and(|(start, end)| *n >= start && *n < end),
            }
        }
        ColumnFilter::Fill(color) => entry.fill == *color,
        ColumnFilter::FontColor(color) => entry.color == *color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TR;

    fn text_entry(text: &str) -> (CellValue, String) {
        (CellValue::Text(text.into()), text.into())
    }

    fn check(filter: &ColumnFilter, value: CellValue, text: &str, context: &Context) -> bool {
        let entry = Entry {
            value: &value,
            text,
            fill: None,
            color: None,
        };
        matches(filter, &entry, context, &TR)
    }

    fn custom(op: Operator, value: &str) -> ColumnFilter {
        ColumnFilter::Custom {
            first: Condition {
                op,
                value: value.into(),
            },
            second: None,
            and: true,
        }
    }

    const NONE: Context = Context {
        threshold: None,
        average: None,
        today: 0.0,
    };

    #[test]
    fn text_conditions_use_wildcards_and_turkish_case() {
        let (value, text) = text_entry("İstanbul Avrupa");
        assert!(check(
            &custom(Operator::BeginsWith, "ist"),
            value.clone(),
            &text,
            &NONE
        ));
        assert!(check(
            &custom(Operator::Contains, "AVR"),
            value.clone(),
            &text,
            &NONE
        ));
        assert!(check(
            &custom(Operator::Equal, "*bul ?vrupa"),
            value.clone(),
            &text,
            &NONE
        ));
        assert!(!check(
            &custom(Operator::EndsWith, "asya"),
            value.clone(),
            &text,
            &NONE
        ));
        assert!(check(
            &custom(Operator::NotContains, "Ankara"),
            value,
            &text,
            &NONE
        ));
        assert!(like("a~*b", "a*b"));
        assert!(!like("a~*b", "axb"));
    }

    #[test]
    fn number_conditions_combine_with_and_or() {
        let between = ColumnFilter::Custom {
            first: Condition {
                op: Operator::GreaterOrEqual,
                value: "10".into(),
            },
            second: Some(Condition {
                op: Operator::LessOrEqual,
                value: "20".into(),
            }),
            and: true,
        };
        let n = |v: f64| CellValue::Number(v);
        assert!(check(&between, n(15.0), "15", &NONE));
        assert!(!check(&between, n(25.0), "25", &NONE));
        let either = ColumnFilter::Custom {
            first: Condition {
                op: Operator::Less,
                value: "1.5".into(),
            },
            second: Some(Condition {
                op: Operator::Greater,
                value: "100".into(),
            }),
            and: false,
        };
        assert!(check(&either, n(1.2), "1,2", &NONE));
        assert!(check(&either, n(500.0), "500", &NONE));
        assert!(!check(&either, n(50.0), "50", &NONE));
        assert!(!check(
            &custom(Operator::Greater, "5"),
            CellValue::Empty,
            "",
            &NONE
        ));
    }

    #[test]
    fn top_and_average_use_column_statistics() {
        let numbers = [5.0, 1.0, 9.0, 7.0, 3.0];
        let top = ColumnFilter::Top {
            count: 2.0,
            percent: false,
            bottom: false,
        };
        let ctx = context(&top, &numbers, 0.0);
        let shown: Vec<f64> = numbers
            .iter()
            .copied()
            .filter(|n| check(&top, CellValue::Number(*n), "", &ctx))
            .collect();
        assert_eq!(shown, [9.0, 7.0]);
        let bottom = ColumnFilter::Top {
            count: 40.0,
            percent: true,
            bottom: true,
        };
        let ctx = context(&bottom, &numbers, 0.0);
        assert!(check(&bottom, CellValue::Number(3.0), "", &ctx));
        assert!(!check(&bottom, CellValue::Number(5.0), "", &ctx));
        let above = ColumnFilter::Dynamic(Dynamic::AboveAverage);
        let ctx = context(&above, &numbers, 0.0);
        assert!(check(&above, CellValue::Number(7.0), "", &ctx));
        assert!(!check(&above, CellValue::Number(5.0), "", &ctx));
    }

    #[test]
    fn dynamic_dates_follow_today() {
        let today = date_serial(2024, 3, 13).unwrap();
        let ctx = Context {
            threshold: None,
            average: None,
            today,
        };
        let date = |y, m, d| CellValue::Number(date_serial(y, m, d).unwrap());
        let is = |dynamic, value| check(&ColumnFilter::Dynamic(dynamic), value, "", &ctx);
        assert!(is(Dynamic::Today, date(2024, 3, 13)));
        assert!(is(Dynamic::Yesterday, date(2024, 3, 12)));
        assert!(is(Dynamic::ThisWeek, date(2024, 3, 10)));
        assert!(is(Dynamic::ThisWeek, date(2024, 3, 16)));
        assert!(!is(Dynamic::ThisWeek, date(2024, 3, 17)));
        assert!(is(Dynamic::NextWeek, date(2024, 3, 17)));
        assert!(is(Dynamic::LastMonth, date(2024, 2, 29)));
        assert!(is(Dynamic::ThisQuarter, date(2024, 1, 1)));
        assert!(is(Dynamic::LastQuarter, date(2023, 12, 31)));
        assert!(is(Dynamic::NextYear, date(2025, 6, 1)));
        assert!(is(Dynamic::YearToDate, date(2024, 3, 13)));
        assert!(!is(Dynamic::YearToDate, date(2024, 3, 14)));
    }
}
