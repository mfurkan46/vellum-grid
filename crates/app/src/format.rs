use vellum_grid_core::{
    Border, Borders, Cell, CellAddr, CellRange, CellValue, Color, DatePeriod, GENERAL, Highlight,
    LineStyle, Locale, Rule, Sheet, Style, numfmt,
};

use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderKind {
    Bottom,
    Top,
    Left,
    Right,
    None,
    All,
    Outside,
    ThickOutside,
    DoubleBottom,
    ThickBottom,
    TopBottom,
}

impl BorderKind {
    pub const MENU: [Self; 11] = [
        Self::Bottom,
        Self::Top,
        Self::Left,
        Self::Right,
        Self::None,
        Self::All,
        Self::Outside,
        Self::ThickOutside,
        Self::DoubleBottom,
        Self::ThickBottom,
        Self::TopBottom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bottom => text::bottom_border(),
            Self::Top => text::top_border(),
            Self::Left => text::left_border(),
            Self::Right => text::right_border(),
            Self::None => text::no_border(),
            Self::All => text::all_borders(),
            Self::Outside => text::outside_borders(),
            Self::ThickOutside => text::thick_outside_borders(),
            Self::DoubleBottom => text::double_bottom_border(),
            Self::ThickBottom => text::thick_bottom_border(),
            Self::TopBottom => text::top_bottom_border(),
        }
    }

    pub fn apply(self, range: CellRange, addr: CellAddr, borders: &mut Borders) {
        let top = addr.row() == range.start().row();
        let bottom = addr.row() == range.end().row();
        let left = addr.col() == range.start().col();
        let right = addr.col() == range.end().col();
        let line = |style| {
            Some(Border {
                style,
                color: Color::BLACK,
            })
        };
        let thin = line(LineStyle::Thin);
        let mut edges = |style: Option<Border>, sides: [bool; 4]| {
            let [t, r, b, l] = sides;
            if t {
                borders.top = style;
            }
            if r {
                borders.right = style;
            }
            if b {
                borders.bottom = style;
            }
            if l {
                borders.left = style;
            }
        };
        match self {
            Self::Bottom => edges(thin, [false, false, bottom, false]),
            Self::Top => edges(thin, [top, false, false, false]),
            Self::Left => edges(thin, [false, false, false, left]),
            Self::Right => edges(thin, [false, right, false, false]),
            Self::None => edges(None, [true; 4]),
            Self::All => edges(thin, [true; 4]),
            Self::Outside => edges(thin, [top, right, bottom, left]),
            Self::ThickOutside => edges(line(LineStyle::Thick), [top, right, bottom, left]),
            Self::DoubleBottom => edges(line(LineStyle::Double), [false, false, bottom, false]),
            Self::ThickBottom => edges(line(LineStyle::Thick), [false, false, bottom, false]),
            Self::TopBottom => edges(thin, [top, false, bottom, false]),
        }
        if self == Self::None {
            borders.diagonal = None;
            borders.diagonal_up = false;
            borders.diagonal_down = false;
        }
    }
}

pub const QUICK_FORMATS: [(text::Label, &str); 9] = [
    (text::format_general, GENERAL),
    (text::format_number, numfmt::NUMBER),
    (text::format_currency, numfmt::CURRENCY),
    (text::format_short_date, numfmt::DATE),
    (text::format_long_date, numfmt::LONG_DATE),
    (text::format_time, numfmt::TIME_SECONDS),
    (text::format_percent, numfmt::PERCENT_DECIMAL),
    (text::format_scientific, numfmt::SCIENTIFIC),
    (text::format_text, numfmt::TEXT),
];

pub fn format_name(code: &str) -> &'static str {
    QUICK_FORMATS
        .iter()
        .find(|(_, quick)| quick.eq_ignore_ascii_case(code))
        .map_or_else(
            || NumberFormat::new(code).category.label(),
            |(name, _)| name(),
        )
}

pub fn step_decimals(code: &str, value: &CellValue, locale: &Locale, delta: i32) -> String {
    if !code.eq_ignore_ascii_case(GENERAL) {
        return numfmt::adjust_decimals(code, delta);
    }
    let shown = match value {
        CellValue::Number(n) => locale.format_number(*n),
        _ => String::new(),
    };
    let decimals = if shown.contains('E') {
        0
    } else {
        shown
            .split_once(locale.decimal)
            .map_or(0, |(_, fraction)| fraction.len())
    };
    let base = if decimals == 0 {
        "0".to_owned()
    } else {
        format!("0.{}", "0".repeat(decimals))
    };
    numfmt::adjust_decimals(&base, delta)
}

