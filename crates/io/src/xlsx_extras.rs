use std::collections::BTreeSet;

use vellum_grid_core::{
    AutoFilter, CellAddr, CellRange, CellValue, Chart, ChartKind, ColorScale, ConditionalFormat,
    DataBar, DatePeriod, Grouping, Highlight, IconKind, IconSet, Layout, Legend, Margins, Paper,
    Rule, Scale, SparkKind, SparklineGroup, Threshold,
};

use vellum_grid_core::filter::{ColumnFilter, Condition, Dynamic, Operator};
use vellum_grid_core::validation::{AlertStyle, Allow, Compare, Validation};

use crate::xlsx_format::{Tag, attr, flag, name, walk};

const MAX_HIDDEN_SPAN: u32 = 1024;

#[derive(Default)]
struct PendingRule {
    kind: String,
    operator: Option<String>,
    text: Option<String>,
    dxf: Option<usize>,
    priority: i64,
    formulas: Vec<String>,
    stop: bool,
    period: Option<String>,
    rank: u32,
    percent: bool,
    bottom: bool,
    below: bool,
    equal: bool,
    deviations: u8,
    cfvos: Vec<(String, Option<String>)>,
    colors: Vec<vellum_grid_core::Color>,
    icon: Option<String>,
    reverse: bool,
    show_value: bool,
    ext_id: Option<String>,
}

