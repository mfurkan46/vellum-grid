use eframe::egui::{CornerRadius, Painter, Pos2, Rect, RichText, Stroke, TextEdit, Ui, pos2, vec2};
use vellum_grid_core::{CellAddr, CellRange, SparkKind, SparklineGroup, spark_pairs};

use crate::app::Spreadsheet;
use crate::dialogs::Dialog;
use crate::format_cells::color_button;
use crate::icons::Icon;
use crate::ribbon::widgets::{big, big_checked, chrome_text, group};
use crate::{fluent, render, text, theme};

#[derive(Debug, Clone)]
pub struct SparkForm {
    pub kind: SparkKind,
    pub data: String,
    pub location: String,
    pub error: bool,
}

fn absolute(range: CellRange) -> String {
    let corner = |addr: CellAddr| {
        format!(
            "${}${}",
            vellum_grid_core::column_name(addr.col()),
            addr.row() + 1
        )
    };
    if range.is_single() {
        corner(range.start())
    } else {
        format!("{}:{}", corner(range.start()), corner(range.end()))
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

pub fn form_ui(ui: &mut Ui, form: &mut SparkForm) {
    ui.set_width(theme::GO_TO_WIDTH);
    ui.label(text::spark_data_prompt());
    ui.horizontal(|ui| {
        ui.label(text::spark_data());
        ui.add(TextEdit::singleline(&mut form.data).desired_width(f32::INFINITY));
    });
    ui.label(text::spark_location_prompt());
    ui.horizontal(|ui| {
        ui.label(text::spark_location());
        ui.add(TextEdit::singleline(&mut form.location).desired_width(f32::INFINITY));
    });
    if form.error {
        ui.label(RichText::new(text::bad_spark_range()).color(theme::error_text()));
    }
}

pub fn paint(painter: &Painter, cell: Rect, group: &SparklineGroup, values: &[Option<f64>]) {
    let rect = cell.shrink2(vec2(theme::SPARK_PADDING, theme::SPARK_PADDING));
    let numbers: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|v| (i, v)))
        .collect();
    if numbers.is_empty() || rect.width() < 2.0 || rect.height() < 2.0 {
        return;
    }
    let count = values.len().max(1);
    let main = render::color(group.color);
    let accent = render::color(group.point_color);
    let high = numbers.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
    let low = numbers.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
    let first = numbers.first().map(|(i, _)| *i);
    let last = numbers.last().map(|(i, _)| *i);
    let special = |i: usize, v: f64| {
        (group.high && v == high)
            || (group.low && v == low)
            || (group.first && Some(i) == first)
            || (group.last && Some(i) == last)
            || (group.negative && v < 0.0)
    };
    match group.kind {
        SparkKind::Line => {
            let span = (high - low).max(f64::EPSILON);
            let x = |i: usize| {
                if count == 1 {
                    rect.center().x
                } else {
                    rect.left() + rect.width() * i as f32 / (count - 1) as f32
                }
            };
            let y = |v: f64| rect.bottom() - rect.height() * ((v - low) / span) as f32;
            let points: Vec<Pos2> = numbers.iter().map(|(i, v)| pos2(x(*i), y(*v))).collect();
            painter.line(points.clone(), Stroke::new(theme::SPARK_LINE, main));
            for ((i, v), point) in numbers.iter().zip(points) {
                if group.markers || special(*i, *v) {
                    let color = if special(*i, *v) { accent } else { main };
                    painter.circle_filled(point, theme::SPARK_MARKER, color);
                }
            }
        }
        SparkKind::Column | SparkKind::WinLoss => {
            let win_loss = group.kind == SparkKind::WinLoss;
            let (bottom, top) = if win_loss {
                (-1.0, 1.0)
            } else {
                (low.min(0.0), high.max(0.0))
            };
            let span = (top - bottom).max(f64::EPSILON);
            let y = |v: f64| rect.bottom() - rect.height() * ((v - bottom) / span) as f32;
            let slot = rect.width() / count as f32;
            let width = (slot * 0.75).max(1.0);
            for (i, v) in &numbers {
                let value = if win_loss { v.signum() } else { *v };
                if value == 0.0 && win_loss {
                    continue;
                }
                let left = rect.left() + slot * *i as f32 + (slot - width) / 2.0;
                let (y0, y1) = (y(0.0), y(value));
                let bar = Rect::from_min_max(
                    pos2(left, y0.min(y1)),
                    pos2(left + width, y0.max(y1).max(y0.min(y1) + 1.0)),
                );
                let color = if special(*i, *v) { accent } else { main };
                painter.rect_filled(bar, CornerRadius::ZERO, color);
            }
        }
    }
}

impl Spreadsheet {
    pub fn sparkline_at(&self, addr: CellAddr) -> Option<usize> {
        self.sheet()
            .layout()
            .sparklines
            .iter()
            .position(|group| group.at(addr).is_some())
    }