pub fn step_size(size: u16, sizes: &[u16], grow: bool) -> u16 {
    let next = if grow {
        sizes.iter().map(|s| s * 10).find(|s| *s > size)
    } else {
        sizes.iter().rev().map(|s| s * 10).find(|s| *s < size)
    };
    next.unwrap_or(size)
}

pub fn sum_source(sheet: &Sheet, at: CellAddr) -> Option<CellRange> {
    let numeric = |row: u32, col: u32| {
        CellAddr::new(row, col)
            .ok()
            .and_then(|addr| sheet.get(addr))
            .is_some_and(|cell| matches!(cell.value, CellValue::Number(_)))
    };
    let (row, col) = (at.row(), at.col());
    let span = |start: CellAddr, end: CellAddr| Some(CellRange::new(start, end));
    if row > 0 && numeric(row - 1, col) {
        let mut first = row - 1;
        while first > 0 && numeric(first - 1, col) {
            first -= 1;
        }
        return span(
            CellAddr::new(first, col).ok()?,
            CellAddr::new(row - 1, col).ok()?,
        );
    }
    if col > 0 && numeric(row, col - 1) {
        let mut first = col - 1;
        while first > 0 && numeric(row, first - 1) {
            first -= 1;
        }
        return span(
            CellAddr::new(row, first).ok()?,
            CellAddr::new(row, col - 1).ok()?,
        );
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    Greater,
    Less,
    Between,
    Equal,
    Contains,
    Date,
    Duplicate,
    Top,
    TopPercent,
    Bottom,
    BottomPercent,
    AboveAverage,
    BelowAverage,
}

impl RuleKind {
    pub const HIGHLIGHT: [Self; 7] = [
        Self::Greater,
        Self::Less,
        Self::Between,
        Self::Equal,
        Self::Contains,
        Self::Date,
        Self::Duplicate,
    ];

    pub const TOP_BOTTOM: [Self; 6] = [
        Self::Top,
        Self::TopPercent,
        Self::Bottom,
        Self::BottomPercent,
        Self::AboveAverage,
        Self::BelowAverage,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Greater => text::rule_greater(),
            Self::Less => text::rule_less(),
            Self::Between => text::rule_between(),
            Self::Equal => text::rule_equal(),
            Self::Contains => text::rule_contains(),
            Self::Date => text::rule_date(),
            Self::Duplicate => text::rule_duplicate(),
            Self::Top => text::rule_top(),
            Self::TopPercent => text::rule_top_percent(),
            Self::Bottom => text::rule_bottom(),
            Self::BottomPercent => text::rule_bottom_percent(),
            Self::AboveAverage => text::rule_above_average(),
            Self::BelowAverage => text::rule_below_average(),
        }
    }

    pub fn prompt(self) -> &'static str {
        match self {
            Self::Greater => text::prompt_greater(),
            Self::Less => text::prompt_less(),
            Self::Between => text::prompt_between(),
            Self::Equal => text::prompt_equal(),
            Self::Contains => text::prompt_contains(),
            Self::Date => text::prompt_date(),
            Self::Duplicate => text::prompt_duplicate(),
            Self::Top | Self::TopPercent => text::prompt_top(),
            Self::Bottom | Self::BottomPercent => text::prompt_bottom(),
            Self::AboveAverage => text::prompt_above_average(),
            Self::BelowAverage => text::prompt_below_average(),
        }
    }

    pub fn inputs(self) -> usize {
        match self {
            Self::Between => 2,
            Self::Duplicate | Self::Date | Self::AboveAverage | Self::BelowAverage => 0,
            _ => 1,
        }
    }

    pub fn initial(self) -> Option<&'static str> {
        matches!(
            self,
            Self::Top | Self::TopPercent | Self::Bottom | Self::BottomPercent
        )
        .then_some("10")
    }

    pub fn rule(self, first: &str, second: &str, period: usize, locale: &Locale) -> Option<Rule> {
        let number = |text: &str| locale.parse_number(text.trim());
        let count = || {
            number(first)
                .filter(|n| n.fract() == 0.0 && (1.0..=1000.0).contains(n))
                .map(|n| n as u32)
        };
        let top = |percent: bool, bottom: bool| -> Option<Rule> {
            Some(Rule::Top {
                count: count()?,
                percent,
                bottom,
            })
        };
        Some(match self {
            Self::Greater => Rule::Greater(number(first)?),
            Self::Less => Rule::Less(number(first)?),
            Self::Between => Rule::Between(number(first)?, number(second)?),
            Self::Equal => Rule::Equal(
                Cell::parse_in(first, locale)
                    .ok()
                    .filter(|cell| cell.formula.is_none() && !cell.value.is_empty())?
                    .value,
            ),
            Self::Contains if !first.is_empty() => Rule::TextContains(first.to_owned()),
            Self::Contains => return None,
            Self::Date => Rule::Dates(*DatePeriod::ALL.get(period)?),
            Self::Duplicate => Rule::Duplicate,
            Self::Top => top(false, false)?,
            Self::TopPercent => top(true, false)?,
            Self::Bottom => top(false, true)?,
            Self::BottomPercent => top(true, true)?,
            Self::AboveAverage | Self::BelowAverage => Rule::Average {
                below: self == Self::BelowAverage,
                equal: false,
                deviations: 0,
            },
        })
    }
}