pub fn sheet_extras(xml: &str, dxfs: &[Highlight], layout: &mut Layout) -> Option<String> {
    let mut drawing = None;
    let mut fit_to_page = false;
    let mut breaks_of_rows = true;
    let mut header_part: Option<bool> = None;
    let mut filter_col: Option<u32> = None;
    let mut values = BTreeSet::new();
    let mut ranges: Vec<CellRange> = Vec::new();
    let mut rule: Option<PendingRule> = None;
    let mut in_formula = false;
    let mut rules: Vec<(i64, ConditionalFormat, Option<String>)> = Vec::new();
    let mut ext_depth = 0usize;
    let mut in_id = false;
    let mut ext_rule: Option<String> = None;
    let mut solid: BTreeSet<String> = BTreeSet::new();
    let mut spark: Option<SparklineGroup> = None;
    let mut spark_text: Option<bool> = None;
    let mut spark_data: Option<CellRange> = None;
    let mut spark_cell: Option<CellAddr> = None;
    let mut validation: Option<Validation> = None;
    let mut column_filter: Option<ColumnFilter> = None;
    let mut validation_formula: Option<bool> = None;
    let mut finish_rule = |rule: PendingRule, ranges: &[CellRange]| {
        let Some(parsed) = build_rule(&rule) else {
            return;
        };
        let format = rule
            .dxf
            .and_then(|index| dxfs.get(index))
            .copied()
            .unwrap_or_default();
        for range in ranges {
            let mut conditional = ConditionalFormat::new(*range, parsed.clone(), format);
            conditional.stop = rule.stop;
            rules.push((rule.priority, conditional, rule.ext_id.clone()));
        }
    };
    walk(xml, |tag| {
        let (e, empty) = match tag {
            Tag::Open(e) => (e, false),
            Tag::Empty(e) => (e, true),
            Tag::Text(text) => {
                if let Some(formula) = spark_text {
                    if formula {
                        spark_data = parse_ref(&text);
                    } else {
                        spark_cell = text.trim().parse().ok();
                    }
                } else if let (Some(first), Some(validation)) =
                    (validation_formula, validation.as_mut())
                {
                    let target = if first {
                        &mut validation.formula1
                    } else {
                        &mut validation.formula2
                    };
                    target.push_str(&text);
                } else if in_formula && let Some(rule) = rule.as_mut() {
                    rule.formulas.push(text);
                } else if in_id && let Some(rule) = rule.as_mut() {
                    rule.ext_id = Some(text.trim().to_owned());
                } else if let Some(header) = header_part {
                    if header {
                        layout.page.header.push_str(&text);
                    } else {
                        layout.page.footer.push_str(&text);
                    }
                }
                return;
            }
            Tag::Close(tag) => {
                match tag.as_str() {
                    "extLst" => ext_depth = ext_depth.saturating_sub(1),
                    "f" | "sqref" => spark_text = None,
                    "sparkline" => {
                        if let (Some(group), Some(cell), Some(data)) =
                            (spark.as_mut(), spark_cell.take(), spark_data.take())
                        {
                            group.cells.push((cell, data));
                        }
                    }
                    "sparklineGroup" => {
                        if let Some(group) = spark.take().filter(|g| !g.cells.is_empty()) {
                            layout.sparklines.push(group);
                        }
                    }
                    "id" => in_id = false,
                    "formula" => in_formula = false,
                    "formula1" | "formula2" => validation_formula = None,
                    "dataValidation" => {
                        if let Some(done) = validation.take() {
                            layout.validations.push(done);
                        }
                    }
                    "oddHeader" | "oddFooter" => header_part = None,
                    "filterColumn" => {
                        let criteria = column_filter.take().or_else(|| {
                            (!values.is_empty())
                                .then(|| ColumnFilter::Values(std::mem::take(&mut values)))
                        });
                        if let (Some(col), Some(filter), Some(criteria)) =
                            (filter_col.take(), layout.filter.as_mut(), criteria)
                        {
                            let col = filter.range.start().col() + col;
                            filter.criteria.insert(col, criteria);
                        }
                    }
                    "cfRule" => {
                        if let Some(done) = rule.take() {
                            finish_rule(done, &ranges);
                        }
                    }
                    _ => {}
                }
                return;
            }
        };
        let number = |key: &str| attr(&e, key).and_then(|v| v.parse::<f64>().ok());
        let tag = name(&e);
        if tag == "extLst" && !empty {
            ext_depth += 1;
            return;
        }
        if ext_depth > 0 {
            match (tag.as_str(), rule.as_ref()) {
                ("id", Some(_)) => in_id = !empty,
                ("cfRule", None) => ext_rule = attr(&e, "id"),
                ("sparklineGroup", None) => {
                    let kind = match attr(&e, "type").as_deref() {
                        Some("column") => SparkKind::Column,
                        Some("stacked") => SparkKind::WinLoss,
                        _ => SparkKind::Line,
                    };
                    let mut group = SparklineGroup::new(kind, Vec::new());
                    group.markers = flag(&e, "markers");
                    group.high = flag(&e, "high");
                    group.low = flag(&e, "low");
                    group.first = flag(&e, "first");
                    group.last = flag(&e, "last");
                    group.negative = flag(&e, "negative");
                    spark = Some(group);
                }
                ("colorSeries", None) => {
                    if let (Some(group), Some(color)) = (
                        spark.as_mut(),
                        attr(&e, "rgb").and_then(|c| vellum_grid_core::Color::from_hex(&c)),
                    ) {
                        group.color = color;
                    }
                }
                ("colorHigh", None) => {
                    if let (Some(group), Some(color)) = (
                        spark.as_mut(),
                        attr(&e, "rgb").and_then(|c| vellum_grid_core::Color::from_hex(&c)),
                    ) {
                        group.point_color = color;
                    }
                }
                ("f", None) if spark.is_some() => spark_text = Some(true),
                ("sqref", None) if spark.is_some() => spark_text = Some(false),
                ("dataBar", None) => {
                    if let (Some(id), Some("0")) = (&ext_rule, attr(&e, "gradient").as_deref()) {
                        solid.insert(id.clone());
                    }
                }
                _ => {}
            }
            return;
        }
        match tag.as_str() {
            "dataValidation" => {
                let found = data_validation(&e);
                if empty {
                    layout.validations.extend(found);
                } else {
                    validation = found;
                }
            }
            "formula1" => validation_formula = Some(true),
            "formula2" => validation_formula = Some(false),
            "pane" => {
                if attr(&e, "state").is_some_and(|s| s.starts_with("frozen")) {
                    layout.freeze_cols = number("xSplit").unwrap_or(0.0) as u32;
                    layout.freeze_rows = number("ySplit").unwrap_or(0.0) as u32;
                }
            }
            "row" => {
                if let Some(row) = number("r").filter(|r| *r >= 1.0).map(|r| r as u32 - 1) {
                    if flag(&e, "hidden") {
                        layout.hidden_rows.insert(row);
                    }
                    if let Some(level) = number("outlineLevel").filter(|l| *l >= 1.0) {
                        layout.row_levels.insert(row, level.min(7.0) as u8);
                    }
                    if flag(&e, "collapsed") && row >= 1 {
                        layout.collapsed_rows.insert(row - 1);
                    }
                }
            }
            "col" => {
                if let (Some(min), Some(max)) = (number("min"), number("max"))
                    && min >= 1.0
                    && max >= min
                    && max - min < f64::from(MAX_HIDDEN_SPAN)
                {
                    let cols = (min as u32 - 1)..=(max as u32 - 1);
                    if flag(&e, "hidden") {
                        layout.hidden_cols.extend(cols.clone());
                    }
                    if let Some(level) = number("outlineLevel").filter(|l| *l >= 1.0) {
                        for col in cols {
                            layout.col_levels.insert(col, level.min(7.0) as u8);
                        }
                    }
                    if flag(&e, "collapsed") && min >= 2.0 {
                        layout.collapsed_cols.insert(min as u32 - 2);
                    }
                }
            }
            "autoFilter" => {
                if let Some(range) = attr(&e, "ref").and_then(|r| r.parse::<CellRange>().ok()) {
                    layout.filter = Some(AutoFilter {
                        range,
                        criteria: Default::default(),
                    });
                }
            }
            "filterColumn" => {
                filter_col = number("colId").map(|c| c as u32);
                values.clear();
                column_filter = None;
                if empty {
                    filter_col = None;
                }
            }
            "customFilters" => {
                column_filter = Some(ColumnFilter::Custom {
                    first: Condition {
                        op: Operator::Equal,
                        value: String::new(),
                    },
                    second: None,
                    and: flag(&e, "and"),
                });
            }
            "customFilter" => {
                let parsed = custom_condition(
                    attr(&e, "operator").as_deref(),
                    &attr(&e, "val").unwrap_or_default(),
                );
                if let Some(ColumnFilter::Custom { first, second, .. }) = column_filter.as_mut() {
                    if first.value.is_empty() && second.is_none() {
                        *first = parsed;
                    } else {
                        *second = Some(parsed);
                    }
                }
            }
            "top10" => {
                column_filter = Some(ColumnFilter::Top {
                    count: number("val").unwrap_or(10.0),
                    percent: flag(&e, "percent"),
                    bottom: attr(&e, "top").as_deref() == Some("0"),
                });
            }
            "dynamicFilter" => {
                column_filter = attr(&e, "type")
                    .and_then(|kind| dynamic(&kind))
                    .map(ColumnFilter::Dynamic);
            }
            "colorFilter" => {
                let highlight = number("dxfId")
                    .and_then(|id| dxfs.get(id as usize))
                    .copied()
                    .unwrap_or_default();
                column_filter = Some(if attr(&e, "cellColor").as_deref() == Some("0") {
                    ColumnFilter::FontColor(highlight.color)
                } else {
                    ColumnFilter::Fill(highlight.fill)
                });
            }
            "filters" => {
                if flag(&e, "blank") {
                    values.insert(String::new());
                }
            }
            "filter" => {
                if let Some(value) = attr(&e, "val") {
                    values.insert(value);
                }
            }
            "conditionalFormatting" => {
                ranges = attr(&e, "sqref")
                    .unwrap_or_default()
                    .split_whitespace()
                    .filter_map(|r| r.parse().ok())
                    .collect();
            }
            "cfRule" => {
                let pending = PendingRule {
                    kind: attr(&e, "type").unwrap_or_default(),
                    operator: attr(&e, "operator"),
                    text: attr(&e, "text"),
                    dxf: number("dxfId").map(|v| v as usize),
                    priority: number("priority").unwrap_or(0.0) as i64,
                    stop: flag(&e, "stopIfTrue"),
                    period: attr(&e, "timePeriod"),
                    rank: number("rank").unwrap_or(10.0) as u32,
                    percent: flag(&e, "percent"),
                    bottom: flag(&e, "bottom"),
                    below: attr(&e, "aboveAverage").as_deref() == Some("0"),
                    equal: flag(&e, "equalAverage"),
                    deviations: number("stdDev").unwrap_or(0.0) as u8,
                    show_value: true,
                    ..PendingRule::default()
                };
                if empty {
                    finish_rule(pending, &ranges);
                } else {
                    rule = Some(pending);
                }
            }
            "formula" => in_formula = !empty,
            "dataBar" | "iconSet" if rule.is_some() => {
                if let Some(rule) = rule.as_mut() {
                    rule.show_value = attr(&e, "showValue").as_deref() != Some("0");
                    rule.reverse = flag(&e, "reverse");
                    rule.icon = attr(&e, "iconSet")
                        .or_else(|| (tag == "iconSet").then(|| "3TrafficLights1".to_owned()));
                }
            }
            "cfvo" if rule.is_some() => {
                if let Some(rule) = rule.as_mut() {
                    rule.cfvos
                        .push((attr(&e, "type").unwrap_or_default(), attr(&e, "val")));
                }
            }
            "color" if rule.is_some() => {
                if let Some(rule) = rule.as_mut() {
                    let color = attr(&e, "rgb")
                        .and_then(|rgb| vellum_grid_core::Color::from_hex(&rgb))
                        .unwrap_or(vellum_grid_core::Color(0x638EC6));
                    rule.colors.push(color);
                }
            }
            "pageSetUpPr" => fit_to_page = flag(&e, "fitToPage"),
            "pageSetup" => {
                let page = &mut layout.page;
                if let Some(paper) = number("paperSize").and_then(|c| Paper::from_code(c as u32)) {
                    page.paper = paper;
                }
                page.landscape = attr(&e, "orientation").as_deref() == Some("landscape");
                page.scale = if fit_to_page {
                    Scale::Fit {
                        width: number("fitToWidth").unwrap_or(1.0) as u16,
                        height: number("fitToHeight").unwrap_or(1.0) as u16,
                    }
                } else {
                    Scale::Percent(number("scale").unwrap_or(100.0) as u16)
                };
            }
            "pageMargins" => {
                let margin = |key: &str, default: f64| number(key).unwrap_or(default);
                let normal = Margins::NORMAL;
                layout.page.margins = Margins {
                    left: margin("left", normal.left),
                    right: margin("right", normal.right),
                    top: margin("top", normal.top),
                    bottom: margin("bottom", normal.bottom),
                    header: margin("header", normal.header),
                    footer: margin("footer", normal.footer),
                };
            }
            "printOptions" => {
                let page = &mut layout.page;
                page.gridlines = flag(&e, "gridLines");
                page.headings = flag(&e, "headings");
                page.center_horizontally = flag(&e, "horizontalCentered");
                page.center_vertically = flag(&e, "verticalCentered");
            }
            "oddHeader" if !empty => header_part = Some(true),
            "oddFooter" if !empty => header_part = Some(false),
            "rowBreaks" => breaks_of_rows = true,
            "colBreaks" => breaks_of_rows = false,
            "brk" => {
                if let Some(id) = number("id") {
                    let breaks = if breaks_of_rows {
                        &mut layout.page.row_breaks
                    } else {
                        &mut layout.page.col_breaks
                    };
                    breaks.insert(id as u32);
                }
            }
            "drawing" => drawing = attr(&e, "id"),
            "tabColor" => {
                layout.tab_color =
                    attr(&e, "rgb").and_then(|rgb| vellum_grid_core::Color::from_hex(&rgb));
            }
            _ => {}
        }
    });
    rules.sort_by_key(|(priority, _, _)| std::cmp::Reverse(*priority));
    layout.conditional = rules
        .into_iter()
        .map(|(_, mut format, id)| {
            if let (Rule::DataBar(bar), Some(id)) = (&mut format.rule, id) {
                bar.gradient = !solid.contains(&id);
            }
            format
        })
        .collect();
    drawing
}

