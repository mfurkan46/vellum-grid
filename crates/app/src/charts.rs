use std::f32::consts::{FRAC_PI_2, TAU};

use eframe::egui::epaint::TextShape;
use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Galley, Mesh, Painter, Pos2, Rect, Shape, Stroke,
    StrokeKind, pos2, vec2,
};
use vellum_grid_core::{
    Chart, ChartData, ChartKind, Grouping, Legend, Series, nice_scale, round_decimal, stacked,
};

use crate::app::locale;
use crate::{text, theme};

pub fn title(chart: &Chart, data: &ChartData) -> String {
    chart
        .title
        .clone()
        .unwrap_or_else(|| match data.series.as_slice() {
            [single] => single.name.clone(),
            [first, ..] if chart.kind.is_round() => first.name.clone(),
            _ => text::chart_title().to_owned(),
        })
}

pub fn paint(painter: &Painter, rect: Rect, chart: &Chart, data: &ChartData) {
    painter.rect(
        rect,
        CornerRadius::ZERO,
        theme::CHART_BG,
        Stroke::new(1.0, theme::CHART_BORDER),
        StrokeKind::Inside,
    );
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let inner = rect.shrink(theme::CHART_PADDING);
    let heading = painter.layout_no_wrap(
        title(chart, data),
        FontId::proportional(theme::CHART_TITLE_SIZE),
        theme::CHART_TEXT,
    );
    let heading_h = heading.size().y;
    painter.galley(
        pos2(inner.center().x - heading.size().x / 2.0, inner.top()),
        heading,
        theme::CHART_TEXT,
    );
    let mut plot = Rect::from_min_max(
        pos2(inner.left(), inner.top() + heading_h + theme::CHART_PADDING),
        inner.max,
    );
    if chart.legend_shown(data) {
        let entries: Vec<String> = if chart.kind.is_round() {
            data.categories.clone()
        } else {
            data.series.iter().map(|s| s.name.clone()).collect()
        };
        plot = legend(&painter, plot, &entries, chart.legend);
    }
    if !chart.kind.is_round() {
        if let Some(name) = &chart.x_title {
            let galley = painter.layout_no_wrap(name.clone(), font(), theme::CHART_TEXT);
            let h = galley.size().y;
            painter.galley(
                pos2(plot.center().x - galley.size().x / 2.0, plot.bottom() - h),
                galley,
                theme::CHART_TEXT,
            );
            plot.max.y -= h + 4.0;
        }
        if let Some(name) = &chart.y_title {
            let galley = painter.layout_no_wrap(name.clone(), font(), theme::CHART_TEXT);
            let (w, h) = (galley.size().x, galley.size().y);
            let anchor = pos2(plot.left(), plot.center().y + w / 2.0);
            painter.add(TextShape::new(anchor, galley, theme::CHART_TEXT).with_angle(-FRAC_PI_2));
            plot.min.x += h + 4.0;
        }
    }
    if plot.width() < 20.0 || plot.height() < 20.0 {
        return;
    }
    match chart.kind {
        ChartKind::Pie | ChartKind::Doughnut => pie(&painter, plot, data, chart),
        ChartKind::Scatter => scatter(&painter, plot, data, chart),
        _ => categories(&painter, plot, data, chart),
    }
}

fn color(index: usize) -> Color32 {
    theme::CHART_COLORS[index % theme::CHART_COLORS.len()]
}

fn font() -> FontId {
    FontId::proportional(theme::CHART_LABEL_SIZE)
}

fn label(value: f64) -> String {
    locale().format_number(round_decimal(value, 10))
}

fn percent(value: f64) -> String {
    text::percent_of(locale().format_number(round_decimal(value * 100.0, 6)))
}

fn legend_entry(painter: &Painter, x: f32, y: f32, index: usize, galley: std::sync::Arc<Galley>) {
    let swatch = theme::CHART_SWATCH;
    let square = Rect::from_center_size(pos2(x + swatch / 2.0, y), vec2(swatch, swatch));
    painter.rect_filled(square, CornerRadius::ZERO, color(index));
    painter.galley(
        pos2(x + swatch + 4.0, y - galley.size().y / 2.0),
        galley,
        theme::CHART_TEXT,
    );
}