const fn wrap(color: Option<u32>) -> Option<Color> {
    match color {
        Some(c) => Some(Color(c)),
        None => None,
    }
}

const fn preset(fill: Option<u32>, color: Option<u32>, border: Option<u32>) -> Highlight {
    Highlight {
        fill: wrap(fill),
        color: wrap(color),
        bold: false,
        italic: false,
        strike: false,
        border: match border {
            Some(c) => Some(Border {
                style: LineStyle::Thin,
                color: Color(c),
            }),
            None => None,
        },
    }
}

pub const PRESETS: [(text::Label, Highlight); 6] = [
    (
        text::preset_red,
        preset(Some(0xFFC7CE), Some(0x9C0006), None),
    ),
    (
        text::preset_yellow,
        preset(Some(0xFFEB9C), Some(0x9C5700), None),
    ),
    (
        text::preset_green,
        preset(Some(0xC6EFCE), Some(0x006100), None),
    ),
    (text::preset_red_fill, preset(Some(0xFFC7CE), None, None)),
    (text::preset_red_text, preset(None, Some(0x9C0006), None)),
    (text::preset_red_border, preset(None, None, Some(0xC00000))),
];

const ACCENTS: [u32; 6] = [0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47];
const TITLE_TEXT: Color = Color(0x44546A);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStyle {
    Normal,
    Bad,
    Good,
    Neutral,
    Calculation,
    CheckCell,
    Explanatory,
    Input,
    LinkedCell,
    Note,
    Output,
    Warning,
    Title,
    Heading(u8),
    Total,
    Accent(u8, u8),
    Comma,
    CommaWhole,
    Currency,
    CurrencyWhole,
    Percent,
}

struct Spec {
    font: Option<(u16, bool, bool, Option<Color>)>,
    fill: Option<Option<Color>>,
    borders: Option<Borders>,
    number: Option<&'static str>,
}

fn all_sides(style: LineStyle, color: Color) -> Borders {
    let side = Some(Border { style, color });
    Borders {
        top: side,
        right: side,
        bottom: side,
        left: side,
        ..Borders::default()
    }
}

fn bottom(style: LineStyle, color: Color) -> Borders {
    Borders {
        bottom: Some(Border { style, color }),
        ..Borders::default()
    }
}

impl CellStyle {
    pub fn groups() -> Vec<(&'static str, Vec<Self>)> {
        let accents = (0..4)
            .flat_map(|level| (1..=6).map(move |accent| Self::Accent(accent, level)))
            .collect();
        vec![
            (
                text::styles_good_bad(),
                vec![Self::Normal, Self::Bad, Self::Good, Self::Neutral],
            ),
            (
                text::styles_data(),
                vec![
                    Self::Calculation,
                    Self::CheckCell,
                    Self::Explanatory,
                    Self::Input,
                    Self::LinkedCell,
                    Self::Note,
                    Self::Output,
                    Self::Warning,
                ],
            ),
            (
                text::styles_titles(),
                vec![
                    Self::Title,
                    Self::Heading(1),
                    Self::Heading(2),
                    Self::Heading(3),
                    Self::Heading(4),
                    Self::Total,
                ],
            ),
            (text::styles_themed(), accents),
            (
                text::styles_number(),
                vec![
                    Self::Comma,
                    Self::CommaWhole,
                    Self::Currency,
                    Self::CurrencyWhole,
                    Self::Percent,
                ],
            ),
        ]
    }

