use std::collections::HashMap;

use crate::formula::{self, Evaluator, Expr};
use crate::{
    Border, CellAddr, CellRange, CellValue, Color, EN, Workbook, date_serial, fold, from_serial,
    overlaps,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatePeriod {
    Yesterday,
    Today,
    Tomorrow,
    Last7Days,
    LastWeek,
    ThisWeek,
    NextWeek,
    LastMonth,
    ThisMonth,
    NextMonth,
}

impl DatePeriod {
    pub const ALL: [Self; 10] = [
        Self::Yesterday,
        Self::Today,
        Self::Tomorrow,
        Self::Last7Days,
        Self::LastWeek,
        Self::ThisWeek,
        Self::NextWeek,
        Self::LastMonth,
        Self::ThisMonth,
        Self::NextMonth,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Yesterday => "yesterday",
            Self::Today => "today",
            Self::Tomorrow => "tomorrow",
            Self::Last7Days => "last7Days",
            Self::LastWeek => "lastWeek",
            Self::ThisWeek => "thisWeek",
            Self::NextWeek => "nextWeek",
            Self::LastMonth => "lastMonth",
            Self::ThisMonth => "thisMonth",
            Self::NextMonth => "nextMonth",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }

    fn span(self, today: f64) -> Option<(f64, f64)> {
        let week = today - (today - 1.0).rem_euclid(7.0);
        let month = |offset: i64| {
            let date = from_serial(today)?;
            let index = date.year * 12 + i64::from(date.month) - 1 + offset;
            date_serial(index.div_euclid(12), (index.rem_euclid(12) + 1) as u32, 1)
        };
        Some(match self {
            Self::Yesterday => (today - 1.0, today),
            Self::Today => (today, today + 1.0),
            Self::Tomorrow => (today + 1.0, today + 2.0),
            Self::Last7Days => (today - 6.0, today + 1.0),
            Self::LastWeek => (week - 7.0, week),
            Self::ThisWeek => (week, week + 7.0),
            Self::NextWeek => (week + 7.0, week + 14.0),
            Self::LastMonth => (month(-1)?, month(0)?),
            Self::ThisMonth => (month(0)?, month(1)?),
            Self::NextMonth => (month(1)?, month(2)?),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Threshold {
    Automatic,
    Lowest,
    Highest,
    Number(f64),
    Percent(f64),
    Percentile(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataBar {
    pub color: Color,
    pub gradient: bool,
    pub min: Threshold,
    pub max: Threshold,
    pub show_value: bool,
}

impl DataBar {
    pub fn new(color: Color, gradient: bool) -> Self {
        Self {
            color,
            gradient,
            min: Threshold::Automatic,
            max: Threshold::Automatic,
            show_value: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorScale {
    pub points: Vec<(Threshold, Color)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Arrows3,
    ArrowsGray3,
    Flags3,
    TrafficLights3,
    TrafficLightsRim3,
    Signs3,
    SymbolsCircled3,
    Symbols3,
    Stars3,
    Triangles3,
    Arrows4,
    ArrowsGray4,
    RedToBlack4,
    Rating4,
    TrafficLights4,
    Arrows5,
    ArrowsGray5,
    Rating5,
    Quarters5,
    Boxes5,
}

impl IconKind {
    pub const ALL: [Self; 20] = [
        Self::Arrows3,
        Self::ArrowsGray3,
        Self::Flags3,
        Self::TrafficLights3,
        Self::TrafficLightsRim3,
        Self::Signs3,
        Self::SymbolsCircled3,
        Self::Symbols3,
        Self::Stars3,
        Self::Triangles3,
        Self::Arrows4,
        Self::ArrowsGray4,
        Self::RedToBlack4,
        Self::Rating4,
        Self::TrafficLights4,
        Self::Arrows5,
        Self::ArrowsGray5,
        Self::Rating5,
        Self::Quarters5,
        Self::Boxes5,
    ];

    pub fn count(self) -> usize {
        match self {
            Self::Arrows4
            | Self::ArrowsGray4
            | Self::RedToBlack4
            | Self::Rating4
            | Self::TrafficLights4 => 4,
            Self::Arrows5 | Self::ArrowsGray5 | Self::Rating5 | Self::Quarters5 | Self::Boxes5 => 5,
            _ => 3,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Arrows3 => "3Arrows",
            Self::ArrowsGray3 => "3ArrowsGray",
            Self::Flags3 => "3Flags",
            Self::TrafficLights3 => "3TrafficLights1",
            Self::TrafficLightsRim3 => "3TrafficLights2",
            Self::Signs3 => "3Signs",
            Self::SymbolsCircled3 => "3Symbols",
            Self::Symbols3 => "3Symbols2",
            Self::Stars3 => "3Stars",
            Self::Triangles3 => "3Triangles",
            Self::Arrows4 => "4Arrows",
            Self::ArrowsGray4 => "4ArrowsGray",
            Self::RedToBlack4 => "4RedToBlack",
            Self::Rating4 => "4Rating",
            Self::TrafficLights4 => "4TrafficLights",
            Self::Arrows5 => "5Arrows",
            Self::ArrowsGray5 => "5ArrowsGray",
            Self::Rating5 => "5Rating",
            Self::Quarters5 => "5Quarters",
            Self::Boxes5 => "5Boxes",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.key() == key)
    }

    pub fn default_thresholds(self) -> Vec<Threshold> {
        let n = self.count();
        (1..n)
            .map(|i| {
                let extra = if n == 3 && i == 2 { 1 } else { 0 };
                Threshold::Percent((100 * i / n + extra) as f64)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IconSet {
    pub kind: IconKind,
    pub reverse: bool,
    pub show_value: bool,
    pub thresholds: Vec<Threshold>,
}

impl IconSet {
    pub fn new(kind: IconKind) -> Self {
        Self {
            kind,
            reverse: false,
            show_value: true,
            thresholds: kind.default_thresholds(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    Greater(f64),
    Less(f64),
    Between(f64, f64),
    Equal(CellValue),
    TextContains(String),
    Duplicate,
    GreaterEqual(f64),
    LessEqual(f64),
    NotBetween(f64, f64),
    NotEqual(CellValue),
    TextNotContains(String),
    BeginsWith(String),
    EndsWith(String),
    Unique,
    Blanks,
    NoBlanks,
    Errors,
    NoErrors,
    Dates(DatePeriod),
    Top {
        count: u32,
        percent: bool,
        bottom: bool,
    },
    Average {
        below: bool,
        equal: bool,
        deviations: u8,
    },
    Expression(String),
    DataBar(DataBar),
    ColorScale(ColorScale),
    IconSet(IconSet),
}

impl Rule {
    pub fn is_graphic(&self) -> bool {
        matches!(
            self,
            Self::DataBar(_) | Self::ColorScale(_) | Self::IconSet(_)
        )
    }

    fn needs_stats(&self) -> bool {
        matches!(
            self,
            Self::Duplicate
                | Self::Unique
                | Self::Top { .. }
                | Self::Average { .. }
                | Self::DataBar(_)
                | Self::ColorScale(_)
                | Self::IconSet(_)
        )
    }

    fn may_match_empty(&self) -> bool {
        matches!(
            self,
            Self::Blanks
                | Self::NoErrors
                | Self::TextNotContains(_)
                | Self::NotEqual(_)
                | Self::Expression(_)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Highlight {
    pub fill: Option<Color>,
    pub color: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub border: Option<Border>,
}

impl Highlight {
    fn merge(&mut self, other: &Highlight) {
        self.fill = self.fill.or(other.fill);
        self.color = self.color.or(other.color);
        self.bold |= other.bold;
        self.italic |= other.italic;
        self.strike |= other.strike;
        self.border = self.border.or(other.border);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalFormat {
    pub range: CellRange,
    pub rule: Rule,
    pub format: Highlight,
    pub stop: bool,
}

impl ConditionalFormat {
    pub fn new(range: CellRange, rule: Rule, format: Highlight) -> Self {
        Self {
            range,
            rule,
            format,
            stop: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub start: f32,
    pub end: f32,
    pub axis: Option<f32>,
    pub color: Color,
    pub gradient: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Extras {
    pub bar: Option<Bar>,
    pub icon: Option<(IconKind, usize)>,
    pub hide_value: bool,
}

#[derive(Default)]
struct Stats {
    numbers: Vec<f64>,
    counts: HashMap<String, usize>,
    mean: f64,
    deviation: f64,
}

impl Stats {
    fn min(&self) -> f64 {
        self.numbers.first().copied().unwrap_or(0.0)
    }

    fn max(&self) -> f64 {
        self.numbers.last().copied().unwrap_or(0.0)
    }

    fn percentile(&self, p: f64) -> f64 {
        let n = self.numbers.len();
        if n == 0 {
            return 0.0;
        }
        let rank = (p / 100.0).clamp(0.0, 1.0) * (n - 1) as f64;
        let low = rank.floor() as usize;
        let high = rank.ceil() as usize;
        self.numbers[low] + (self.numbers[high] - self.numbers[low]) * (rank - low as f64)
    }

    fn resolve(&self, threshold: Threshold, automatic_low: bool) -> f64 {
        match threshold {
            Threshold::Automatic if automatic_low => self.min().min(0.0),
            Threshold::Automatic => self.max().max(0.0),
            Threshold::Lowest => self.min(),
            Threshold::Highest => self.max(),
            Threshold::Number(n) => n,
            Threshold::Percent(p) => self.min() + (self.max() - self.min()) * p / 100.0,
            Threshold::Percentile(p) => self.percentile(p),
        }
    }
}

fn key(value: &CellValue) -> Option<String> {
    match value {
        CellValue::Empty => None,
        CellValue::Number(n) => Some(format!("n{n}")),
        CellValue::Text(text) => Some(format!("t{}", fold(text))),
        CellValue::Bool(b) => Some(format!("b{b}")),
        CellValue::Error(e) => Some(format!("e{e:?}")),
    }
}

fn stats(book: &Workbook, sheet: usize, range: CellRange) -> Stats {
    let mut stats = Stats::default();
    for (_, cell) in book.sheets()[sheet].cells_in(range) {
        if let Some(key) = key(&cell.value) {
            *stats.counts.entry(key).or_insert(0) += 1;
        }
        if let CellValue::Number(n) = cell.value {
            stats.numbers.push(n);
        }
    }
    stats.numbers.sort_by(f64::total_cmp);
    let n = stats.numbers.len() as f64;
    if n > 0.0 {
        stats.mean = stats.numbers.iter().sum::<f64>() / n;
        if n > 1.0 {
            let squares: f64 = stats.numbers.iter().map(|x| (x - stats.mean).powi(2)).sum();
            stats.deviation = (squares / (n - 1.0)).sqrt();
        }
    }
    stats
}

fn text_of(value: &CellValue) -> String {
    match value {
        CellValue::Text(text) => fold(text),
        CellValue::Empty => String::new(),
        other => fold(&EN.display(other)),
    }
}

fn blend(a: Color, b: Color, t: f64) -> Color {
    let [ar, ag, ab] = a.rgb();
    let [br, bg, bb] = b.rgb();
    let mix = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u32;
    Color((mix(ar, br) << 16) | (mix(ag, bg) << 8) | mix(ab, bb))
}

struct Prepared<'a> {
    format: &'a ConditionalFormat,
    stats: Option<Stats>,
    expr: Option<Expr>,
}

pub struct Highlighter<'a> {
    book: &'a Workbook,
    sheet: usize,
    today: f64,
    rules: Vec<Prepared<'a>>,
}

impl<'a> Highlighter<'a> {
    pub fn new(book: &'a Workbook, sheet: usize, visible: CellRange, today: f64) -> Self {
        let rules = book.sheets()[sheet]
            .layout()
            .conditional
            .iter()
            .rev()
            .filter(|format| overlaps(format.range, visible))
            .map(|format| Prepared {
                format,
                stats: format
                    .rule
                    .needs_stats()
                    .then(|| stats(book, sheet, format.range)),
                expr: match &format.rule {
                    Rule::Expression(text) => formula::parse(text, &EN).ok().map(|expr| {
                        let expr = book.expand_names(&expr, sheet);
                        book.expand_tables(&expr, sheet, format.range.start())
                    }),
                    _ => None,
                },
            })
            .collect();
        Self {
            book,
            sheet,
            today,
            rules,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn empty_ranges(&self) -> Vec<CellRange> {
        self.rules
            .iter()
            .filter(|r| r.format.rule.may_match_empty())
            .map(|r| r.format.range)
            .collect()
    }

    pub fn highlight(&self, addr: CellAddr, value: &CellValue) -> Highlight {
        self.evaluate(addr, value).0
    }

    pub fn evaluate(&self, addr: CellAddr, value: &CellValue) -> (Highlight, Extras) {
        let mut result = Highlight::default();
        let mut extras = Extras::default();
        let number = match value {
            CellValue::Number(n) => Some(*n),
            _ => None,
        };
        for prepared in &self.rules {
            let format = prepared.format;
            if !format.range.contains(addr) {
                continue;
            }
            let stats = prepared.stats.as_ref();
            match &format.rule {
                Rule::DataBar(bar) => {
                    if let (Some(n), Some(stats), None) = (number, stats, extras.bar) {
                        extras.bar = Some(bar_for(bar, n, stats));
                        extras.hide_value |= !bar.show_value;
                    }
                }
                Rule::ColorScale(scale) => {
                    if let (Some(n), Some(stats), None) = (number, stats, result.fill) {
                        result.fill = scale_color(scale, n, stats);
                    }
                }
                Rule::IconSet(set) => {
                    if let (Some(n), Some(stats), None) = (number, stats, extras.icon) {
                        extras.icon = Some((set.kind, icon_index(set, n, stats)));
                        extras.hide_value |= !set.show_value;
                    }
                }
                _ if self.matches(prepared, addr, value) => {
                    result.merge(&format.format);
                    if format.stop {
                        break;
                    }
                }
                _ => {}
            }
        }
        (result, extras)
    }

    fn matches(&self, prepared: &Prepared, addr: CellAddr, value: &CellValue) -> bool {
        let number = match value {
            CellValue::Number(n) => Some(*n),
            _ => None,
        };
        let stats = prepared.stats.as_ref();
        let equal = |expected: &CellValue| match (value, expected) {
            (CellValue::Text(a), CellValue::Text(b)) => fold(a) == fold(b),
            (a, b) => a == b && !a.is_empty(),
        };
        match &prepared.format.rule {
            Rule::Greater(limit) => number.is_some_and(|n| n > *limit),
            Rule::Less(limit) => number.is_some_and(|n| n < *limit),
            Rule::GreaterEqual(limit) => number.is_some_and(|n| n >= *limit),
            Rule::LessEqual(limit) => number.is_some_and(|n| n <= *limit),
            Rule::Between(a, b) => number.is_some_and(|n| n >= a.min(*b) && n <= a.max(*b)),
            Rule::NotBetween(a, b) => number.is_some_and(|n| n < a.min(*b) || n > a.max(*b)),
            Rule::Equal(expected) => equal(expected),
            Rule::NotEqual(expected) => !equal(expected),
            Rule::TextContains(part) => {
                matches!(value, CellValue::Text(_)) && text_of(value).contains(&fold(part))
            }
            Rule::TextNotContains(part) => !text_of(value).contains(&fold(part)),
            Rule::BeginsWith(part) => text_of(value).starts_with(&fold(part)),
            Rule::EndsWith(part) => text_of(value).ends_with(&fold(part)),
            Rule::Duplicate | Rule::Unique => {
                let count = key(value)
                    .and_then(|k| stats?.counts.get(&k).copied())
                    .unwrap_or(0);
                if prepared.format.rule == Rule::Duplicate {
                    count > 1
                } else {
                    count == 1
                }
            }
            Rule::Blanks => text_of(value).trim().is_empty(),
            Rule::NoBlanks => !text_of(value).trim().is_empty(),
            Rule::Errors => matches!(value, CellValue::Error(_)),
            Rule::NoErrors => !matches!(value, CellValue::Error(_)),
            Rule::Dates(period) => match (number, period.span(self.today)) {
                (Some(n), Some((from, to))) => n.floor() >= from && n.floor() < to,
                _ => false,
            },
            Rule::Top {
                count,
                percent,
                bottom,
            } => {
                let (Some(n), Some(stats)) = (number, stats) else {
                    return false;
                };
                let total = stats.numbers.len();
                let take = if *percent {
                    ((total as f64 * f64::from(*count) / 100.0).floor() as usize).max(1)
                } else {
                    *count as usize
                };
                if take == 0 || total == 0 {
                    return false;
                }
                let take = take.min(total);
                if *bottom {
                    n <= stats.numbers[take - 1]
                } else {
                    n >= stats.numbers[total - take]
                }
            }
            Rule::Average {
                below,
                equal,
                deviations,
            } => {
                let (Some(n), Some(stats)) = (number, stats) else {
                    return false;
                };
                let offset = stats.deviation * f64::from(*deviations);
                match (below, equal) {
                    (false, false) => n > stats.mean + offset,
                    (false, true) => n >= stats.mean + offset,
                    (true, false) => n < stats.mean - offset,
                    (true, true) => n <= stats.mean - offset,
                }
            }
            Rule::Expression(_) => {
                let Some(expr) = &prepared.expr else {
                    return false;
                };
                let origin = prepared.format.range.start();
                let shifted = expr.shifted(
                    i64::from(addr.row()) - i64::from(origin.row()),
                    i64::from(addr.col()) - i64::from(origin.col()),
                );
                let evaluator = Evaluator {
                    book: self.book,
                    sheet: self.sheet,
                    locale: &EN,
                    at: addr,
                    today: self.today,
                    now: self.today,
                };
                match evaluator.evaluate(&shifted) {
                    CellValue::Bool(b) => b,
                    CellValue::Number(n) => n != 0.0,
                    _ => false,
                }
            }
            Rule::DataBar(_) | Rule::ColorScale(_) | Rule::IconSet(_) => false,
        }
    }
}

fn bar_for(bar: &DataBar, n: f64, stats: &Stats) -> Bar {
    let low = stats.resolve(bar.min, true);
    let high = stats.resolve(bar.max, false).max(low + f64::EPSILON);
    let fraction = |v: f64| ((v - low) / (high - low)).clamp(0.0, 1.0) as f32;
    let axis = (low < 0.0 && high > 0.0).then(|| fraction(0.0));
    let (start, end, color) = match axis {
        Some(axis) if n < 0.0 => (fraction(n), axis, Color(0xFF0000)),
        Some(axis) => (axis, fraction(n), bar.color),
        None => (0.0, fraction(n).max(0.02), bar.color),
    };
    Bar {
        start,
        end,
        axis,
        color,
        gradient: bar.gradient,
    }
}

fn scale_color(scale: &ColorScale, n: f64, stats: &Stats) -> Option<Color> {
    let points: Vec<(f64, Color)> = scale
        .points
        .iter()
        .enumerate()
        .map(|(i, (threshold, color))| (stats.resolve(*threshold, i == 0), *color))
        .collect();
    let (first, last) = (points.first()?, points.last()?);
    if n <= first.0 {
        return Some(first.1);
    }
    if n >= last.0 {
        return Some(last.1);
    }
    points.windows(2).find_map(|pair| {
        let ((a, ca), (b, cb)) = (pair[0], pair[1]);
        (n >= a && n <= b).then(|| {
            let t = if b > a { (n - a) / (b - a) } else { 0.0 };
            blend(ca, cb, t)
        })
    })
}

fn icon_index(set: &IconSet, n: f64, stats: &Stats) -> usize {
    let index = set
        .thresholds
        .iter()
        .filter(|t| n >= stats.resolve(**t, true))
        .count()
        .min(set.kind.count() - 1);
    if set.reverse {
        set.kind.count() - 1 - index
    } else {
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cell, Sheet, TR};

    fn book(rules: Vec<(&str, Rule, u32)>) -> Workbook {
        let mut sheet = Sheet::new("s");
        for (at, input) in [
            ("A1", "5"),
            ("A2", "12"),
            ("A3", "elma"),
            ("A4", "Elma"),
            ("A5", "12"),
            ("B1", "1"),
            ("B2", "2"),
            ("B3", "3"),
            ("B4", "4"),
            ("B5", "10"),
        ] {
            sheet.set(at.parse().unwrap(), Cell::parse_in(input, &TR).unwrap());
        }
        for (range, rule, color) in rules {
            sheet.layout_mut().conditional.push(ConditionalFormat::new(
                range.parse().unwrap(),
                rule,
                Highlight {
                    fill: Some(Color(color)),
                    ..Highlight::default()
                },
            ));
        }
        Workbook::from_sheets(vec![sheet]).unwrap()
    }

    fn eval(book: &Workbook, at: &str) -> (Highlight, Extras) {
        let addr: CellAddr = at.parse().unwrap();
        Highlighter::new(book, 0, "A1:Z99".parse().unwrap(), 45000.0)
            .evaluate(addr, book.sheets()[0].value(addr))
    }

    fn fill(book: &Workbook, at: &str) -> Option<u32> {
        eval(book, at).0.fill.map(|c| c.0)
    }

    #[test]
    fn number_rules() {
        let s = book(vec![("A1:A5", Rule::Greater(10.0), 1)]);
        assert_eq!(fill(&s, "A2"), Some(1));
        assert_eq!(fill(&s, "A1"), None);
        assert_eq!(fill(&s, "A3"), None);
        let s = book(vec![("A1:A5", Rule::Between(12.0, 1.0), 2)]);
        assert_eq!(fill(&s, "A1"), Some(2));
        let s = book(vec![("A1:A5", Rule::NotBetween(1.0, 6.0), 3)]);
        assert_eq!(fill(&s, "A1"), None);
        assert_eq!(fill(&s, "A2"), Some(3));
    }

    #[test]
    fn text_duplicate_and_blank_rules() {
        let s = book(vec![("A1:A5", Rule::TextContains("LM".into()), 4)]);
        assert_eq!(fill(&s, "A4"), Some(4));
        assert_eq!(fill(&s, "A2"), None);
        let s = book(vec![("A1:A5", Rule::Duplicate, 5)]);
        assert_eq!(fill(&s, "A3"), Some(5));
        assert_eq!(fill(&s, "A1"), None);
        let s = book(vec![("A1:A5", Rule::Unique, 5)]);
        assert_eq!(fill(&s, "A1"), Some(5));
        assert_eq!(fill(&s, "A2"), None);
        let s = book(vec![("A1:A5", Rule::BeginsWith("EL".into()), 6)]);
        assert_eq!(fill(&s, "A3"), Some(6));
        let s = book(vec![("A1:C9", Rule::Blanks, 7)]);
        assert_eq!(fill(&s, "C9"), Some(7));
        assert_eq!(fill(&s, "A1"), None);
        assert_eq!(
            Highlighter::new(&s, 0, "A1:C3".parse().unwrap(), 0.0).empty_ranges(),
            vec!["A1:C9".parse().unwrap()]
        );
    }

    #[test]
    fn top_average_and_expression_rules() {
        let top = |count, percent, bottom| Rule::Top {
            count,
            percent,
            bottom,
        };
        let s = book(vec![("B1:B5", top(2, false, false), 1)]);
        assert_eq!(fill(&s, "B5"), Some(1));
        assert_eq!(fill(&s, "B4"), Some(1));
        assert_eq!(fill(&s, "B3"), None);
        let s = book(vec![("B1:B5", top(20, true, true), 1)]);
        assert_eq!(fill(&s, "B1"), Some(1));
        assert_eq!(fill(&s, "B2"), None);
        let average = Rule::Average {
            below: false,
            equal: false,
            deviations: 0,
        };
        let s = book(vec![("B1:B5", average, 2)]);
        assert_eq!(fill(&s, "B5"), Some(2));
        assert_eq!(fill(&s, "B4"), None);
        let s = book(vec![("B1:B5", Rule::Expression("MOD(B1,2)=0".into()), 3)]);
        assert_eq!(fill(&s, "B2"), Some(3));
        assert_eq!(fill(&s, "B3"), None);
        assert_eq!(fill(&s, "B5"), Some(3));
    }

    #[test]
    fn stop_and_priority() {
        let mut s = book(vec![
            ("A1:A5", Rule::Greater(0.0), 1),
            ("A1:A5", Rule::Greater(10.0), 2),
        ]);
        assert_eq!(fill(&s, "A2"), Some(2));
        assert_eq!(fill(&s, "A1"), Some(1));
        s.sheet_at_mut(0).layout_mut().conditional[0].format.bold = true;
        assert!(eval(&s, "A2").0.bold);
        s.sheet_at_mut(0).layout_mut().conditional[1].stop = true;
        assert!(!eval(&s, "A2").0.bold);
    }

    #[test]
    fn graphics_follow_the_range() {
        let s = book(vec![(
            "B1:B5",
            Rule::DataBar(DataBar::new(Color(0x638EC6), true)),
            0,
        )]);
        let bar = eval(&s, "B5").1.bar.unwrap();
        assert_eq!((bar.start, bar.end), (0.0, 1.0));
        assert!((eval(&s, "B3").1.bar.unwrap().end - 0.3).abs() < 1e-6);
        let scale = ColorScale {
            points: vec![
                (Threshold::Lowest, Color(0x000000)),
                (Threshold::Highest, Color(0xFFFFFF)),
            ],
        };
        let s = book(vec![("B1:B5", Rule::ColorScale(scale), 0)]);
        assert_eq!(eval(&s, "B1").0.fill, Some(Color(0)));
        assert_eq!(eval(&s, "B5").0.fill, Some(Color(0xFFFFFF)));
        let s = book(vec![(
            "B1:B5",
            Rule::IconSet(IconSet::new(IconKind::Arrows3)),
            0,
        )]);
        assert_eq!(eval(&s, "B1").1.icon, Some((IconKind::Arrows3, 0)));
        assert_eq!(eval(&s, "B5").1.icon, Some((IconKind::Arrows3, 2)));
        assert_eq!(eval(&s, "B3").1.icon, Some((IconKind::Arrows3, 0)));
    }

    #[test]
    fn date_periods() {
        let today = date_serial(2026, 10, 22).unwrap();
        let within = |p: DatePeriod, y, m, d| {
            let (from, to) = p.span(today).unwrap();
            let n = date_serial(y, m, d).unwrap();
            n >= from && n < to
        };
        assert!(within(DatePeriod::Yesterday, 2026, 10, 21));
        assert!(within(DatePeriod::ThisWeek, 2026, 10, 18));
        assert!(!within(DatePeriod::ThisWeek, 2026, 10, 25));
        assert!(within(DatePeriod::LastMonth, 2026, 9, 30));
        assert!(within(DatePeriod::NextMonth, 2026, 11, 1));
        assert!(within(DatePeriod::Last7Days, 2026, 10, 16));
        assert!(!within(DatePeriod::Last7Days, 2026, 10, 15));
    }
}