fn custom_condition(operator: Option<&str>, value: &str) -> Condition {
    let negated = operator == Some("notEqual");
    let starts = value.starts_with('*') && !value.starts_with("~*");
    let ends = value.ends_with('*') && !value.ends_with("~*") && value.len() > 1;
    let inner = value.trim_start_matches('*').trim_end_matches('*');
    let plain = !inner.contains(['*', '?']);
    let (op, value) = match operator {
        Some("greaterThan") => (Operator::Greater, value),
        Some("greaterThanOrEqual") => (Operator::GreaterOrEqual, value),
        Some("lessThan") => (Operator::Less, value),
        Some("lessThanOrEqual") => (Operator::LessOrEqual, value),
        _ if plain && starts && ends => (
            if negated {
                Operator::NotContains
            } else {
                Operator::Contains
            },
            inner,
        ),
        _ if plain && ends => (
            if negated {
                Operator::NotBeginsWith
            } else {
                Operator::BeginsWith
            },
            inner,
        ),
        _ if plain && starts => (
            if negated {
                Operator::NotEndsWith
            } else {
                Operator::EndsWith
            },
            inner,
        ),
        _ if negated => (Operator::NotEqual, value),
        _ => (Operator::Equal, value),
    };
    Condition {
        op,
        value: value.to_owned(),
    }
}