    pub fn label(self) -> String {
        match self {
            Self::Normal => text::style_normal().into(),
            Self::Bad => text::style_bad().into(),
            Self::Good => text::style_good().into(),
            Self::Neutral => text::style_neutral().into(),
            Self::Calculation => text::style_calculation().into(),
            Self::CheckCell => text::style_check_cell().into(),
            Self::Explanatory => text::style_explanatory().into(),
            Self::Input => text::style_input().into(),
            Self::LinkedCell => text::style_linked_cell().into(),
            Self::Note => text::style_note().into(),
            Self::Output => text::style_output().into(),
            Self::Warning => text::style_warning_text().into(),
            Self::Title => text::style_title().into(),
            Self::Heading(n) => format!("{} {n}", text::style_title()),
            Self::Total => text::style_total().into(),
            Self::Accent(accent, 3) => format!("{}{accent}", text::style_accent()),
            Self::Accent(accent, level) => {
                format!(
                    "{} - {}{accent}",
                    text::percent_of(20 * (level + 1)),
                    text::style_accent()
                )
            }
            Self::Comma => text::style_comma().into(),
            Self::CommaWhole => format!("{} [0]", text::style_comma()),
            Self::Currency => text::format_currency().into(),
            Self::CurrencyWhole => format!("{} [0]", text::format_currency()),
            Self::Percent => text::format_percent().into(),
        }
    }

