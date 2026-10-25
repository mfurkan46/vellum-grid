use eframe::egui::{
    self, Color32, ComboBox, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, RichText, Sense,
    Shape, Stroke, StrokeKind, TextEdit, Ui, pos2, vec2,
};
use vellum_grid_core::{
    Bar, Border, Cell, CellRange, CellValue, Color, ColorScale, ConditionalFormat, DataBar,
    DatePeriod, EN, Highlight, IconKind, IconSet, LineStyle, Locale, Rule, Threshold, formula,
};

use crate::app::{Spreadsheet, locale};
use crate::dialogs::{Dialog, list_box, plain_button};
use crate::format::RuleKind;
use crate::format_cells::color_button;
use crate::icons::Icon;
use crate::{menu, render, text, theme};

const BAR_COLORS: [u32; 6] = [0x638EC6, 0x63C384, 0xFF555A, 0xFFB628, 0x008AEF, 0xD6007B];
const RED: u32 = 0xF8696B;
const YELLOW: u32 = 0xFFEB84;
const GREEN: u32 = 0x63BE7B;
const WHITE: u32 = 0xFCFCFF;
const BLUE: u32 = 0x5A8AC6;
const PALE_YELLOW: u32 = 0xFFEF9C;
const SCALES: [&[u32]; 12] = [
    &[GREEN, YELLOW, RED],
    &[RED, YELLOW, GREEN],
    &[GREEN, WHITE, RED],
    &[RED, WHITE, GREEN],
    &[BLUE, WHITE, RED],
    &[RED, WHITE, BLUE],
    &[WHITE, RED],
    &[RED, WHITE],
    &[GREEN, WHITE],
    &[WHITE, GREEN],
    &[GREEN, PALE_YELLOW],
    &[PALE_YELLOW, GREEN],
];

pub fn period_label(period: DatePeriod) -> &'static str {
    match period {
        DatePeriod::Yesterday => text::period_yesterday(),
        DatePeriod::Today => text::period_today(),
        DatePeriod::Tomorrow => text::period_tomorrow(),
        DatePeriod::Last7Days => text::period_last_7_days(),
        DatePeriod::LastWeek => text::period_last_week(),
        DatePeriod::ThisWeek => text::period_this_week(),
        DatePeriod::NextWeek => text::period_next_week(),
        DatePeriod::LastMonth => text::period_last_month(),
        DatePeriod::ThisMonth => text::period_this_month(),
        DatePeriod::NextMonth => text::period_next_month(),
    }
}

fn scale_rule(colors: &[u32]) -> ColorScale {
    let mut points: Vec<(Threshold, Color)> = Vec::new();
    let top = colors.len() - 1;
    for (index, color) in colors.iter().rev().enumerate() {
        let threshold = match index {
            0 => Threshold::Lowest,
            i if i == top => Threshold::Highest,
            _ => Threshold::Percentile(50.0),
        };
        points.push((threshold, Color(*color)));
    }
    ColorScale { points }
}

fn c32(color: Color) -> Color32 {
    render::color(color)
}

fn shade(color: Color32, amount: f32) -> Color32 {
    color.lerp_to_gamma(theme::cell_bg(), amount)
}

pub fn paint_bar(painter: &Painter, cell: Rect, bar: &Bar) {
    let inner = cell.shrink2(vec2(2.0, 2.0));
    let x = |f: f32| inner.left() + inner.width() * f;
    let rect = Rect::from_min_max(
        pos2(x(bar.start), inner.top()),
        pos2(x(bar.end), inner.bottom()),
    );
    let color = c32(bar.color);
    if bar.gradient {
        let mut mesh = Mesh::default();
        let (left, right) = if bar.end <= bar.axis.unwrap_or(0.0) {
            (shade(color, 0.9), color)
        } else {
            (color, shade(color, 0.9))
        };
        let base = mesh.vertices.len() as u32;
        for (pos, tint) in [
            (rect.left_top(), left),
            (rect.right_top(), right),
            (rect.right_bottom(), right),
            (rect.left_bottom(), left),
        ] {
            mesh.colored_vertex(pos, tint);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
        painter.add(Shape::mesh(mesh));
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, color),
            StrokeKind::Inside,
        );
    } else {
        painter.rect_filled(rect, CornerRadius::ZERO, color);
    }
    if let Some(axis) = bar.axis {
        let line = Stroke::new(1.0, Color32::BLACK);
        painter.extend(Shape::dashed_line(
            &[pos2(x(axis), inner.top()), pos2(x(axis), inner.bottom())],
            line,
            2.0,
            2.0,
        ));
    }
}

const ICON_GREEN: Color32 = Color32::from_rgb(0x00, 0xB0, 0x50);
const ICON_YELLOW: Color32 = Color32::from_rgb(0xFF, 0xC0, 0x00);
const ICON_RED: Color32 = Color32::from_rgb(0xE0, 0x30, 0x30);
const ICON_ORANGE: Color32 = Color32::from_rgb(0xFF, 0x8C, 0x00);
const ICON_GRAY: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
const ICON_DARK: Color32 = Color32::from_rgb(0x40, 0x40, 0x40);
const ICON_BLUE: Color32 = Color32::from_rgb(0x2F, 0x75, 0xB5);

fn arrow(painter: &Painter, rect: Rect, angle: f32, color: Color32) {
    let c = rect.center();
    let r = rect.width() / 2.0;
    let rot = egui::emath::Rot2::from_angle(angle);
    let pt = |x: f32, y: f32| c + rot * vec2(x * r, y * r);
    let head = vec![pt(0.0, -1.0), pt(0.85, -0.05), pt(-0.85, -0.05)];
    painter.add(Shape::convex_polygon(head, color, Stroke::NONE));
    painter.line_segment([pt(0.0, -0.1), pt(0.0, 0.95)], Stroke::new(r * 0.55, color));
}