fn dynamic(kind: &str) -> Option<Dynamic> {
    Some(match kind {
        "aboveAverage" => Dynamic::AboveAverage,
        "belowAverage" => Dynamic::BelowAverage,
        "tomorrow" => Dynamic::Tomorrow,
        "today" => Dynamic::Today,
        "yesterday" => Dynamic::Yesterday,
        "nextWeek" => Dynamic::NextWeek,
        "thisWeek" => Dynamic::ThisWeek,
        "lastWeek" => Dynamic::LastWeek,
        "nextMonth" => Dynamic::NextMonth,
        "thisMonth" => Dynamic::ThisMonth,
        "lastMonth" => Dynamic::LastMonth,
        "nextQuarter" => Dynamic::NextQuarter,
        "thisQuarter" => Dynamic::ThisQuarter,
        "lastQuarter" => Dynamic::LastQuarter,
        "nextYear" => Dynamic::NextYear,
        "thisYear" => Dynamic::ThisYear,
        "lastYear" => Dynamic::LastYear,
        "yearToDate" => Dynamic::YearToDate,
        _ => return None,
    })
}

fn data_validation(e: &quick_xml::events::BytesStart) -> Option<Validation> {
    let ranges: Vec<CellRange> = attr(e, "sqref")?
        .split_whitespace()
        .filter_map(|part| part.replace('$', "").parse().ok())
        .collect();
    if ranges.is_empty() {
        return None;
    }
    let allow = match attr(e, "type").as_deref() {
        Some("whole") => Allow::Whole,
        Some("decimal") => Allow::Decimal,
        Some("list") => Allow::List,
        Some("date") => Allow::Date,
        Some("time") => Allow::Time,
        Some("textLength") => Allow::TextLength,
        Some("custom") => Allow::Custom,
        _ => Allow::Any,
    };
    let compare = match attr(e, "operator").as_deref() {
        Some("notBetween") => Compare::NotBetween,
        Some("equal") => Compare::Equal,
        Some("notEqual") => Compare::NotEqual,
        Some("greaterThan") => Compare::Greater,
        Some("lessThan") => Compare::Less,
        Some("greaterThanOrEqual") => Compare::GreaterOrEqual,
        Some("lessThanOrEqual") => Compare::LessOrEqual,
        _ => Compare::Between,
    };
    let style = match attr(e, "errorStyle").as_deref() {
        Some("warning") => AlertStyle::Warning,
        Some("information") => AlertStyle::Information,
        _ => AlertStyle::Stop,
    };
    Some(Validation {
        ranges,
        allow,
        compare,
        formula1: String::new(),
        formula2: String::new(),
        ignore_blank: flag(e, "allowBlank"),
        dropdown: !flag(e, "showDropDown"),
        show_input: flag(e, "showInputMessage"),
        input_title: attr(e, "promptTitle").unwrap_or_default(),
        input_message: attr(e, "prompt").unwrap_or_default(),
        show_error: flag(e, "showErrorMessage"),
        style,
        error_title: attr(e, "errorTitle").unwrap_or_default(),
        error_message: attr(e, "error").unwrap_or_default(),
    })
}