fn legend(painter: &Painter, area: Rect, entries: &[String], position: Legend) -> Rect {
    let galleys: Vec<_> = entries
        .iter()
        .map(|entry| painter.layout_no_wrap(entry.clone(), font(), theme::CHART_TEXT))
        .collect();
    let swatch = theme::CHART_SWATCH;
    let gap = theme::CHART_PADDING;
    match position {
        Legend::Left | Legend::Right => {
            let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + swatch + 4.0;
            let line = theme::CHART_LEGEND_HEIGHT;
            let total = line * galleys.len() as f32;
            let x = if position == Legend::Left {
                area.left()
            } else {
                area.right() - width
            };
            let mut y = area.center().y - total / 2.0 + line / 2.0;
            for (index, galley) in galleys.into_iter().enumerate() {
                legend_entry(painter, x, y, index, galley);
                y += line;
            }
            if position == Legend::Left {
                Rect::from_min_max(pos2(area.left() + width + gap, area.top()), area.max)
            } else {
                Rect::from_min_max(area.min, pos2(area.right() - width - gap, area.bottom()))
            }
        }
        _ => {
            let total: f32 = galleys
                .iter()
                .map(|g| swatch + 4.0 + g.size().x + gap)
                .sum::<f32>()
                - gap;
            let mut x = area.center().x - total / 2.0;
            let top = position == Legend::Top;
            let y = if top {
                area.top() + theme::CHART_LEGEND_HEIGHT / 2.0
            } else {
                area.bottom() - theme::CHART_LEGEND_HEIGHT / 2.0
            };
            for (index, galley) in galleys.into_iter().enumerate() {
                let width = galley.size().x;
                legend_entry(painter, x, y, index, galley);
                x += swatch + 4.0 + width + gap;
            }
            if top {
                Rect::from_min_max(
                    pos2(area.left(), area.top() + theme::CHART_LEGEND_HEIGHT),
                    area.max,
                )
            } else {
                Rect::from_min_max(
                    area.min,
                    pos2(area.right(), area.bottom() - theme::CHART_LEGEND_HEIGHT),
                )
            }
        }
    }
}

struct Scale {
    low: f64,
    high: f64,
    step: f64,
    percent: bool,
}

impl Scale {
    fn of(values: impl Iterator<Item = f64>, include_zero: bool) -> Self {
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for value in values {
            min = min.min(value);
            max = max.max(value);
        }
        if !min.is_finite() {
            (min, max) = (0.0, 1.0);
        }
        if include_zero {
            min = min.min(0.0);
            max = max.max(0.0);
        }
        let (low, high, step) = nice_scale(min, max, theme::CHART_TICKS);
        Self {
            low,
            high,
            step,
            percent: false,
        }
    }

    fn ticks(&self) -> Vec<f64> {
        let count = ((self.high - self.low) / self.step).round() as usize;
        (0..=count)
            .map(|i| self.low + self.step * i as f64)
            .collect()
    }

    fn fraction(&self, value: f64) -> f32 {
        ((value - self.low) / (self.high - self.low)) as f32
    }

    fn text(&self, value: f64) -> String {
        if self.percent {
            percent(value)
        } else {
            label(value)
        }
    }

    fn label_width(&self, painter: &Painter) -> f32 {
        self.ticks()
            .into_iter()
            .map(|tick| {
                painter
                    .layout_no_wrap(self.text(tick), font(), theme::CHART_TEXT)
                    .size()
                    .x
            })
            .fold(0.0, f32::max)
    }
}