fn triangle(painter: &Painter, rect: Rect, up: bool, color: Color32) {
    let r = rect.shrink(rect.width() * 0.12);
    let points = if up {
        vec![r.center_top(), r.right_bottom(), r.left_bottom()]
    } else {
        vec![r.left_top(), r.right_top(), r.center_bottom()]
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

fn mark(painter: &Painter, rect: Rect, index: usize, circled: bool) {
    let color = [ICON_RED, ICON_YELLOW, ICON_GREEN][index];
    let c = rect.center();
    let r = rect.width() / 2.0;
    let ink = if circled {
        painter.circle_filled(c, r, color);
        Color32::WHITE
    } else {
        color
    };
    let s = Stroke::new(r * 0.3, ink);
    match index {
        2 => {
            painter.line_segment([c + vec2(-0.5 * r, 0.0), c + vec2(-0.1 * r, 0.45 * r)], s);
            painter.line_segment(
                [c + vec2(-0.1 * r, 0.45 * r), c + vec2(0.55 * r, -0.45 * r)],
                s,
            );
        }
        1 => {
            painter.line_segment([c + vec2(0.0, -0.55 * r), c + vec2(0.0, 0.15 * r)], s);
            painter.circle_filled(c + vec2(0.0, 0.5 * r), r * 0.15, ink);
        }
        _ => {
            let d = 0.45 * r;
            painter.line_segment([c + vec2(-d, -d), c + vec2(d, d)], s);
            painter.line_segment([c + vec2(d, -d), c + vec2(-d, d)], s);
        }
    }
}

fn star(painter: &Painter, rect: Rect, fill: f32) {
    let c = rect.center();
    let r = rect.width() / 2.0;
    let points: Vec<Pos2> = (0..10)
        .map(|i| {
            let radius = if i % 2 == 0 { r } else { r * 0.45 };
            let angle = std::f32::consts::PI * (i as f32) / 5.0 - std::f32::consts::FRAC_PI_2;
            c + vec2(angle.cos(), angle.sin()) * radius
        })
        .collect();
    let gold = Color32::from_rgb(0xFF, 0xC0, 0x00);
    for i in 0..10 {
        let tri = vec![c, points[i], points[(i + 1) % 10]];
        let on = fill >= 1.0 || (fill > 0.0 && points[i].x <= c.x + 0.1);
        let color = if on { gold } else { Color32::from_gray(0xE0) };
        painter.add(Shape::convex_polygon(tri, color, Stroke::NONE));
    }
}

pub fn paint_icon(painter: &Painter, rect: Rect, kind: IconKind, index: usize) {
    let n = kind.count();
    let level = index as f32 / (n - 1) as f32;
    match kind {
        IconKind::Arrows3
        | IconKind::ArrowsGray3
        | IconKind::Arrows4
        | IconKind::ArrowsGray4
        | IconKind::Arrows5
        | IconKind::ArrowsGray5 => {
            let angle = std::f32::consts::PI * (1.0 - level);
            let colors: &[Color32] = match n {
                3 => &[ICON_RED, ICON_YELLOW, ICON_GREEN],
                4 => &[ICON_RED, ICON_ORANGE, ICON_YELLOW, ICON_GREEN],
                _ => &[ICON_RED, ICON_ORANGE, ICON_YELLOW, ICON_YELLOW, ICON_GREEN],
            };
            let gray = matches!(
                kind,
                IconKind::ArrowsGray3 | IconKind::ArrowsGray4 | IconKind::ArrowsGray5
            );
            let color = if gray { ICON_GRAY } else { colors[index] };
            arrow(painter, rect, angle, color);
        }
        IconKind::Triangles3 => match index {
            2 => triangle(painter, rect, true, ICON_GREEN),
            0 => triangle(painter, rect, false, ICON_RED),
            _ => {
                let bar = Rect::from_center_size(
                    rect.center(),
                    vec2(rect.width() * 0.7, rect.height() * 0.25),
                );
                painter.rect_filled(bar, CornerRadius::ZERO, ICON_YELLOW);
            }
        },
        IconKind::TrafficLights3 | IconKind::TrafficLightsRim3 | IconKind::TrafficLights4 => {
            let colors: &[Color32] = if n == 4 {
                &[ICON_DARK, ICON_RED, ICON_YELLOW, ICON_GREEN]
            } else {
                &[ICON_RED, ICON_YELLOW, ICON_GREEN]
            };
            let r = rect.width() / 2.0 * 0.9;
            if kind == IconKind::TrafficLightsRim3 {
                painter.rect_filled(rect, CornerRadius::same(3), ICON_DARK);
            }
            painter.circle_filled(rect.center(), r * 0.85, colors[index]);
        }
        IconKind::RedToBlack4 => {
            let colors = [
                ICON_DARK,
                ICON_GRAY,
                Color32::from_rgb(0xFF, 0x99, 0x99),
                ICON_RED,
            ];
            painter.circle_filled(rect.center(), rect.width() * 0.42, colors[index]);
        }
        IconKind::Signs3 => match index {
            2 => {
                painter.circle_filled(rect.center(), rect.width() * 0.45, ICON_GREEN);
            }
            1 => triangle(painter, rect, true, ICON_YELLOW),
            _ => {
                let c = rect.center();
                let r = rect.width() * 0.48;
                let points = vec![
                    c + vec2(0.0, -r),
                    c + vec2(r, 0.0),
                    c + vec2(0.0, r),
                    c + vec2(-r, 0.0),
                ];
                painter.add(Shape::convex_polygon(points, ICON_RED, Stroke::NONE));
            }
        },
        IconKind::SymbolsCircled3 => mark(painter, rect, index, true),
        IconKind::Symbols3 => mark(painter, rect, index, false),
        IconKind::Flags3 => {
            let color = [ICON_RED, ICON_YELLOW, ICON_GREEN][index];
            let left = rect.left() + rect.width() * 0.25;
            painter.line_segment(
                [pos2(left, rect.top()), pos2(left, rect.bottom())],
                Stroke::new(1.5, ICON_DARK),
            );
            let flag = vec![
                pos2(left, rect.top()),
                pos2(rect.right(), rect.top() + rect.height() * 0.25),
                pos2(left, rect.top() + rect.height() * 0.5),
            ];
            painter.add(Shape::convex_polygon(flag, color, Stroke::NONE));
        }
        IconKind::Stars3 => star(painter, rect, level),
        IconKind::Rating4 | IconKind::Rating5 => {
            let bars = n - 1;
            let w = rect.width() / bars as f32;
            for i in 0..bars {
                let h = rect.height() * (i + 1) as f32 / bars as f32;
                let r = Rect::from_min_max(
                    pos2(rect.left() + i as f32 * w + 1.0, rect.bottom() - h),
                    pos2(rect.left() + (i + 1) as f32 * w - 1.0, rect.bottom()),
                );
                let color = if i < index {
                    ICON_BLUE
                } else {
                    Color32::from_gray(0xD0)
                };
                painter.rect_filled(r, CornerRadius::ZERO, color);
            }
        }
        IconKind::Quarters5 => {
            let c = rect.center();
            let r = rect.width() * 0.45;
            painter.circle_filled(c, r, Color32::WHITE);
            let turns = level * std::f32::consts::TAU;
            if index == n - 1 {
                painter.circle_filled(c, r, ICON_DARK);
            } else if index > 0 {
                let steps = 24;
                let mut points = vec![c];
                for s in 0..=steps {
                    let a = -std::f32::consts::FRAC_PI_2 + turns * s as f32 / steps as f32;
                    points.push(c + vec2(a.cos(), a.sin()) * r);
                }
                for pair in points[1..].windows(2) {
                    painter.add(Shape::convex_polygon(
                        vec![c, pair[0], pair[1]],
                        ICON_DARK,
                        Stroke::NONE,
                    ));
                }
            }
            painter.circle_stroke(c, r, Stroke::new(1.0, ICON_DARK));
        }
        IconKind::Boxes5 => {
            let half = rect.width() / 2.0;
            for i in 0..4 {
                let min = rect.min + vec2((i % 2) as f32 * half, (i / 2) as f32 * half);
                let r = Rect::from_min_size(min, vec2(half, half)).shrink(1.0);
                let color = if i < index {
                    ICON_BLUE
                } else {
                    Color32::from_gray(0xD0)
                };
                painter.rect_filled(r, CornerRadius::ZERO, color);
            }
        }
    }
}

pub fn icon_rect(cell: Rect) -> Rect {
    let size = (cell.height() - 4.0).clamp(6.0, theme::CF_ICON_SIZE);
    Rect::from_min_size(
        pos2(cell.left() + 2.0, cell.center().y - size / 2.0),
        vec2(size, size),
    )
}

fn icon_row(ui: &mut Ui, kind: IconKind) -> egui::Response {
    let n = kind.count();
    let size = vec2(theme::CF_ICON_SIZE * 5.0 + 12.0, theme::CF_ICON_SIZE + 6.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(2), theme::tool_hover());
    }
    for i in 0..n {
        let min = rect.min + vec2(4.0 + i as f32 * (theme::CF_ICON_SIZE + 2.0), 3.0);
        let r = Rect::from_min_size(min, vec2(theme::CF_ICON_SIZE, theme::CF_ICON_SIZE));
        paint_icon(ui.painter(), r, kind, n - 1 - i);
    }
    response
}

fn sample_box(ui: &mut Ui, paint: impl FnOnce(&Painter, Rect)) -> egui::Response {
    let size = vec2(theme::CF_SAMPLE, theme::CF_SAMPLE);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, theme::cell_bg());
    paint(&painter, rect.shrink(3.0));
    let line = if response.hovered() {
        Stroke::new(2.0, theme::accent())
    } else {
        Stroke::new(1.0, theme::ribbon_line())
    };
    painter.rect_stroke(rect, CornerRadius::ZERO, line, StrokeKind::Inside);
    response
}