fn threshold(cfvo: &(String, Option<String>), bar: bool) -> Threshold {
    let value = cfvo
        .1
        .as_deref()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    match cfvo.0.as_str() {
        "min" | "max" | "autoMin" | "autoMax" if bar => Threshold::Automatic,
        "min" => Threshold::Lowest,
        "max" => Threshold::Highest,
        "percent" => Threshold::Percent(value),
        "percentile" => Threshold::Percentile(value),
        _ => Threshold::Number(value),
    }
}

fn build_rule(rule: &PendingRule) -> Option<Rule> {
    let number = |index: usize| rule.formulas.get(index)?.trim().parse::<f64>().ok();
    let literal = |index: usize| {
        let formula = rule.formulas.get(index)?.trim();
        Some(match formula.parse::<f64>() {
            Ok(n) => CellValue::Number(n),
            Err(_) => CellValue::Text(formula.trim_matches('"').replace("\"\"", "\"")),
        })
    };
    let text = || rule.text.clone();
    Some(match rule.kind.as_str() {
        "cellIs" => match rule.operator.as_deref()? {
            "greaterThan" => Rule::Greater(number(0)?),
            "lessThan" => Rule::Less(number(0)?),
            "greaterThanOrEqual" => Rule::GreaterEqual(number(0)?),
            "lessThanOrEqual" => Rule::LessEqual(number(0)?),
            "between" => Rule::Between(number(0)?, number(1)?),
            "notBetween" => Rule::NotBetween(number(0)?, number(1)?),
            "equal" => Rule::Equal(literal(0)?),
            "notEqual" => Rule::NotEqual(literal(0)?),
            _ => return None,
        },
        "containsText" => Rule::TextContains(text()?),
        "notContainsText" => Rule::TextNotContains(text()?),
        "beginsWith" => Rule::BeginsWith(text()?),
        "endsWith" => Rule::EndsWith(text()?),
        "duplicateValues" => Rule::Duplicate,
        "uniqueValues" => Rule::Unique,
        "containsBlanks" => Rule::Blanks,
        "notContainsBlanks" => Rule::NoBlanks,
        "containsErrors" => Rule::Errors,
        "notContainsErrors" => Rule::NoErrors,
        "timePeriod" => Rule::Dates(DatePeriod::from_key(rule.period.as_deref()?)?),
        "top10" => Rule::Top {
            count: rule.rank,
            percent: rule.percent,
            bottom: rule.bottom,
        },
        "aboveAverage" => Rule::Average {
            below: rule.below,
            equal: rule.equal,
            deviations: rule.deviations,
        },
        "expression" => Rule::Expression(rule.formulas.first()?.trim().to_owned()),
        "dataBar" => {
            let mut bar = DataBar::new(*rule.colors.first()?, true);
            bar.min = threshold(rule.cfvos.first()?, true);
            bar.max = threshold(rule.cfvos.get(1)?, true);
            bar.show_value = rule.show_value;
            Rule::DataBar(bar)
        }
        "colorScale" => Rule::ColorScale(ColorScale {
            points: rule
                .cfvos
                .iter()
                .zip(&rule.colors)
                .map(|(cfvo, color)| (threshold(cfvo, false), *color))
                .collect(),
        }),
        "iconSet" => {
            let kind = IconKind::from_key(rule.icon.as_deref()?)?;
            Rule::IconSet(IconSet {
                kind,
                reverse: rule.reverse,
                show_value: rule.show_value,
                thresholds: rule
                    .cfvos
                    .iter()
                    .skip(1)
                    .map(|cfvo| threshold(cfvo, false))
                    .collect(),
            })
        }
        _ => return None,
    })
}