fn categories(painter: &Painter, plot: Rect, data: &ChartData, chart: &Chart) {
    let kind = chart.kind;
    let grouping = chart.grouping;
    let spans = stacked(data, grouping);
    let mut scale = Scale::of(
        spans.iter().flatten().flatten().flat_map(|(a, b)| [*a, *b]),
        true,
    );
    if grouping == Grouping::Percent {
        let negative = spans
            .iter()
            .flatten()
            .flatten()
            .any(|(a, b)| a.min(*b) < 0.0);
        let (low, high, step) =
            nice_scale(if negative { -1.0 } else { 0.0 }, 1.0, theme::CHART_TICKS);
        scale = Scale {
            low,
            high,
            step,
            percent: true,
        };
    }
    let horizontal = kind == ChartKind::Bar;
    let count = data.categories.len().max(1);
    let line = Stroke::new(1.0, theme::CHART_GRID);
    let axis = Stroke::new(1.0, theme::CHART_AXIS);
    let (area, along) = if horizontal {
        let width = data
            .categories
            .iter()
            .map(|c| {
                painter
                    .layout_no_wrap(c.clone(), font(), theme::CHART_TEXT)
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let area = Rect::from_min_max(
            pos2(plot.left() + width + 6.0, plot.top()),
            pos2(plot.right() - 6.0, plot.bottom() - theme::CHART_AXIS_LABEL),
        );
        (area, area.height() / count as f32)
    } else {
        let area = Rect::from_min_max(
            pos2(plot.left() + scale.label_width(painter) + 6.0, plot.top()),
            pos2(plot.right(), plot.bottom() - theme::CHART_AXIS_LABEL),
        );
        (area, area.width() / count as f32)
    };
    for tick in scale.ticks() {
        let f = scale.fraction(tick);
        if horizontal {
            let x = area.left() + f * area.width();
            if chart.gridlines {
                painter.vline(x, area.y_range(), line);
            }
            painter.text(
                pos2(x, area.bottom() + 3.0),
                Align2::CENTER_TOP,
                scale.text(tick),
                font(),
                theme::CHART_TEXT,
            );
        } else {
            let y = area.bottom() - f * area.height();
            if chart.gridlines {
                painter.hline(area.x_range(), y, line);
            }
            painter.text(
                pos2(area.left() - 6.0, y),
                Align2::RIGHT_CENTER,
                scale.text(tick),
                font(),
                theme::CHART_TEXT,
            );
        }
    }
    let zero = scale.fraction(0.0_f64.clamp(scale.low, scale.high));
    if horizontal {
        painter.vline(area.left() + zero * area.width(), area.y_range(), axis);
    } else {
        painter.hline(area.x_range(), area.bottom() - zero * area.height(), axis);
    }
    let every = label_stride(painter, &data.categories, along, horizontal);
    for (index, category) in data.categories.iter().enumerate() {
        if index % every != 0 {
            continue;
        }
        if horizontal {
            let y = area.bottom() - along * (index as f32 + 0.5);
            painter.text(
                pos2(area.left() - 6.0, y),
                Align2::RIGHT_CENTER,
                category,
                font(),
                theme::CHART_TEXT,
            );
        } else {
            let x = area.left() + along * (index as f32 + 0.5);
            painter.text(
                pos2(x, area.bottom() + 3.0),
                Align2::CENTER_TOP,
                category,
                font(),
                theme::CHART_TEXT,
            );
        }
    }
    let side = |index: usize| area.left() + along * (index as f32 + 0.5);
    let height = |value: f64| area.bottom() - scale.fraction(value) * area.height();
    let clustered = grouping == Grouping::Clustered;
    let slots = if clustered {
        data.series.len().max(1) as f32
    } else {
        1.0
    };
    let bar = along / (slots + theme::CHART_GAP);
    let mut labels: Vec<(Pos2, Align2, String)> = Vec::new();
    let shown = |series: &Series, index: usize| {
        label(series.values.get(index).copied().flatten().unwrap_or(0.0))
    };
    for (s, item) in data.series.iter().enumerate() {
        let stroke = Stroke::new(theme::CHART_LINE_WIDTH, color(s));
        let mut points = Vec::new();
        let series_spans = &spans[s];
        if kind == ChartKind::Area {
            for index in 0..series_spans.len().saturating_sub(1) {
                let (Some((b0, t0)), Some((b1, t1))) =
                    (series_spans[index], series_spans[index + 1])
                else {
                    continue;
                };
                let quad = vec![
                    pos2(side(index), height(t0)),
                    pos2(side(index + 1), height(t1)),
                    pos2(side(index + 1), height(b1)),
                    pos2(side(index), height(b0)),
                ];
                painter.add(Shape::convex_polygon(quad, color(s), Stroke::NONE));
            }
        }
        for (index, span) in series_spans.iter().enumerate() {
            let Some((base, top)) = span else {
                if !points.is_empty() {
                    painter.add(Shape::line(std::mem::take(&mut points), stroke));
                }
                continue;
            };
            match kind {
                ChartKind::Area => {
                    if chart.labels {
                        labels.push((
                            pos2(side(index), height(*top) - 2.0),
                            Align2::CENTER_BOTTOM,
                            shown(item, index),
                        ));
                    }
                }
                ChartKind::Line => {
                    let point = pos2(side(index), height(*top));
                    points.push(point);
                    if chart.markers {
                        painter.circle_filled(point, theme::CHART_MARKER, color(s));
                    }
                    if chart.labels {
                        labels.push((
                            point - vec2(0.0, 6.0),
                            Align2::CENTER_BOTTOM,
                            shown(item, index),
                        ));
                    }
                }
                ChartKind::Bar => {
                    let slot = if clustered { s as f32 } else { 0.0 };
                    let center = area.bottom() - along * index as f32 - along / 2.0
                        + bar * slots / 2.0
                        - bar * slot;
                    let x0 = area.left() + scale.fraction(*base) * area.width();
                    let x1 = area.left() + scale.fraction(*top) * area.width();
                    let r = Rect::from_min_max(
                        pos2(x0.min(x1), center - bar),
                        pos2(x0.max(x1), center),
                    );
                    painter.rect_filled(r, CornerRadius::ZERO, color(s));
                    if chart.labels {
                        let (pos, align) = if clustered {
                            (pos2(r.right() + 3.0, r.center().y), Align2::LEFT_CENTER)
                        } else {
                            (r.center(), Align2::CENTER_CENTER)
                        };
                        labels.push((pos, align, shown(item, index)));
                    }
                }
                _ => {
                    let slot = if clustered { s as f32 } else { 0.0 };
                    let left = area.left() + along * index as f32 + along / 2.0 - bar * slots / 2.0
                        + bar * slot;
                    let (y0, y1) = (height(*base), height(*top));
                    let r =
                        Rect::from_min_max(pos2(left, y0.min(y1)), pos2(left + bar, y0.max(y1)));
                    painter.rect_filled(r, CornerRadius::ZERO, color(s));
                    if chart.labels {
                        let (pos, align) = if clustered {
                            (pos2(r.center().x, r.top() - 2.0), Align2::CENTER_BOTTOM)
                        } else {
                            (r.center(), Align2::CENTER_CENTER)
                        };
                        labels.push((pos, align, shown(item, index)));
                    }
                }
            }
        }
        if points.len() > 1 {
            painter.add(Shape::line(points, stroke));
        }
    }
    for (pos, align, value) in labels {
        painter.text(pos, align, value, font(), theme::CHART_TEXT);
    }
}

fn label_stride(painter: &Painter, labels: &[String], space: f32, horizontal: bool) -> usize {
    let size = labels
        .iter()
        .map(|l| {
            let s = painter
                .layout_no_wrap(l.clone(), font(), theme::CHART_TEXT)
                .size();
            if horizontal { s.y } else { s.x + 6.0 }
        })
        .fold(1.0, f32::max);
    ((size / space.max(1.0)).ceil() as usize).max(1)
}

fn pie(painter: &Painter, plot: Rect, data: &ChartData, chart: &Chart) {
    let Some(series) = data.series.first() else {
        return;
    };
    let values: Vec<f64> = series
        .values
        .iter()
        .map(|v| v.unwrap_or(0.0).max(0.0))
        .collect();
    let total: f64 = values.iter().sum();
    if total <= 0.0 {
        return;
    }
    let center = plot.center();
    let radius = plot.width().min(plot.height()) / 2.0 - 2.0;
    let mut angle = -FRAC_PI_2;
    let point = |a: f32, r: f32| center + vec2(a.cos(), a.sin()) * r;
    let mut edges = Vec::new();
    let mut labels = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let sweep = (*value / total) as f32 * TAU;
        if sweep > 0.0 {
            let segments = ((sweep / TAU) * 96.0).ceil().max(1.0) as u32;
            let arc: Vec<Pos2> = (0..=segments)
                .map(|i| point(angle + sweep * i as f32 / segments as f32, radius))
                .collect();
            let mut mesh = Mesh::default();
            mesh.colored_vertex(center, color(index));
            for p in &arc {
                mesh.colored_vertex(*p, color(index));
            }
            for i in 1..=segments {
                mesh.add_triangle(0, i, i + 1);
            }
            painter.add(Shape::mesh(mesh));
            painter.line(arc, Stroke::new(1.0, color(index)));
        }
        if *value > 0.0 {
            let middle = if chart.kind == ChartKind::Doughnut {
                0.75
            } else {
                0.65
            };
            labels.push((point(angle + sweep / 2.0, radius * middle), label(*value)));
        }
        edges.push(angle);
        angle += sweep;
    }
    if values.iter().filter(|v| **v > 0.0).count() > 1 {
        for edge in edges {
            painter.line_segment(
                [center, point(edge, radius)],
                Stroke::new(1.5, theme::CHART_BG),
            );
        }
    }
    if chart.kind == ChartKind::Doughnut {
        painter.circle_filled(center, radius * theme::DOUGHNUT_HOLE, theme::CHART_BG);
    }
    if chart.labels {
        for (pos, value) in labels {
            painter.text(pos, Align2::CENTER_CENTER, value, font(), Color32::WHITE);
        }
    }
}

fn scatter(painter: &Painter, plot: Rect, data: &ChartData, chart: &Chart) {
    let (xs, plotted): (Vec<f64>, &[Series]) =
        if data.categories_range.is_none() && data.series.len() > 1 {
            let xs = data.series[0]
                .values
                .iter()
                .enumerate()
                .map(|(i, n)| n.unwrap_or(i as f64 + 1.0))
                .collect();
            (xs, &data.series[1..])
        } else {
            let xs = data
                .numbers
                .iter()
                .enumerate()
                .map(|(i, n)| n.unwrap_or(i as f64 + 1.0))
                .collect();
            (xs, &data.series)
        };
    let x_scale = Scale::of(xs.iter().copied(), false);
    let y_scale = Scale::of(
        plotted
            .iter()
            .flat_map(|s| s.values.iter().flatten().copied()),
        true,
    );
    let area = Rect::from_min_max(
        pos2(plot.left() + y_scale.label_width(painter) + 6.0, plot.top()),
        pos2(plot.right() - 10.0, plot.bottom() - theme::CHART_AXIS_LABEL),
    );
    let line = Stroke::new(1.0, theme::CHART_GRID);
    for tick in y_scale.ticks() {
        let y = area.bottom() - y_scale.fraction(tick) * area.height();
        if chart.gridlines {
            painter.hline(area.x_range(), y, line);
        }
        painter.text(
            pos2(area.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            label(tick),
            font(),
            theme::CHART_TEXT,
        );
    }
    for tick in x_scale.ticks() {
        let x = area.left() + x_scale.fraction(tick) * area.width();
        if chart.gridlines {
            painter.vline(x, area.y_range(), line);
        }
        painter.text(
            pos2(x, area.bottom() + 3.0),
            Align2::CENTER_TOP,
            label(tick),
            font(),
            theme::CHART_TEXT,
        );
    }
    for (s, series) in plotted.iter().enumerate() {
        for (x, value) in xs.iter().zip(&series.values) {
            let Some(y) = value else {
                continue;
            };
            let center = pos2(
                area.left() + x_scale.fraction(*x) * area.width(),
                area.bottom() - y_scale.fraction(*y) * area.height(),
            );
            painter.circle_filled(center, theme::CHART_MARKER, color(s));
            if chart.labels {
                painter.text(
                    center - vec2(0.0, 6.0),
                    Align2::CENTER_BOTTOM,
                    label(*y),
                    font(),
                    theme::CHART_TEXT,
                );
            }
        }
    }
}

impl crate::app::Spreadsheet {
    pub fn insert_chart(&mut self, kind: ChartKind) {
        if let Some(index) = self.chart_index() {
            self.doc.set_layout(|layout| {
                if let Some(chart) = layout.charts.get_mut(index) {
                    chart.kind = kind;
                }
            });
            return;
        }
        let selected = self.selected();
        let data = if selected.is_single() {
            vellum_grid_core::current_region(self.sheet(), self.sel.active)
        } else {
            self.sheet().trim_to_extent(selected)
        };
        let col = (data.end().col() + 2).min(vellum_grid_core::MAX_COLS - 1);
        let anchor =
            vellum_grid_core::CellAddr::new(data.start().row(), col).expect("inside the sheet");
        let chart = Chart::new(kind, data, anchor);
        self.doc.set_layout(|layout| layout.charts.push(chart));
        self.object = Some(crate::objects::Object::Chart(
            self.sheet().layout().charts.len() - 1,
        ));
    }

    pub fn open_chart_title(&mut self, index: usize) {
        let Some(chart) = self.sheet().layout().charts.get(index) else {
            return;
        };
        let data = vellum_grid_core::chart_data(self.sheet(), chart.data, locale());
        let title = title(chart, &data);
        self.dialog = Some(crate::dialogs::Dialog::ChartTitle { index, title });
    }
}