    fn spec(self) -> Spec {
        let font = |size: u16, bold: bool, italic: bool, color: Option<u32>| {
            Some((size, bold, italic, color.map(Color)))
        };
        let plain = Spec {
            font: None,
            fill: None,
            borders: None,
            number: None,
        };
        let colored = |fill: u32, color: u32| Spec {
            font: font(110, false, false, Some(color)),
            fill: Some(Some(Color(fill))),
            ..plain
        };
        let grey = Color(0x7F7F7F);
        let heading = |size, borders| Spec {
            font: Some((size, true, false, Some(TITLE_TEXT))),
            borders: Some(borders),
            ..plain
        };
        let accent = |n: u8| Color(ACCENTS[usize::from(n.clamp(1, 6) - 1)]);
        match self {
            Self::Normal => Spec {
                font: font(110, false, false, None),
                fill: Some(None),
                borders: Some(Borders::default()),
                number: Some(GENERAL),
            },
            Self::Bad => colored(0xFFC7CE, 0x9C0006),
            Self::Good => colored(0xC6EFCE, 0x006100),
            Self::Neutral => colored(0xFFEB9C, 0x9C5700),
            Self::Calculation => Spec {
                font: font(110, true, false, Some(0xFA7D00)),
                fill: Some(Some(Color(0xF2F2F2))),
                borders: Some(all_sides(LineStyle::Thin, grey)),
                ..plain
            },
            Self::CheckCell => Spec {
                font: font(110, true, false, Some(0xFFFFFF)),
                fill: Some(Some(Color(0xA5A5A5))),
                borders: Some(all_sides(LineStyle::Double, Color(0x3F3F3F))),
                ..plain
            },
            Self::Explanatory => Spec {
                font: font(110, false, true, Some(0x7F7F7F)),
                ..plain
            },
            Self::Input => Spec {
                font: font(110, false, false, Some(0x3F3F76)),
                fill: Some(Some(Color(0xFFCC99))),
                borders: Some(all_sides(LineStyle::Thin, grey)),
                ..plain
            },
            Self::LinkedCell => Spec {
                font: font(110, false, false, Some(0xFA7D00)),
                borders: Some(bottom(LineStyle::Double, Color(0xFF8001))),
                ..plain
            },
            Self::Note => Spec {
                fill: Some(Some(Color(0xFFFFCC))),
                borders: Some(all_sides(LineStyle::Thin, Color(0xB2B2B2))),
                ..plain
            },
            Self::Output => Spec {
                font: font(110, true, false, Some(0x3F3F3F)),
                fill: Some(Some(Color(0xF2F2F2))),
                borders: Some(all_sides(LineStyle::Thin, Color(0x3F3F3F))),
                ..plain
            },
            Self::Warning => Spec {
                font: font(110, false, false, Some(0xFF0000)),
                ..plain
            },
            Self::Title => Spec {
                font: Some((180, false, false, Some(TITLE_TEXT))),
                ..plain
            },
            Self::Heading(1) => heading(150, bottom(LineStyle::Thick, accent(1))),
            Self::Heading(2) => heading(130, bottom(LineStyle::Thick, accent(1).tinted(0.5))),
            Self::Heading(3) => heading(110, bottom(LineStyle::Medium, accent(1).tinted(0.4))),
            Self::Heading(_) => Spec {
                font: Some((110, true, false, Some(TITLE_TEXT))),
                ..plain
            },
            Self::Total => Spec {
                font: font(110, true, false, None),
                borders: Some(Borders {
                    top: Some(Border {
                        style: LineStyle::Thin,
                        color: accent(1),
                    }),
                    bottom: Some(Border {
                        style: LineStyle::Double,
                        color: accent(1),
                    }),
                    ..Borders::default()
                }),
                ..plain
            },
            Self::Accent(n, level) => {
                let (fill, color) = match level {
                    0 => (accent(n).tinted(0.8), None),
                    1 => (accent(n).tinted(0.6), None),
                    2 => (accent(n).tinted(0.4), Some(0xFFFFFF)),
                    _ => (accent(n), Some(0xFFFFFF)),
                };
                Spec {
                    font: font(110, false, false, color),
                    fill: Some(Some(fill)),
                    ..plain
                }
            }
            Self::Comma => Spec {
                number: Some("_-* #,##0.00_-;-* #,##0.00_-;_-* \"-\"??_-;_-@_-"),
                ..plain
            },
            Self::CommaWhole => Spec {
                number: Some("_-* #,##0_-;-* #,##0_-;_-* \"-\"_-;_-@_-"),
                ..plain
            },
            Self::Currency => Spec {
                number: Some("_-* #,##0.00 \"₺\"_-;-* #,##0.00 \"₺\"_-;_-* \"-\"?? \"₺\"_-;_-@_-"),
                ..plain
            },
            Self::CurrencyWhole => Spec {
                number: Some("_-* #,##0 \"₺\"_-;-* #,##0 \"₺\"_-;_-* \"-\" \"₺\"_-;_-@_-"),
                ..plain
            },
            Self::Percent => Spec {
                number: Some("0%"),
                ..plain
            },
        }
    }

    pub fn apply(self, style: &mut Style) {
        let spec = self.spec();
        if let Some((size, bold, italic, color)) = spec.font {
            style.font = if self == Self::Title {
                text::title_font().to_owned()
            } else {
                vellum_grid_core::DEFAULT_FONT.to_owned()
            };
            style.size = size;
            style.bold = bold;
            style.italic = italic;
            style.underline = vellum_grid_core::Underline::None;
            style.strike = false;
            style.color = color;
        }
        if let Some(fill) = spec.fill {
            style.fill = fill;
            style.pattern = None;
        }
        if let Some(borders) = spec.borders {
            style.borders = borders;
        }
        if let Some(code) = spec.number {
            style.number_format = code.to_owned();
        }
        if self == Self::Normal {
            *style = Style::default();
        }
    }

    pub fn sample(self) -> Style {
        let mut style = Style::default();
        self.apply(&mut style);
        style
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    General,
    Number,
    Currency,
    Accounting,
    Date,
    Time,
    Percent,
    Fraction,
    Scientific,
    Text,
    Special,
    Custom,
}

impl Category {
    pub const ALL: [Self; 12] = [
        Self::General,
        Self::Number,
        Self::Currency,
        Self::Accounting,
        Self::Date,
        Self::Time,
        Self::Percent,
        Self::Fraction,
        Self::Scientific,
        Self::Text,
        Self::Special,
        Self::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::General => text::format_general(),
            Self::Number => text::format_number(),
            Self::Currency => text::format_currency(),
            Self::Accounting => text::format_accounting(),
            Self::Date => text::format_date(),
            Self::Time => text::format_time(),
            Self::Percent => text::format_percent(),
            Self::Fraction => text::format_fraction(),
            Self::Scientific => text::format_scientific(),
            Self::Text => text::format_text(),
            Self::Special => text::format_special(),
            Self::Custom => text::format_custom(),
        }
    }