pub fn parse_chart(
    xml: &str,
    anchor: &crate::xlsx_drawing::Anchor,
    layout: &Layout,
) -> Option<Chart> {
    let mut kind = None;
    let mut grouping = Grouping::Clustered;
    let mut legend = Legend::Hidden;
    let mut labels = false;
    let mut gridlines = false;
    let mut markers = true;
    let mut refs: Vec<CellRange> = Vec::new();
    let mut title = String::new();
    let (mut x_title, mut y_title) = (String::new(), String::new());
    let mut axis: Option<(String, String)> = None;
    let (mut in_ser, mut in_title, mut in_text, mut in_ref) = (false, false, false, false);
    let mut in_labels = false;
    walk(xml, |tag| match tag {
        Tag::Open(e) | Tag::Empty(e) => match name(&e).as_str() {
            "barChart" | "bar3DChart" if kind.is_none() => kind = Some(ChartKind::Column),
            "barDir" => {
                if attr(&e, "val").as_deref() == Some("bar") {
                    kind = Some(ChartKind::Bar);
                }
            }
            "lineChart" | "line3DChart" if kind.is_none() => kind = Some(ChartKind::Line),
            "areaChart" | "area3DChart" if kind.is_none() => kind = Some(ChartKind::Area),
            "pieChart" | "pie3DChart" | "ofPieChart" if kind.is_none() => {
                kind = Some(ChartKind::Pie);
            }
            "doughnutChart" if kind.is_none() => kind = Some(ChartKind::Doughnut),
            "scatterChart" if kind.is_none() => kind = Some(ChartKind::Scatter),
            "grouping" => {
                grouping = match attr(&e, "val").as_deref() {
                    Some("stacked") => Grouping::Stacked,
                    Some("percentStacked") => Grouping::Percent,
                    _ => grouping,
                }
            }
            "legend" => legend = Legend::Right,
            "legendPos" => {
                legend = match attr(&e, "val").as_deref() {
                    Some("b") => Legend::Bottom,
                    Some("t") => Legend::Top,
                    Some("l") => Legend::Left,
                    _ => Legend::Right,
                }
            }
            "dLbls" => in_labels = true,
            "showVal" if in_labels && attr(&e, "val").as_deref() == Some("1") => labels = true,
            "symbol" if in_ser && attr(&e, "val").as_deref() == Some("none") => markers = false,
            "catAx" | "dateAx" | "valAx" | "serAx" => axis = Some((name(&e), String::new())),
            "axPos" => {
                if let Some((_, pos)) = axis.as_mut() {
                    *pos = attr(&e, "val").unwrap_or_default();
                }
            }
            "majorGridlines" if axis.is_some() => gridlines = true,
            "ser" => in_ser = true,
            "title" if !in_ser => in_title = true,
            "t" if in_title => in_text = true,
            "f" if in_ser => in_ref = true,
            _ => {}
        },
        Tag::Text(text) => {
            if in_text {
                let target = match &axis {
                    None => &mut title,
                    Some((element, pos)) => {
                        let horizontal = element != "valAx"
                            || (kind == Some(ChartKind::Scatter)
                                && matches!(pos.as_str(), "b" | "t"));
                        if horizontal {
                            &mut x_title
                        } else {
                            &mut y_title
                        }
                    }
                };
                target.push_str(&text);
            } else if in_ref && let Some(range) = parse_ref(&text) {
                refs.push(range);
            }
        }
        Tag::Close(tag) => match tag.as_str() {
            "ser" => in_ser = false,
            "title" => in_title = false,
            "t" => in_text = false,
            "f" => in_ref = false,
            "dLbls" => in_labels = false,
            "catAx" | "dateAx" | "valAx" | "serAx" => axis = None,
            _ => {}
        },
    });
    let text = |value: &str| (!value.trim().is_empty()).then(|| value.trim().to_owned());
    let data = refs
        .iter()
        .copied()
        .reduce(|a, b| a.including(b.start()).including(b.end()))?;
    let (anchor_cell, offset, size) = crate::xlsx_drawing::placement(anchor, layout)?;
    let kind = kind?;
    let mut chart = Chart::new(kind, data, anchor_cell);
    chart.title = text(&title);
    chart.offset = offset;
    chart.size = size;
    chart.grouping = if kind.stacks() {
        grouping
    } else {
        Grouping::Clustered
    };
    chart.legend = legend;
    chart.labels = labels;
    chart.x_title = text(&x_title);
    chart.y_title = text(&y_title);
    chart.gridlines = gridlines;
    chart.markers = kind == ChartKind::Line && markers;
    Some(chart)
}