    pub fn open_sparklines(&mut self, kind: SparkKind) {
        let data = self.selected();
        let location = if data.rows() > 1 && data.cols() > 1 {
            let col = (data.end().col() + 1).min(vellum_grid_core::MAX_COLS - 1);
            CellRange::new(
                CellAddr::new(data.start().row(), col).expect("inside the sheet"),
                CellAddr::new(data.end().row(), col).expect("inside the sheet"),
            )
        } else {
            let col = (data.end().col() + 1).min(vellum_grid_core::MAX_COLS - 1);
            CellRange::single(CellAddr::new(data.start().row(), col).expect("inside the sheet"))
        };
        self.dialog = Some(Dialog::Sparkline(SparkForm {
            kind,
            data: absolute(data),
            location: absolute(location),
            error: false,
        }));
    }

    pub fn apply_sparklines(&mut self, mut form: SparkForm) {
        let pairs = parse_range(&form.data)
            .zip(parse_range(&form.location))
            .and_then(|(data, location)| spark_pairs(data, location));
        let Some(pairs) = pairs else {
            form.error = true;
            self.dialog = Some(Dialog::Sparkline(form));
            return;
        };
        let group = SparklineGroup::new(form.kind, pairs);
        self.doc.set_layout(|layout| {
            for existing in &mut layout.sparklines {
                existing.cells.retain(|(at, _)| group.at(*at).is_none());
            }
            layout.sparklines.retain(|g| !g.cells.is_empty());
            layout.sparklines.push(group);
        });
    }

    pub fn sparkline_buttons(&mut self, ui: &mut Ui) {
        group(ui, text::sparklines(), |ui| {
            ui.horizontal_top(|ui| {
                for (icon, label, kind) in [
                    (fluent::DATA_TRENDING, text::chart_line(), SparkKind::Line),
                    (
                        fluent::DATA_BAR_VERTICAL,
                        text::chart_column(),
                        SparkKind::Column,
                    ),
                    (fluent::DATA_HISTOGRAM, text::win_loss(), SparkKind::WinLoss),
                ] {
                    if big(ui, Icon::Plain(icon), label, text::sparklines(), false).clicked() {
                        self.open_sparklines(kind);
                    }
                }
            });
        });
    }

    fn update_sparkline(&mut self, index: usize, f: impl FnOnce(&mut SparklineGroup)) {
        self.doc.set_layout(|layout| {
            if let Some(group) = layout.sparklines.get_mut(index) {
                f(group);
            }
        });
    }

    pub fn sparkline_tab(&mut self, ui: &mut Ui) {
        let Some(index) = self.sparkline_at(self.sel.active) else {
            return;
        };
        let current = self.sheet().layout().sparklines[index].clone();
        group(ui, text::chart_type(), |ui| {
            ui.horizontal_top(|ui| {
                for (icon, label, kind) in [
                    (fluent::DATA_TRENDING, text::chart_line(), SparkKind::Line),
                    (
                        fluent::DATA_BAR_VERTICAL,
                        text::chart_column(),
                        SparkKind::Column,
                    ),
                    (fluent::DATA_HISTOGRAM, text::win_loss(), SparkKind::WinLoss),
                ] {
                    let on = current.kind == kind;
                    if big_checked(ui, Icon::Plain(icon), label, label, on).clicked() {
                        self.update_sparkline(index, |g| g.kind = kind);
                    }
                }
            });
        });
        group(ui, text::spark_show(), |ui| {
            let mut edited = current.clone();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.checkbox(&mut edited.high, chrome_text(text::high_point()));
                    ui.checkbox(&mut edited.low, chrome_text(text::low_point()));
                    ui.checkbox(&mut edited.negative, chrome_text(text::negative_points()));
                });
                ui.add_space(theme::RIBBON_COLUMN_GAP);
                ui.vertical(|ui| {
                    ui.checkbox(&mut edited.first, chrome_text(text::first_point()));
                    ui.checkbox(&mut edited.last, chrome_text(text::last_point()));
                    ui.add_enabled(
                        edited.kind == SparkKind::Line,
                        eframe::egui::Checkbox::new(
                            &mut edited.markers,
                            chrome_text(text::markers()),
                        ),
                    );
                });
            });
            if edited != current {
                self.update_sparkline(index, |g| *g = edited);
            }
        });
        group(ui, text::spark_style(), |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(chrome_text(text::spark_color()));
                    if let Some(choice) =
                        color_button(ui, "spark-color", Some(current.color), text::automatic())
                    {
                        let color = choice.unwrap_or(vellum_grid_core::Color(0x376092));
                        self.update_sparkline(index, |g| g.color = color);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(chrome_text(text::marker_color()));
                    if let Some(choice) = color_button(
                        ui,
                        "spark-point-color",
                        Some(current.point_color),
                        text::automatic(),
                    ) {
                        let color = choice.unwrap_or(vellum_grid_core::Color(0xD00000));
                        self.update_sparkline(index, |g| g.point_color = color);
                    }
                });
            });
        });
        group(ui, text::spark_group(), |ui| {
            let icon = Icon::Plain(fluent::DELETE);
            if big(
                ui,
                icon,
                text::clear_sparklines(),
                text::clear_sparklines(),
                false,
            )
            .clicked()
            {
                self.doc.set_layout(|layout| {
                    if index < layout.sparklines.len() {
                        layout.sparklines.remove(index);
                    }
                });
            }
        });
    }
}