fn gallery_bar(ui: &mut Ui, color: u32, gradient: bool) -> egui::Response {
    sample_box(ui, |painter, rect| {
        for (i, f) in [0.9_f32, 0.6, 0.35].iter().enumerate() {
            let h = rect.height() / 3.0;
            let row =
                Rect::from_min_size(rect.min + vec2(0.0, i as f32 * h), vec2(rect.width(), h));
            paint_bar(
                painter,
                row,
                &Bar {
                    start: 0.0,
                    end: *f,
                    axis: None,
                    color: Color(color),
                    gradient,
                },
            );
        }
    })
}

fn gallery_scale(ui: &mut Ui, colors: &[u32]) -> egui::Response {
    sample_box(ui, |painter, rect| {
        let rows = 3;
        let h = rect.height() / rows as f32;
        for i in 0..rows {
            let t = i as f32 / (rows - 1) as f32;
            let index = ((colors.len() - 1) as f32 * t).round() as usize;
            let row =
                Rect::from_min_size(rect.min + vec2(0.0, i as f32 * h), vec2(rect.width(), h));
            painter.rect_filled(row, CornerRadius::ZERO, c32(Color(colors[index])));
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleType {
    Graded,
    Cell,
    TopBottom,
    Average,
    Unique,
    Formula,
}

impl RuleType {
    const ALL: [Self; 6] = [
        Self::Graded,
        Self::Cell,
        Self::TopBottom,
        Self::Average,
        Self::Unique,
        Self::Formula,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Graded => text::rule_type_graded(),
            Self::Cell => text::rule_type_cell(),
            Self::TopBottom => text::rule_type_top(),
            Self::Average => text::rule_type_average(),
            Self::Unique => text::rule_type_unique(),
            Self::Formula => text::rule_type_formula(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Graded {
    TwoColor,
    ThreeColor,
    Bar,
    Icons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Value,
    Text,
    Dates,
    Blanks,
    NoBlanks,
    Errors,
    NoErrors,
}

impl CellKind {
    const ALL: [Self; 7] = [
        Self::Value,
        Self::Text,
        Self::Dates,
        Self::Blanks,
        Self::NoBlanks,
        Self::Errors,
        Self::NoErrors,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Value => text::cell_kind_value(),
            Self::Text => text::cell_kind_text(),
            Self::Dates => text::cell_kind_dates(),
            Self::Blanks => text::cell_kind_blanks(),
            Self::NoBlanks => text::cell_kind_no_blanks(),
            Self::Errors => text::cell_kind_errors(),
            Self::NoErrors => text::cell_kind_no_errors(),
        }
    }
}

const OPERATORS: [text::Label; 8] = [
    text::op_between,
    text::op_not_between,
    text::op_equal,
    text::op_not_equal,
    text::op_greater,
    text::op_less,
    text::op_greater_equal,
    text::op_less_equal,
];
const TEXT_OPERATORS: [text::Label; 4] = [
    text::op_contains,
    text::op_not_contains,
    text::op_begins,
    text::op_ends,
];
const AVERAGES: [(text::Label, bool, bool, u8); 10] = [
    (text::avg_above, false, false, 0),
    (text::avg_below, true, false, 0),
    (text::avg_equal_above, false, true, 0),
    (text::avg_equal_below, true, true, 0),
    (text::avg_std1_above, false, false, 1),
    (text::avg_std1_below, true, false, 1),
    (text::avg_std2_above, false, false, 2),
    (text::avg_std2_below, true, false, 2),
    (text::avg_std3_above, false, false, 3),
    (text::avg_std3_below, true, false, 3),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointKind {
    Automatic,
    Lowest,
    Highest,
    Number,
    Percent,
    Percentile,
}

impl PointKind {
    fn label(self) -> &'static str {
        match self {
            Self::Automatic => text::point_automatic(),
            Self::Lowest => text::point_lowest(),
            Self::Highest => text::point_highest(),
            Self::Number => text::point_number(),
            Self::Percent => text::point_percent(),
            Self::Percentile => text::point_percentile(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Point {
    kind: PointKind,
    value: String,
    color: Color,
}

impl Point {
    fn new(kind: PointKind, value: f64, color: u32) -> Self {
        Self {
            kind,
            value: locale().format_number(value),
            color: Color(color),
        }
    }

    fn from_threshold(threshold: Threshold, color: Color) -> Self {
        let (kind, value) = match threshold {
            Threshold::Automatic => (PointKind::Automatic, 0.0),
            Threshold::Lowest => (PointKind::Lowest, 0.0),
            Threshold::Highest => (PointKind::Highest, 0.0),
            Threshold::Number(n) => (PointKind::Number, n),
            Threshold::Percent(n) => (PointKind::Percent, n),
            Threshold::Percentile(n) => (PointKind::Percentile, n),
        };
        Self {
            kind,
            value: locale().format_number(value),
            color,
        }
    }

    fn threshold(&self, locale: &Locale) -> Option<Threshold> {
        let value = || locale.parse_number(self.value.trim());
        Some(match self.kind {
            PointKind::Automatic => Threshold::Automatic,
            PointKind::Lowest => Threshold::Lowest,
            PointKind::Highest => Threshold::Highest,
            PointKind::Number => Threshold::Number(value()?),
            PointKind::Percent => Threshold::Percent(value()?),
            PointKind::Percentile => Threshold::Percentile(value()?),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RuleForm {
    range: CellRange,
    editing: Option<usize>,
    back: Option<Box<ManageState>>,
    kind: RuleType,
    graded: Graded,
    points: [Point; 3],
    bar_color: Color,
    gradient: bool,
    show_value: bool,
    icon: IconKind,
    reverse: bool,
    icon_points: Vec<Point>,
    cell: CellKind,
    operator: usize,
    first: String,
    second: String,
    text_operator: usize,
    text: String,
    period: usize,
    bottom: bool,
    count: String,
    percent: bool,
    average: usize,
    duplicate: bool,
    formula: String,
    format: Highlight,
    stop: bool,
    error: bool,
}

impl RuleForm {
    pub fn heading(&self) -> &'static str {
        if self.editing.is_some() {
            text::edit_rule_title()
        } else {
            text::new_rule_title()
        }
    }

    fn new(range: CellRange) -> Self {
        Self {
            range,
            editing: None,
            back: None,
            kind: RuleType::Graded,
            graded: Graded::TwoColor,
            points: [
                Point::new(PointKind::Lowest, 0.0, RED),
                Point::new(PointKind::Percentile, 50.0, YELLOW),
                Point::new(PointKind::Highest, 0.0, GREEN),
            ],
            bar_color: Color(BAR_COLORS[0]),
            gradient: true,
            show_value: true,
            icon: IconKind::Arrows3,
            reverse: false,
            icon_points: icon_points(IconKind::Arrows3),
            cell: CellKind::Value,
            operator: 0,
            first: String::new(),
            second: String::new(),
            text_operator: 0,
            text: String::new(),
            period: 1,
            bottom: false,
            count: "10".into(),
            percent: false,
            average: 0,
            duplicate: true,
            formula: String::new(),
            format: Highlight::default(),
            stop: false,
            error: false,
        }
    }

    fn from_format(format: &ConditionalFormat, index: usize) -> Self {
        let mut form = Self::new(format.range);
        form.editing = Some(index);
        form.format = format.format;
        form.stop = format.stop;
        let value = |v: &CellValue| match v {
            CellValue::Empty => String::new(),
            other => locale().display(other),
        };
        let number = |n: f64| locale().format_number(n);
        let mut cell = |kind: CellKind, operator: usize, first: String, second: String| {
            form.kind = RuleType::Cell;
            form.cell = kind;
            form.operator = operator;
            form.first = first;
            form.second = second;
        };
        match &format.rule {
            Rule::Between(a, b) => cell(CellKind::Value, 0, number(*a), number(*b)),
            Rule::NotBetween(a, b) => cell(CellKind::Value, 1, number(*a), number(*b)),
            Rule::Equal(v) => cell(CellKind::Value, 2, value(v), String::new()),
            Rule::NotEqual(v) => cell(CellKind::Value, 3, value(v), String::new()),
            Rule::Greater(n) => cell(CellKind::Value, 4, number(*n), String::new()),
            Rule::Less(n) => cell(CellKind::Value, 5, number(*n), String::new()),
            Rule::GreaterEqual(n) => cell(CellKind::Value, 6, number(*n), String::new()),
            Rule::LessEqual(n) => cell(CellKind::Value, 7, number(*n), String::new()),
            Rule::Blanks => cell(CellKind::Blanks, 0, String::new(), String::new()),
            Rule::NoBlanks => cell(CellKind::NoBlanks, 0, String::new(), String::new()),
            Rule::Errors => cell(CellKind::Errors, 0, String::new(), String::new()),
            Rule::NoErrors => cell(CellKind::NoErrors, 0, String::new(), String::new()),
            Rule::Dates(period) => {
                cell(CellKind::Dates, 0, String::new(), String::new());
                form.period = DatePeriod::ALL
                    .iter()
                    .position(|p| p == period)
                    .unwrap_or(0);
            }
            Rule::TextContains(t)
            | Rule::TextNotContains(t)
            | Rule::BeginsWith(t)
            | Rule::EndsWith(t) => {
                cell(CellKind::Text, 0, String::new(), String::new());
                form.text = t.clone();
                form.text_operator = match &format.rule {
                    Rule::TextNotContains(_) => 1,
                    Rule::BeginsWith(_) => 2,
                    Rule::EndsWith(_) => 3,
                    _ => 0,
                };
            }
            Rule::Duplicate | Rule::Unique => {
                form.kind = RuleType::Unique;
                form.duplicate = format.rule == Rule::Duplicate;
            }
            Rule::Top {
                count,
                percent,
                bottom,
            } => {
                form.kind = RuleType::TopBottom;
                form.count = count.to_string();
                form.percent = *percent;
                form.bottom = *bottom;
            }
            Rule::Average {
                below,
                equal,
                deviations,
            } => {
                form.kind = RuleType::Average;
                form.average = AVERAGES
                    .iter()
                    .position(|(_, b, e, d)| b == below && e == equal && d == deviations)
                    .unwrap_or(0);
            }
            Rule::Expression(text) => {
                form.kind = RuleType::Formula;
                form.formula = formula::parse(text, &EN)
                    .map(|expr| format!("={}", expr.write(locale())))
                    .unwrap_or_else(|_| format!("={text}"));
            }
            Rule::ColorScale(scale) => {
                form.kind = RuleType::Graded;
                let points: Vec<Point> = scale
                    .points
                    .iter()
                    .map(|(t, c)| Point::from_threshold(*t, *c))
                    .collect();
                if points.len() == 3 {
                    form.graded = Graded::ThreeColor;
                    form.points = [points[0].clone(), points[1].clone(), points[2].clone()];
                } else if let [low, high] = points.as_slice() {
                    form.graded = Graded::TwoColor;
                    form.points[0] = low.clone();
                    form.points[2] = high.clone();
                }
            }
            Rule::DataBar(bar) => {
                form.kind = RuleType::Graded;
                form.graded = Graded::Bar;
                form.bar_color = bar.color;
                form.gradient = bar.gradient;
                form.show_value = bar.show_value;
                form.points[0] = Point::from_threshold(bar.min, bar.color);
                form.points[2] = Point::from_threshold(bar.max, bar.color);
            }
            Rule::IconSet(set) => {
                form.kind = RuleType::Graded;
                form.graded = Graded::Icons;
                form.icon = set.kind;
                form.reverse = set.reverse;
                form.show_value = set.show_value;
                form.icon_points = set
                    .thresholds
                    .iter()
                    .map(|t| Point::from_threshold(*t, Color::BLACK))
                    .collect();
            }
        }
        form
    }

    fn rule(&self, locale: &Locale) -> Option<Rule> {
        let number = |text: &str| locale.parse_number(text.trim());
        let point = |i: usize| self.points[i].threshold(locale);
        Some(match self.kind {
            RuleType::Graded => match self.graded {
                Graded::TwoColor => Rule::ColorScale(ColorScale {
                    points: vec![
                        (point(0)?, self.points[0].color),
                        (point(2)?, self.points[2].color),
                    ],
                }),
                Graded::ThreeColor => Rule::ColorScale(ColorScale {
                    points: (0..3)
                        .map(|i| Some((point(i)?, self.points[i].color)))
                        .collect::<Option<Vec<_>>>()?,
                }),
                Graded::Bar => Rule::DataBar(DataBar {
                    color: self.bar_color,
                    gradient: self.gradient,
                    min: point(0)?,
                    max: point(2)?,
                    show_value: self.show_value,
                }),
                Graded::Icons => Rule::IconSet(IconSet {
                    kind: self.icon,
                    reverse: self.reverse,
                    show_value: self.show_value,
                    thresholds: self
                        .icon_points
                        .iter()
                        .map(|p| p.threshold(locale))
                        .collect::<Option<Vec<_>>>()?,
                }),
            },
            RuleType::Cell => match self.cell {
                CellKind::Value => {
                    let first = number(&self.first);
                    match self.operator {
                        0 => Rule::Between(first?, number(&self.second)?),
                        1 => Rule::NotBetween(first?, number(&self.second)?),
                        2 | 3 => {
                            let value = Cell::parse_in(&self.first, locale)
                                .ok()
                                .filter(|c| c.formula.is_none() && !c.value.is_empty())?
                                .value;
                            if self.operator == 2 {
                                Rule::Equal(value)
                            } else {
                                Rule::NotEqual(value)
                            }
                        }
                        4 => Rule::Greater(first?),
                        5 => Rule::Less(first?),
                        6 => Rule::GreaterEqual(first?),
                        _ => Rule::LessEqual(first?),
                    }
                }
                CellKind::Text if self.text.is_empty() => return None,
                CellKind::Text => match self.text_operator {
                    0 => Rule::TextContains(self.text.clone()),
                    1 => Rule::TextNotContains(self.text.clone()),
                    2 => Rule::BeginsWith(self.text.clone()),
                    _ => Rule::EndsWith(self.text.clone()),
                },
                CellKind::Dates => Rule::Dates(*DatePeriod::ALL.get(self.period)?),
                CellKind::Blanks => Rule::Blanks,
                CellKind::NoBlanks => Rule::NoBlanks,
                CellKind::Errors => Rule::Errors,
                CellKind::NoErrors => Rule::NoErrors,
            },
            RuleType::TopBottom => Rule::Top {
                count: number(&self.count)
                    .filter(|n| n.fract() == 0.0 && (1.0..=1000.0).contains(n))?
                    as u32,
                percent: self.percent,
                bottom: self.bottom,
            },
            RuleType::Average => {
                let (_, below, equal, deviations) = AVERAGES[self.average];
                Rule::Average {
                    below,
                    equal,
                    deviations,
                }
            }
            RuleType::Unique if self.duplicate => Rule::Duplicate,
            RuleType::Unique => Rule::Unique,
            RuleType::Formula => {
                let body = self.formula.trim().trim_start_matches('=');
                let expr = formula::parse(body, locale).ok()?;
                Rule::Expression(expr.write(&EN))
            }
        })
    }
}

fn icon_points(kind: IconKind) -> Vec<Point> {
    kind.default_thresholds()
        .into_iter()
        .map(|t| Point::from_threshold(t, Color::BLACK))
        .collect()
}

pub fn describe(rule: &Rule) -> String {
    let n = |v: f64| locale().format_number(v);
    let value = |v: &CellValue| locale().display(v);
    let cell = text::cell_kind_value();
    match rule {
        Rule::Greater(v) => format!("{cell} > {}", n(*v)),
        Rule::Less(v) => format!("{cell} < {}", n(*v)),
        Rule::GreaterEqual(v) => format!("{cell} >= {}", n(*v)),
        Rule::LessEqual(v) => format!("{cell} <= {}", n(*v)),
        Rule::Between(a, b) => format!(
            "{cell} {} {} {} {}",
            text::op_between(),
            n(*a),
            text::and(),
            n(*b)
        ),
        Rule::NotBetween(a, b) => {
            format!(
                "{cell} {} {} {} {}",
                text::op_not_between(),
                n(*a),
                text::and(),
                n(*b)
            )
        }
        Rule::Equal(v) => format!("{cell} = {}", value(v)),
        Rule::NotEqual(v) => format!("{cell} <> {}", value(v)),
        Rule::TextContains(t) => format!("{} '{t}'", text::op_contains()),
        Rule::TextNotContains(t) => format!("{} '{t}'", text::op_not_contains()),
        Rule::BeginsWith(t) => format!("'{t}' {}", text::op_begins()),
        Rule::EndsWith(t) => format!("'{t}' {}", text::op_ends()),
        Rule::Duplicate => text::rule_duplicate().into(),
        Rule::Unique => text::rule_unique().into(),
        Rule::Blanks => text::cell_kind_blanks().into(),
        Rule::NoBlanks => text::cell_kind_no_blanks().into(),
        Rule::Errors => text::cell_kind_errors().into(),
        Rule::NoErrors => text::cell_kind_no_errors().into(),
        Rule::Dates(p) => format!("{}: {}", text::cell_kind_dates(), period_label(*p)),
        Rule::Top {
            count,
            percent,
            bottom,
        } => {
            let side = if *bottom { text::bottom() } else { text::top() };
            let unit = if *percent { "%" } else { "" };
            format!("{side} {unit}{count}")
        }
        Rule::Average {
            below,
            equal,
            deviations,
        } => AVERAGES
            .iter()
            .find(|(_, b, e, d)| b == below && e == equal && d == deviations)
            .map_or(text::avg_above(), |(label, ..)| label())
            .to_owned(),
        Rule::Expression(f) => {
            let shown = formula::parse(f, &EN).map_or_else(|_| f.clone(), |e| e.write(locale()));
            format!("{}: ={shown}", text::formula())
        }
        Rule::DataBar(_) => text::data_bar().into(),
        Rule::ColorScale(scale) if scale.points.len() == 3 => text::scale_three().into(),
        Rule::ColorScale(_) => text::scale_two().into(),
        Rule::IconSet(_) => text::icon_set().into(),
    }
}

fn absolute(range: CellRange) -> String {
    let corner = |addr: vellum_grid_core::CellAddr| {
        format!(
            "${}${}",
            vellum_grid_core::column_name(addr.col()),
            addr.row() + 1
        )
    };
    if range.is_single() {
        format!("={}", corner(range.start()))
    } else {
        format!("={}:{}", corner(range.start()), corner(range.end()))
    }
}

fn parse_range(text: &str) -> Option<CellRange> {
    let cleaned: String = text
        .trim()
        .trim_start_matches('=')
        .chars()
        .filter(|c| *c != '$')
        .collect();
    cleaned.parse().ok()
}

#[derive(Debug, Clone)]
pub struct ManageState {
    rules: Vec<ConditionalFormat>,
    ranges: Vec<String>,
    selection: CellRange,
    sheet: bool,
    selected: Option<usize>,
    error: bool,
}

pub enum ManageAction {
    New,
    Edit(usize),
}

fn format_sample(ui: &mut Ui, format: &ConditionalFormat) {
    let size = vec2(
        theme::CF_FORMAT_SAMPLE_WIDTH,
        theme::CF_FORMAT_SAMPLE_HEIGHT,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, theme::cell_bg());
    match &format.rule {
        Rule::DataBar(bar) => paint_bar(
            &painter,
            rect,
            &Bar {
                start: 0.0,
                end: 0.7,
                axis: None,
                color: bar.color,
                gradient: bar.gradient,
            },
        ),
        Rule::ColorScale(scale) => {
            let n = scale.points.len();
            let w = rect.width() / n as f32;
            for (i, (_, color)) in scale.points.iter().enumerate() {
                let r =
                    Rect::from_min_size(rect.min + vec2(i as f32 * w, 0.0), vec2(w, rect.height()));
                painter.rect_filled(r, CornerRadius::ZERO, c32(*color));
            }
        }
        Rule::IconSet(set) => {
            let n = set.kind.count();
            for i in 0..n {
                let r = Rect::from_min_size(
                    rect.min + vec2(2.0 + i as f32 * (theme::CF_ICON_SIZE + 2.0), 2.0),
                    vec2(theme::CF_ICON_SIZE, theme::CF_ICON_SIZE),
                );
                paint_icon(&painter, r, set.kind, n - 1 - i);
            }
        }
        _ => highlight_sample(&painter, rect, &format.format),
    }
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
}

fn highlight_sample(painter: &Painter, rect: Rect, format: &Highlight) {
    if let Some(fill) = format.fill {
        painter.rect_filled(rect, CornerRadius::ZERO, c32(fill));
    }
    if let Some(border) = format.border {
        painter.rect_stroke(
            rect.shrink(2.0),
            CornerRadius::ZERO,
            Stroke::new(1.0, c32(border.color)),
            StrokeKind::Inside,
        );
    }
    let mut font = FontId::proportional(theme::DIALOG_FONT_SIZE);
    if format.bold {
        font.family = egui::FontFamily::Name(crate::fonts::CHROME_BOLD.into());
    }
    let color = format.color.map_or(theme::cell_text(), c32);
    let galley = painter.layout_no_wrap(text::format_preview_text().to_owned(), font, color);
    let pos = rect.center() - galley.size() / 2.0;
    let width = galley.size().x;
    painter.galley(pos, galley, color);
    if format.strike {
        let y = rect.center().y;
        painter.hline(pos.x..=pos.x + width, y, Stroke::new(1.0, color));
    }
}

pub fn manage_ui(ui: &mut Ui, state: &mut ManageState) -> Option<ManageAction> {
    let mut action = None;
    ui.set_width(theme::CF_MANAGE_WIDTH);
    ui.horizontal(|ui| {
        ui.label(text::show_rules_for());
        let shown = if state.sheet {
            text::this_sheet()
        } else {
            text::current_selection()
        };
        ComboBox::from_id_salt("cf-scope")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.sheet, false, text::current_selection());
                ui.selectable_value(&mut state.sheet, true, text::this_sheet());
            });
    });
    ui.horizontal(|ui| {
        if ui.add(plain_button(text::new_rule())).clicked() {
            action = Some(ManageAction::New);
        }
        let chosen = state.selected;
        if ui
            .add_enabled(chosen.is_some(), plain_button(text::edit_rule()))
            .clicked()
            && let Some(index) = chosen
        {
            action = Some(ManageAction::Edit(index));
        }
        if ui
            .add_enabled(chosen.is_some(), plain_button(text::delete_rule()))
            .clicked()
            && let Some(index) = chosen
        {
            state.rules.remove(index);
            state.ranges.remove(index);
            state.selected = None;
        }
        if ui
            .add_enabled(chosen.is_some(), plain_button(text::duplicate_rule()))
            .clicked()
            && let Some(index) = chosen
        {
            state.rules.insert(index + 1, state.rules[index].clone());
            state.ranges.insert(index + 1, state.ranges[index].clone());
            state.selected = Some(index + 1);
        }
        let up = Icon::Plain(crate::fluent::CHEVRON_UP);
        let down = Icon::Plain(crate::fluent::CHEVRON_DOWN);
        let last = state.rules.len().saturating_sub(1);
        if crate::ribbon::widgets::tool(ui, up, text::move_up(), false).clicked()
            && let Some(index) = chosen.filter(|i| *i < last)
        {
            state.rules.swap(index, index + 1);
            state.ranges.swap(index, index + 1);
            state.selected = Some(index + 1);
        }
        if crate::ribbon::widgets::tool(ui, down, text::move_down(), false).clicked()
            && let Some(index) = chosen.filter(|i| *i > 0)
        {
            state.rules.swap(index, index - 1);
            state.ranges.swap(index, index - 1);
            state.selected = Some(index - 1);
        }
    });
    let header = |ui: &mut Ui, label: &str, width: f32| {
        ui.add_sized(
            vec2(width, theme::INPUT_HEIGHT),
            egui::Label::new(RichText::new(label).color(theme::chrome_text_muted())),
        );
    };
    ui.horizontal(|ui| {
        header(ui, text::rule_column(), theme::CF_RULE_COLUMN);
        header(ui, text::format_column(), theme::CF_FORMAT_SAMPLE_WIDTH);
        header(ui, text::applies_to(), theme::CF_RANGE_COLUMN);
        header(ui, text::stop_if_true(), theme::CF_STOP_COLUMN);
    });
    list_box(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        for index in (0..state.rules.len()).rev() {
            if !state.sheet
                && !vellum_grid_core::overlaps(state.rules[index].range, state.selection)
            {
                continue;
            }
            ui.horizontal(|ui| {
                let selected = state.selected == Some(index);
                let label = describe(&state.rules[index].rule);
                let item = ui.add_sized(
                    vec2(theme::CF_RULE_COLUMN, theme::CF_FORMAT_SAMPLE_HEIGHT),
                    egui::Button::selectable(selected, label),
                );
                if item.clicked() {
                    state.selected = Some(index);
                }
                if item.double_clicked() {
                    action = Some(ManageAction::Edit(index));
                }
                format_sample(ui, &state.rules[index]);
                ui.add(
                    TextEdit::singleline(&mut state.ranges[index])
                        .desired_width(theme::CF_RANGE_COLUMN),
                );
                let rule = &mut state.rules[index];
                ui.add_enabled_ui(!rule.rule.is_graphic(), |ui| {
                    ui.checkbox(&mut rule.stop, "");
                });
            });
        }
    });
    if state.error {
        ui.label(RichText::new(text::bad_rule_range()).color(theme::error_text()));
    }
    action
}

fn point_row(ui: &mut Ui, id: &str, point: &mut Point, kinds: &[PointKind], color: bool) {
    ui.horizontal(|ui| {
        ComboBox::from_id_salt(format!("{id}-kind"))
            .width(theme::CF_POINT_KIND_WIDTH)
            .selected_text(point.kind.label())
            .show_ui(ui, |ui| {
                for kind in kinds {
                    ui.selectable_value(&mut point.kind, *kind, kind.label());
                }
            });
        let editable = matches!(
            point.kind,
            PointKind::Number | PointKind::Percent | PointKind::Percentile
        );
        ui.add_enabled(
            editable,
            TextEdit::singleline(&mut point.value).desired_width(theme::RULE_INPUT_WIDTH),
        );
        if color
            && let Some(choice) = color_button(
                ui,
                &format!("{id}-color"),
                Some(point.color),
                text::automatic(),
            )
        {
            point.color = choice.unwrap_or(Color::BLACK);
        }
    });
}

fn format_editor(ui: &mut Ui, format: &mut Highlight) {
    ui.label(RichText::new(text::format_section()).color(theme::chrome_text_muted()));
    ui.horizontal(|ui| {
        ui.checkbox(&mut format.bold, text::bold());
        ui.checkbox(&mut format.italic, text::italic());
        ui.checkbox(&mut format.strike, text::strikethrough());
    });
    ui.horizontal(|ui| {
        ui.label(text::font_color());
        if let Some(choice) = color_button(ui, "cf-font", format.color, text::automatic()) {
            format.color = choice;
        }
        ui.label(text::fill_color());
        if let Some(choice) = color_button(ui, "cf-fill", format.fill, text::no_color()) {
            format.fill = choice;
        }
        ui.label(text::border());
        let current = format.border.map(|b| b.color);
        if let Some(choice) = color_button(ui, "cf-border", current, text::none()) {
            format.border = choice.map(|color| Border {
                style: LineStyle::Thin,
                color,
            });
        }
    });
    let (rect, _) = ui.allocate_exact_size(
        vec2(
            theme::CF_PREVIEW_WIDTH,
            theme::CF_FORMAT_SAMPLE_HEIGHT * 1.5,
        ),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, theme::cell_bg());
    highlight_sample(&painter, rect, format);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
}

pub fn rule_ui(ui: &mut Ui, form: &mut RuleForm) {
    ui.set_width(theme::CF_RULE_WIDTH);
    ui.label(text::select_rule_type());
    egui::Frame::NONE
        .stroke(Stroke::new(1.0, theme::chrome_line()))
        .fill(theme::input_bg())
        .inner_margin(egui::Margin::same(4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for kind in RuleType::ALL {
                if ui
                    .selectable_label(form.kind == kind, kind.label())
                    .clicked()
                {
                    form.kind = kind;
                    form.error = false;
                }
            }
        });
    ui.label(text::edit_rule_description());
    let all = [
        PointKind::Lowest,
        PointKind::Highest,
        PointKind::Number,
        PointKind::Percent,
        PointKind::Percentile,
    ];
    match form.kind {
        RuleType::Graded => {
            ComboBox::from_id_salt("cf-graded")
                .width(theme::CF_POINT_KIND_WIDTH)
                .selected_text(match form.graded {
                    Graded::TwoColor => text::scale_two(),
                    Graded::ThreeColor => text::scale_three(),
                    Graded::Bar => text::data_bar(),
                    Graded::Icons => text::icon_set(),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.graded, Graded::TwoColor, text::scale_two());
                    ui.selectable_value(&mut form.graded, Graded::ThreeColor, text::scale_three());
                    ui.selectable_value(&mut form.graded, Graded::Bar, text::data_bar());
                    ui.selectable_value(&mut form.graded, Graded::Icons, text::icon_set());
                });
            match form.graded {
                Graded::TwoColor | Graded::ThreeColor => {
                    ui.label(text::minimum());
                    point_row(ui, "cf-min", &mut form.points[0], &all, true);
                    if form.graded == Graded::ThreeColor {
                        ui.label(text::midpoint());
                        point_row(ui, "cf-mid", &mut form.points[1], &all[2..], true);
                    }
                    ui.label(text::maximum());
                    point_row(ui, "cf-max", &mut form.points[2], &all, true);
                }
                Graded::Bar => {
                    let kinds = [
                        PointKind::Automatic,
                        PointKind::Lowest,
                        PointKind::Highest,
                        PointKind::Number,
                        PointKind::Percent,
                        PointKind::Percentile,
                    ];
                    ui.label(text::minimum());
                    point_row(ui, "cf-bar-min", &mut form.points[0], &kinds, false);
                    ui.label(text::maximum());
                    point_row(ui, "cf-bar-max", &mut form.points[2], &kinds, false);
                    ui.horizontal(|ui| {
                        ui.label(text::bar_color());
                        if let Some(choice) = color_button(
                            ui,
                            "cf-bar-color",
                            Some(form.bar_color),
                            text::automatic(),
                        ) {
                            form.bar_color = choice.unwrap_or(Color(BAR_COLORS[0]));
                        }
                        ui.checkbox(&mut form.gradient, text::gradient_fill());
                    });
                    ui.checkbox(&mut form.show_value, text::show_value());
                }
                Graded::Icons => {
                    ComboBox::from_id_salt("cf-icon-kind")
                        .width(theme::CF_POINT_KIND_WIDTH)
                        .selected_text(text::icon_style())
                        .show_ui(ui, |ui| {
                            for kind in IconKind::ALL {
                                if icon_row(ui, kind).clicked() {
                                    form.icon = kind;
                                    form.icon_points = icon_points(kind);
                                }
                            }
                        });
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(theme::CF_ICON_SIZE * 6.0, theme::CF_ICON_SIZE + 4.0),
                        Sense::hover(),
                    );
                    let n = form.icon.count();
                    for i in 0..n {
                        let r = Rect::from_min_size(
                            rect.min + vec2(i as f32 * (theme::CF_ICON_SIZE + 2.0), 2.0),
                            vec2(theme::CF_ICON_SIZE, theme::CF_ICON_SIZE),
                        );
                        let index = if form.reverse { i } else { n - 1 - i };
                        paint_icon(ui.painter(), r, form.icon, index);
                    }
                    ui.checkbox(&mut form.reverse, text::reverse_icons());
                    ui.checkbox(&mut form.show_value, text::show_value());
                    let kinds = [PointKind::Number, PointKind::Percent, PointKind::Percentile];
                    for (i, point) in form.icon_points.iter_mut().enumerate().rev() {
                        ui.label(format!("{} {}", text::icon_when(), i + 2));
                        point_row(ui, &format!("cf-icon-{i}"), point, &kinds, false);
                    }
                }
            }
        }
        RuleType::Cell => {
            ui.horizontal(|ui| {
                ComboBox::from_id_salt("cf-cell-kind")
                    .width(theme::CF_POINT_KIND_WIDTH)
                    .selected_text(form.cell.label())
                    .show_ui(ui, |ui| {
                        for kind in CellKind::ALL {
                            ui.selectable_value(&mut form.cell, kind, kind.label());
                        }
                    });
                match form.cell {
                    CellKind::Value => {
                        ComboBox::from_id_salt("cf-operator")
                            .selected_text(OPERATORS[form.operator]())
                            .show_ui(ui, |ui| {
                                for (i, label) in OPERATORS.iter().enumerate() {
                                    ui.selectable_value(&mut form.operator, i, label());
                                }
                            });
                        ui.add(
                            TextEdit::singleline(&mut form.first)
                                .desired_width(theme::RULE_INPUT_WIDTH),
                        );
                        if form.operator < 2 {
                            ui.label(text::and());
                            ui.add(
                                TextEdit::singleline(&mut form.second)
                                    .desired_width(theme::RULE_INPUT_WIDTH),
                            );
                        }
                    }
                    CellKind::Text => {
                        ComboBox::from_id_salt("cf-text-operator")
                            .selected_text(TEXT_OPERATORS[form.text_operator]())
                            .show_ui(ui, |ui| {
                                for (i, label) in TEXT_OPERATORS.iter().enumerate() {
                                    ui.selectable_value(&mut form.text_operator, i, label());
                                }
                            });
                        ui.add(
                            TextEdit::singleline(&mut form.text)
                                .desired_width(theme::RULE_INPUT_WIDTH),
                        );
                    }
                    CellKind::Dates => {
                        ComboBox::from_id_salt("cf-period")
                            .selected_text(period_label(DatePeriod::ALL[form.period]))
                            .show_ui(ui, |ui| {
                                for (i, p) in DatePeriod::ALL.iter().enumerate() {
                                    ui.selectable_value(&mut form.period, i, period_label(*p));
                                }
                            });
                    }
                    _ => {}
                }
            });
            format_editor(ui, &mut form.format);
        }
        RuleType::TopBottom => {
            ui.horizontal(|ui| {
                ComboBox::from_id_salt("cf-top")
                    .selected_text(if form.bottom {
                        text::bottom()
                    } else {
                        text::top()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut form.bottom, false, text::top());
                        ui.selectable_value(&mut form.bottom, true, text::bottom());
                    });
                ui.add(
                    TextEdit::singleline(&mut form.count).desired_width(theme::RULE_INPUT_WIDTH),
                );
                ui.checkbox(&mut form.percent, text::percent_of_range());
            });
            format_editor(ui, &mut form.format);
        }
        RuleType::Average => {
            ComboBox::from_id_salt("cf-average")
                .width(theme::CF_POINT_KIND_WIDTH)
                .selected_text(AVERAGES[form.average].0())
                .show_ui(ui, |ui| {
                    for (i, (label, ..)) in AVERAGES.iter().enumerate() {
                        ui.selectable_value(&mut form.average, i, label());
                    }
                });
            format_editor(ui, &mut form.format);
        }
        RuleType::Unique => {
            ComboBox::from_id_salt("cf-unique")
                .selected_text(if form.duplicate {
                    text::duplicate_word()
                } else {
                    text::unique_word()
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.duplicate, true, text::duplicate_word());
                    ui.selectable_value(&mut form.duplicate, false, text::unique_word());
                });
            format_editor(ui, &mut form.format);
        }
        RuleType::Formula => {
            ui.label(text::formula_prompt());
            ui.add(
                TextEdit::singleline(&mut form.formula)
                    .desired_width(f32::INFINITY)
                    .background_color(theme::input_bg()),
            );
            format_editor(ui, &mut form.format);
        }
    }
    if form.error {
        ui.label(RichText::new(text::bad_rule()).color(theme::error_text()));
    }
}

impl Spreadsheet {
    pub fn conditional_menu(&mut self, ui: &mut Ui) {
        menu::sub(ui, Some(Icon::Conditional), text::highlight_rules(), |ui| {
            for kind in RuleKind::HIGHLIGHT {
                if menu::item(ui, &format!("{}…", kind.label())) {
                    self.open_rule_dialog(kind);
                }
            }
        });
        menu::sub(
            ui,
            Some(Icon::Conditional),
            text::top_bottom_rules(),
            |ui| {
                for kind in RuleKind::TOP_BOTTOM {
                    if menu::item(ui, &format!("{}…", kind.label())) {
                        self.open_rule_dialog(kind);
                    }
                }
            },
        );
        menu::separator(ui);
        menu::sub(ui, Some(Icon::Conditional), text::data_bars(), |ui| {
            for (label, gradient) in [(text::gradient_fill(), true), (text::solid_fill(), false)] {
                ui.label(crate::ribbon::widgets::chrome_text(label));
                ui.horizontal(|ui| {
                    for color in BAR_COLORS {
                        if gallery_bar(ui, color, gradient).clicked() {
                            let rule = Rule::DataBar(DataBar::new(Color(color), gradient));
                            self.add_graphic(rule);
                            ui.close();
                        }
                    }
                });
            }
        });
        menu::sub(ui, Some(Icon::Conditional), text::color_scales(), |ui| {
            for row in SCALES.chunks(4) {
                ui.horizontal(|ui| {
                    for colors in row {
                        if gallery_scale(ui, colors).clicked() {
                            self.add_graphic(Rule::ColorScale(scale_rule(colors)));
                            ui.close();
                        }
                    }
                });
            }
        });
        menu::sub(ui, Some(Icon::Conditional), text::icon_sets(), |ui| {
            for kind in IconKind::ALL {
                if icon_row(ui, kind).clicked() {
                    self.add_graphic(Rule::IconSet(IconSet::new(kind)));
                    ui.close();
                }
            }
        });
        menu::separator(ui);
        if menu::item(ui, text::new_rule()) {
            self.dialog = Some(Dialog::ConditionalRule(Box::new(RuleForm::new(
                self.selected(),
            ))));
        }
        menu::sub(ui, Some(Icon::Clear), text::clear_rules(), |ui| {
            if menu::item(ui, text::clear_rules_selection()) {
                let range = self.selected();
                self.doc.set_layout(|layout| {
                    layout
                        .conditional
                        .retain(|format| !vellum_grid_core::overlaps(format.range, range));
                });
            }
            if menu::item(ui, text::clear_rules_sheet()) {
                self.doc.set_layout(|layout| layout.conditional.clear());
            }
        });
        if menu::item(ui, text::manage_rules()) {
            self.open_manage_rules();
        }
    }

    fn add_graphic(&mut self, rule: Rule) {
        let format = ConditionalFormat::new(self.selected(), rule, Highlight::default());
        self.doc
            .set_layout(|layout| layout.conditional.push(format));
    }

    pub fn open_manage_rules(&mut self) {
        let rules = self.sheet().layout().conditional.clone();
        let ranges = rules.iter().map(|r| absolute(r.range)).collect();
        self.dialog = Some(Dialog::ManageRules(Box::new(ManageState {
            rules,
            ranges,
            selection: self.selected(),
            sheet: false,
            selected: None,
            error: false,
        })));
    }

    pub fn manage_action(&mut self, state: ManageState, action: ManageAction) {
        let mut state = state;
        for (rule, text) in state.rules.iter_mut().zip(&state.ranges) {
            if let Some(range) = parse_range(text) {
                rule.range = range;
            }
        }
        let form = match action {
            ManageAction::New => RuleForm::new(state.selection),
            ManageAction::Edit(index) => RuleForm::from_format(&state.rules[index], index),
        };
        self.dialog = Some(Dialog::ConditionalRule(Box::new(RuleForm {
            back: Some(Box::new(state)),
            ..form
        })));
    }

    pub fn apply_manage(&mut self, mut state: ManageState) {
        let mut rules = state.rules.clone();
        for (rule, text) in rules.iter_mut().zip(&state.ranges) {
            match parse_range(text) {
                Some(range) => rule.range = range,
                None => {
                    state.error = true;
                    self.dialog = Some(Dialog::ManageRules(Box::new(state)));
                    return;
                }
            }
        }
        self.doc.set_layout(|layout| layout.conditional = rules);
    }

    pub fn close_rule_form(&mut self, form: RuleForm) {
        if let Some(back) = form.back {
            self.dialog = Some(Dialog::ManageRules(back));
        }
    }

    pub fn apply_rule_form(&mut self, mut form: RuleForm) {
        let Some(rule) = form.rule(locale()) else {
            form.error = true;
            self.dialog = Some(Dialog::ConditionalRule(Box::new(form)));
            return;
        };
        let mut format = ConditionalFormat::new(form.range, rule, form.format);
        format.stop = form.stop && !format.rule.is_graphic();
        if format.rule.is_graphic() {
            format.format = Highlight::default();
        }
        match form.back.take() {
            Some(mut back) => {
                match form.editing {
                    Some(index) => {
                        format.range = back.rules[index].range;
                        back.rules[index] = format;
                    }
                    None => {
                        back.ranges.push(absolute(format.range));
                        back.rules.push(format);
                        back.selected = Some(back.rules.len() - 1);
                    }
                }
                self.dialog = Some(Dialog::ManageRules(back));
            }
            None => {
                self.doc
                    .set_layout(|layout| layout.conditional.push(format));
            }
        }
    }
}

pub fn paint_extras(
    painter: &Painter,
    rect: Rect,
    bar: Option<&Bar>,
    icon: Option<(IconKind, usize)>,
) {
    if let Some(bar) = bar {
        paint_bar(painter, rect, bar);
    }
    if let Some((kind, index)) = icon {
        paint_icon(painter, icon_rect(rect), kind, index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_grid_core::TR;

    #[test]
    fn rule_forms_round_trip_rules() {
        let range: CellRange = "A1:B5".parse().unwrap();
        let rules = [
            Rule::NotBetween(1.0, 2.5),
            Rule::NotEqual(CellValue::Text("x".into())),
            Rule::EndsWith("ar".into()),
            Rule::Dates(DatePeriod::NextWeek),
            Rule::Top {
                count: 3,
                percent: true,
                bottom: true,
            },
            Rule::Average {
                below: true,
                equal: false,
                deviations: 2,
            },
            Rule::Unique,
            Rule::Expression("AND(A1>0,B1<5)".into()),
            Rule::DataBar(DataBar::new(Color(0x638EC6), false)),
            Rule::ColorScale(scale_rule(&[GREEN, YELLOW, RED])),
            Rule::IconSet(IconSet::new(IconKind::Rating5)),
        ];
        for rule in rules {
            let format = ConditionalFormat::new(range, rule.clone(), Highlight::default());
            let form = RuleForm::from_format(&format, 0);
            assert_eq!(form.rule(&TR), Some(rule));
        }
    }

    #[test]
    fn scales_put_the_first_color_on_top() {
        let scale = scale_rule(&[GREEN, YELLOW, RED]);
        assert_eq!(scale.points[0], (Threshold::Lowest, Color(RED)));
        assert_eq!(scale.points[2], (Threshold::Highest, Color(GREEN)));
        assert!(describe(&Rule::Greater(1.5)).contains("1,5"));
    }
}