fn parse_ref(formula: &str) -> Option<CellRange> {
    let local = formula.rsplit('!').next()?.replace('$', "");
    local.trim().parse().ok()
}

pub fn resolve(base: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_owned();
    }
    let mut parts: Vec<&str> = base.split('/').collect();
    parts.pop();
    for piece in target.split('/') {
        match piece {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            piece => parts.push(piece),
        }
    }
    parts.join("/")
}

pub fn print_names(text: &str, layout: &mut Layout, titles: bool) {
    for part in text.split(',') {
        let reference = part
            .rsplit_once('!')
            .map_or(part, |(_, r)| r)
            .replace('$', "");
        if !titles {
            layout.page.print_area = reference.parse().ok();
            return;
        }
        let Some((first, last)) = reference.split_once(':') else {
            continue;
        };
        if first.chars().all(|c| c.is_ascii_digit()) {
            let row = |s: &str| s.parse::<u32>().ok().filter(|r| *r >= 1).map(|r| r - 1);
            if let (Some(a), Some(b)) = (row(first), row(last)) {
                layout.page.title_rows = Some((a, b));
            }
        } else if let (Ok(a), Ok(b)) = (
            vellum_grid_core::column_index(first),
            vellum_grid_core::column_index(last),
        ) {
            layout.page.title_cols = Some((a, b));
        }
    }
}