    pub fn has_decimals(self) -> bool {
        matches!(
            self,
            Self::Number | Self::Currency | Self::Accounting | Self::Percent | Self::Scientific
        )
    }
}

pub const DATE_CODES: [&str; 5] = [
    numfmt::DATE,
    numfmt::LONG_DATE,
    "d.m.yyyy",
    "d mmm yyyy",
    "mmmm yyyy",
];
pub const TIME_CODES: [&str; 3] = [numfmt::TIME, numfmt::TIME_SECONDS, numfmt::DATE_TIME];
pub const FRACTION_CODES: [(&str, text::Label); 9] = [
    ("# ?/?", text::fraction_one_digit),
    ("# ??/??", text::fraction_two_digits),
    ("# ???/???", text::fraction_three_digits),
    ("# ?/2", text::fraction_halves),
    ("# ?/4", text::fraction_quarters),
    ("# ?/8", text::fraction_eighths),
    ("# ??/16", text::fraction_sixteenths),
    ("# ?/10", text::fraction_tenths),
    ("# ??/100", text::fraction_hundredths),
];
pub const SPECIAL_CODES: [(&str, text::Label); 3] = [
    ("00000", text::special_postal),
    ("0 (###) ### ## ##", text::special_phone),
    ("00000000000", text::special_id),
];
const MAX_DETECTED_DECIMALS: u8 = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct NumberFormat {
    pub category: Category,
    pub decimals: u8,
    pub separator: bool,
    pub symbol: bool,
    pub date: usize,
    pub time: usize,
    pub fraction: usize,
    pub special: usize,
    pub custom: String,
}

impl NumberFormat {
    pub fn new(code: &str) -> Self {
        let mut format = Self {
            category: Category::Custom,
            decimals: 2,
            separator: false,
            symbol: true,
            date: 0,
            time: 0,
            fraction: 0,
            special: 0,
            custom: code.to_owned(),
        };
        let mut candidates = Vec::new();
        for category in Category::ALL {
            match category {
                Category::Custom => {}
                Category::Date => candidates.extend((0..DATE_CODES.len()).map(|date| Self {
                    category,
                    date,
                    ..format.clone()
                })),
                Category::Time => candidates.extend((0..TIME_CODES.len()).map(|time| Self {
                    category,
                    time,
                    ..format.clone()
                })),
                Category::Fraction => {
                    candidates.extend((0..FRACTION_CODES.len()).map(|fraction| Self {
                        category,
                        fraction,
                        ..format.clone()
                    }))
                }
                Category::Special => {
                    candidates.extend((0..SPECIAL_CODES.len()).map(|special| Self {
                        category,
                        special,
                        ..format.clone()
                    }))
                }
                _ => {
                    for decimals in 0..=MAX_DETECTED_DECIMALS {
                        for flag in [false, true] {
                            candidates.push(Self {
                                category,
                                decimals,
                                separator: flag,
                                symbol: flag,
                                ..format.clone()
                            });
                        }
                    }
                }
            }
        }
        if let Some(found) = candidates
            .into_iter()
            .find(|candidate| candidate.code().eq_ignore_ascii_case(code))
        {
            format = found;
        }
        format
    }

    pub fn select(&mut self, category: Category) {
        if category == Category::Custom && self.category != Category::Custom {
            self.custom = self.code();
        }
        self.category = category;
    }