pub fn rels_path(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{path}.rels"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_panes_hidden_rows_filters_and_rules() {
        let xml = r#"<worksheet>
            <sheetViews><sheetView><pane xSplit="1" ySplit="2" topLeftCell="B3" state="frozen"/></sheetView></sheetViews>
            <cols><col min="3" max="4" width="9" hidden="1"/></cols>
            <sheetData><row r="5" hidden="1"><c r="A5"/></row></sheetData>
            <autoFilter ref="A1:C9"><filterColumn colId="1"><filters blank="1"><filter val="Ankara"/></filters></filterColumn></autoFilter>
            <conditionalFormatting sqref="B2:B9"><cfRule type="cellIs" dxfId="1" priority="2" operator="between"><formula>5</formula><formula>10</formula></cfRule></conditionalFormatting>
            <conditionalFormatting sqref="C2:C9 E1"><cfRule type="duplicateValues" dxfId="0" priority="1"/></conditionalFormatting>
            <drawing r:id="rId7"/>
        </worksheet>"#;
        let dxfs = [
            Highlight {
                fill: Some(vellum_grid_core::Color(0xFFC7CE)),
                ..Highlight::default()
            },
            Highlight::default(),
        ];
        let mut layout = Layout::default();
        let drawing = sheet_extras(xml, &dxfs, &mut layout);
        assert_eq!(drawing.as_deref(), Some("rId7"));
        assert_eq!((layout.freeze_rows, layout.freeze_cols), (2, 1));
        assert!(layout.hidden_cols.contains(&2) && layout.hidden_cols.contains(&3));
        assert!(layout.hidden_rows.contains(&4));
        let filter = layout.filter.unwrap();
        let ColumnFilter::Values(shown) = &filter.criteria[&1] else {
            panic!("value filter expected");
        };
        let shown: Vec<&str> = shown.iter().map(String::as_str).collect();
        assert_eq!(shown, ["", "Ankara"]);
        assert_eq!(layout.conditional.len(), 3);
        assert_eq!(layout.conditional[0].rule, Rule::Between(5.0, 10.0));
        assert_eq!(layout.conditional[2].rule, Rule::Duplicate);
        assert_eq!(layout.conditional[2].format, dxfs[0]);
    }

    #[test]
    fn reads_chart_anchor_and_series() {
        let drawing = r#"<xdr:wsDr><xdr:twoCellAnchor editAs="oneCell">
            <xdr:from><xdr:col>4</xdr:col><xdr:colOff>9525</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
            <xdr:to><xdr:col>11</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>15</xdr:row><xdr:rowOff>76200</xdr:rowOff></xdr:to>
            <xdr:graphicFrame><xdr:xfrm><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData><c:chart r:id="rId1"/></a:graphicData></a:graphic></xdr:graphicFrame>
        </xdr:twoCellAnchor></xdr:wsDr>"#;
        let anchors = crate::xlsx_drawing::drawing_anchors(drawing, &[]);
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].chart.as_deref(), Some("rId1"));
        let chart = r#"<c:chartSpace><c:chart><c:title><c:tx><c:rich><a:p><a:r><a:t>Satış</a:t></a:r></a:p></c:rich></c:tx></c:title>
            <c:plotArea><c:barChart><c:barDir val="bar"/>
            <c:ser><c:tx><c:strRef><c:f>'Özet Sayfa'!$B$1</c:f></c:strRef></c:tx>
            <c:cat><c:strRef><c:f>'Özet Sayfa'!$A$2:$A$7</c:f></c:strRef></c:cat>
            <c:val><c:numRef><c:f>'Özet Sayfa'!$B$2:$B$7</c:f></c:numRef></c:val></c:ser>
            </c:barChart></c:plotArea></c:chart></c:chartSpace>"#;
        let parsed = parse_chart(chart, &anchors[0], &Layout::default()).unwrap();
        assert_eq!(parsed.kind, ChartKind::Bar);
        assert_eq!(parsed.data, "A1:B7".parse().unwrap());
        assert_eq!(parsed.title.as_deref(), Some("Satış"));
        assert_eq!(parsed.anchor, "E2".parse().unwrap());
        assert_eq!(parsed.offset, (1, 0));
        assert_eq!(parsed.size, (447, 288));
    }

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(
            resolve("xl/worksheets/sheet1.xml", "../drawings/drawing1.xml"),
            "xl/drawings/drawing1.xml"
        );
        assert_eq!(
            resolve("xl/workbook.xml", "/xl/charts/chart1.xml"),
            "xl/charts/chart1.xml"
        );
        assert_eq!(
            rels_path("xl/worksheets/sheet1.xml"),
            "xl/worksheets/_rels/sheet1.xml.rels"
        );
    }
}