    pub fn code(&self) -> String {
        let fraction = if self.decimals == 0 {
            String::new()
        } else {
            format!(".{}", "0".repeat(usize::from(self.decimals)))
        };
        match self.category {
            Category::General => GENERAL.to_owned(),
            Category::Number if self.separator => format!("#,##0{fraction}"),
            Category::Number => format!("0{fraction}"),
            Category::Currency if self.symbol => format!("#,##0{fraction} \"₺\""),
            Category::Currency => format!("#,##0{fraction}"),
            Category::Accounting => {
                let unit = if self.symbol { " \"₺\"" } else { "" };
                let zero = "?".repeat(usize::from(self.decimals));
                format!(
                    "_-* #,##0{fraction}{unit}_-;-* #,##0{fraction}{unit}_-;_-* \"-\"{zero}{unit}_-;_-@_-"
                )
            }
            Category::Fraction => FRACTION_CODES[self.fraction].0.to_owned(),
            Category::Special => SPECIAL_CODES[self.special].0.to_owned(),
            Category::Date => DATE_CODES[self.date].to_owned(),
            Category::Time => TIME_CODES[self.time].to_owned(),
            Category::Percent => format!("0{fraction}%"),
            Category::Scientific => format!("0{fraction}E+00"),
            Category::Text => numfmt::TEXT.to_owned(),
            Category::Custom => self.custom.clone(),
        }
    }
}

pub fn chars_from_width(units: f64, digit: f32) -> f64 {
    let digit = f64::from(digit);
    let px = (units * digit).round();
    let padding = (digit / 4.0).round() * 2.0 + 1.0;
    if px <= padding {
        return 0.0;
    }
    ((px - padding) / digit * 100.0).round() / 100.0
}

pub fn round_points(points: f64) -> f64 {
    (points * 4.0).round() / 4.0
}

pub fn width_from_chars(chars: f64, digit: f32) -> f64 {
    if chars <= 0.0 {
        return 0.0;
    }
    let digit = f64::from(digit);
    let padding = (digit / 4.0).round() * 2.0 + 1.0;
    ((chars * digit + padding) / digit * 256.0).trunc() / 256.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_grid_core::{DEFAULT_COLUMN_WIDTH, TR};

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn range(s: &str) -> CellRange {
        s.parse().unwrap()
    }

    fn borders(kind: BorderKind, at: &str) -> Borders {
        let mut borders = Borders::default();
        kind.apply(range("B2:D4"), addr(at), &mut borders);
        borders
    }

    #[test]
    fn outside_borders_follow_the_edges() {
        let corner = borders(BorderKind::Outside, "B2");
        assert!(corner.top.is_some() && corner.left.is_some());
        assert!(corner.right.is_none() && corner.bottom.is_none());
        assert_eq!(borders(BorderKind::Outside, "C3"), Borders::default());
        let edge = borders(BorderKind::ThickOutside, "D3");
        assert_eq!(edge.right.unwrap().style, LineStyle::Thick);
        assert!(edge.top.is_none());
    }

    #[test]
    fn bottom_and_all_borders() {
        assert!(borders(BorderKind::Bottom, "C2").bottom.is_none());
        assert!(borders(BorderKind::Bottom, "C4").bottom.is_some());
        assert_eq!(
            borders(BorderKind::DoubleBottom, "B4")
                .bottom
                .unwrap()
                .style,
            LineStyle::Double
        );
        let all = borders(BorderKind::All, "C3");
        assert!(all.top.is_some() && all.right.is_some() && all.bottom.is_some());
        let mut cleared = all;
        BorderKind::None.apply(range("C3"), addr("C3"), &mut cleared);
        assert_eq!(cleared, Borders::default());
    }

    #[test]
    fn names_quick_formats() {
        assert_eq!(format_name("General"), text::format_general());
        assert_eq!(format_name(numfmt::CURRENCY), text::format_currency());
        assert_eq!(format_name("#,##0.000"), text::format_number());
        assert_eq!(format_name("0.0 \"kg\""), text::format_custom());
    }

    #[test]
    fn decimals_start_from_the_shown_value() {
        let n = |v| CellValue::Number(v);
        assert_eq!(step_decimals("General", &n(1.5), &TR, 1), "0.00");
        assert_eq!(step_decimals("General", &n(1.5), &TR, -1), "0");
        assert_eq!(step_decimals("General", &n(12.0), &TR, 1), "0.0");
        assert_eq!(step_decimals("General", &CellValue::Empty, &TR, 1), "0.0");
        assert_eq!(step_decimals("0.00", &n(1.0), &TR, -1), "0.0");
    }

    #[test]
    fn font_size_steps_through_the_list() {
        let sizes = [8, 9, 10, 11, 12, 14];
        assert_eq!(step_size(110, &sizes, true), 120);
        assert_eq!(step_size(110, &sizes, false), 100);
        assert_eq!(step_size(105, &sizes, true), 110);
        assert_eq!(step_size(140, &sizes, true), 140);
        assert_eq!(step_size(80, &sizes, false), 80);
    }

    #[test]
    fn finds_numbers_to_sum() {
        let mut sheet = Sheet::new("s");
        for (at, value) in [
            ("B1", "Başlık"),
            ("B2", "1"),
            ("B3", "2"),
            ("D5", "4"),
            ("E5", "5"),
        ] {
            sheet.set(
                addr(at),
                vellum_grid_core::Cell::parse_in(value, &TR).unwrap(),
            );
        }
        assert_eq!(sum_source(&sheet, addr("B4")), Some(range("B2:B3")));
        assert_eq!(sum_source(&sheet, addr("F5")), Some(range("D5:E5")));
        assert_eq!(sum_source(&sheet, addr("C9")), None);
    }

    #[test]
    fn rules_parse_turkish_input() {
        assert_eq!(
            RuleKind::Greater.rule("1.234,5", "", 0, &TR),
            Some(Rule::Greater(1234.5))
        );
        assert_eq!(
            RuleKind::Between.rule("10", "5", 0, &TR),
            Some(Rule::Between(10.0, 5.0))
        );
        assert_eq!(RuleKind::Less.rule("abc", "", 0, &TR), None);
        assert_eq!(
            RuleKind::Equal.rule("Elma", "", 0, &TR),
            Some(Rule::Equal(CellValue::Text("Elma".into())))
        );
        assert_eq!(RuleKind::Contains.rule("", "", 0, &TR), None);
        assert_eq!(
            RuleKind::Duplicate.rule("", "", 0, &TR),
            Some(Rule::Duplicate)
        );
        assert_eq!(
            RuleKind::BottomPercent.rule("15", "", 0, &TR),
            Some(Rule::Top {
                count: 15,
                percent: true,
                bottom: true
            })
        );
        assert_eq!(RuleKind::Top.rule("0", "", 0, &TR), None);
        assert_eq!(
            RuleKind::Date.rule("", "", 3, &TR),
            Some(Rule::Dates(DatePeriod::Last7Days))
        );
    }

    #[test]
    fn number_format_round_trips_codes() {
        for code in [
            GENERAL,
            numfmt::NUMBER,
            numfmt::THOUSANDS,
            numfmt::CURRENCY,
            numfmt::PERCENT,
            numfmt::PERCENT_DECIMAL,
            numfmt::SCIENTIFIC,
            numfmt::DATE,
            numfmt::LONG_DATE,
            numfmt::TIME_SECONDS,
            numfmt::TEXT,
            "0.000 \"kg\"",
        ] {
            assert_eq!(NumberFormat::new(code).code(), code);
        }
        assert_eq!(
            NumberFormat::new(numfmt::CURRENCY).category,
            Category::Currency
        );
        assert_eq!(NumberFormat::new("#,##0").category, Category::Number);
        assert_eq!(NumberFormat::new("0.000 \"kg\"").category, Category::Custom);
    }

    #[test]
    fn number_format_builds_codes() {
        let mut format = NumberFormat::new(GENERAL);
        format.select(Category::Number);
        format.decimals = 1;
        format.separator = true;
        assert_eq!(format.code(), "#,##0.0");
        format.select(Category::Currency);
        format.symbol = false;
        format.decimals = 0;
        assert_eq!(format.code(), "#,##0");
        format.select(Category::Custom);
        assert_eq!(format.code(), "#,##0");
        format.custom = "0%".into();
        assert_eq!(format.code(), "0%");
    }

    #[test]
    fn column_width_in_characters() {
        assert_eq!(chars_from_width(DEFAULT_COLUMN_WIDTH, 7.0), 8.43);
        assert_eq!(width_from_chars(8.43, 7.0), DEFAULT_COLUMN_WIDTH);
        assert_eq!(chars_from_width(0.0, 7.0), 0.0);
        assert_eq!(width_from_chars(0.0, 7.0), 0.0);
        assert_eq!(chars_from_width(width_from_chars(20.0, 7.0), 7.0), 20.0);
    }
}
