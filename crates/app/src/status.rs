use eframe::egui::{
    Align, Button, CornerRadius, Label, Layout, Rect, Sense, Stroke, Ui, pos2, vec2,
};
use vellum_grid_core::CellValue;

use crate::app::{EditMode, Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::geometry::Axis;
use crate::icons::{self, Icon};
use crate::ribbon::chrome_text;
use crate::{fluent, menu, text, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub average: bool,
    pub count: bool,
    pub numeric: bool,
    pub min: bool,
    pub max: bool,
    pub sum: bool,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            average: true,
            count: true,
            numeric: false,
            min: false,
            max: false,
            sum: true,
        }
    }
}

struct Summary {
    count: usize,
    numbers: usize,
    sum: f64,
    min: f64,
    max: f64,
}

pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 4.0;

pub fn clamp_zoom(zoom: f32) -> f32 {
    ((zoom * 100.0).round() / 100.0).clamp(MIN_ZOOM, MAX_ZOOM)
}

fn slider_position(zoom: f32) -> f32 {
    if zoom <= 1.0 {
        (zoom - MIN_ZOOM) / (1.0 - MIN_ZOOM) * 0.5
    } else {
        0.5 + (zoom - 1.0) / (MAX_ZOOM - 1.0) * 0.5
    }
}

fn slider_zoom(position: f32) -> f32 {
    let position = position.clamp(0.0, 1.0);
    if position <= 0.5 {
        MIN_ZOOM + position / 0.5 * (1.0 - MIN_ZOOM)
    } else {
        1.0 + (position - 0.5) / 0.5 * (MAX_ZOOM - 1.0)
    }
}

impl Spreadsheet {
    pub fn status_bar(&mut self, ui: &mut Ui) {
        let bar = ui.max_rect();
        let response = ui.interact(bar, ui.scope_id().with("status-bar"), Sense::click());
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0 * theme::CHROME_SPACING;
            let mode = match self.edit.as_ref() {
                None => text::ready(),
                Some(edit) if edit.point.is_some() => text::point(),
                Some(edit) if edit.mode == EditMode::Enter => text::enter(),
                Some(_) => text::edit(),
            };
            ui.label(chrome_text(mode));
            if self.doc.needs_calculation() {
                let calculate =
                    ui.add(Label::new(chrome_text(text::calculate_status())).sense(Sense::click()));
                if calculate.on_hover_text(text::calculate_now()).clicked() {
                    self.calculate();
                }
            }
            if let Some((sheet, addr)) = self.doc.circular() {
                let place = if sheet == self.doc.active() {
                    addr.to_string()
                } else {
                    format!("{}!{addr}", self.doc.book().sheets()[sheet].name())
                };
                ui.label(chrome_text(&format!("{}: {place}", text::circular())));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                self.zoom_controls(ui);
                ui.add_space(theme::CHROME_SPACING);
                self.view_buttons(ui);
                ui.add_space(2.0 * theme::CHROME_SPACING);
                ui.spacing_mut().item_spacing.x = 2.0 * theme::CHROME_SPACING;
                self.statistics(ui);
            });
        });
        response.context_menu(|ui| {
            ui.set_min_width(theme::CONTEXT_MENU_WIDTH);
            let stats = &mut self.stats;
            menu::check(ui, &mut stats.average, text::average());
            menu::check(ui, &mut stats.count, text::count());
            menu::check(ui, &mut stats.numeric, text::numeric_count());
            menu::check(ui, &mut stats.min, text::minimum());
            menu::check(ui, &mut stats.max, text::maximum());
            menu::check(ui, &mut stats.sum, text::sum());
        });
    }

    fn statistics(&self, ui: &mut Ui) {
        let Some(summary) = self.summary() else {
            return;
        };
        let number = |n: f64| {
            locale()
                .format_number_fit(n, theme::STATUS_DIGITS)
                .unwrap_or_else(|| locale().format_number(n))
        };
        let stats = self.stats;
        let mut items: Vec<String> = Vec::new();
        if summary.numbers > 0 && stats.average {
            let average = number(summary.sum / summary.numbers as f64);
            items.push(format!("{}: {average}", text::average()));
        }
        if stats.count {
            items.push(format!("{}: {}", text::count(), summary.count));
        }
        if stats.numeric {
            items.push(format!("{}: {}", text::numeric_count(), summary.numbers));
        }
        if summary.numbers > 0 {
            if stats.min {
                items.push(format!("{}: {}", text::minimum(), number(summary.min)));
            }
            if stats.max {
                items.push(format!("{}: {}", text::maximum(), number(summary.max)));
            }
            if stats.sum {
                items.push(format!("{}: {}", text::sum(), number(summary.sum)));
            }
        }
        for item in items.iter().rev() {
            ui.label(chrome_text(item));
        }
    }

    fn summary(&self) -> Option<Summary> {
        let ranges = self.ranges();
        if ranges.len() == 1 && ranges[0].is_single() {
            return None;
        }
        let mut summary = Summary {
            count: 0,
            numbers: 0,
            sum: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        };
        for (_, cell) in ranges
            .iter()
            .flat_map(|&range| self.sheet().cells_in(range))
        {
            if !cell.has_content() {
                continue;
            }
            summary.count += 1;
            if let CellValue::Number(n) = cell.value {
                summary.numbers += 1;
                summary.sum += n;
                summary.min = summary.min.min(n);
                summary.max = summary.max.max(n);
            }
        }
        (summary.count > 0).then_some(summary)
    }

    fn view_buttons(&mut self, ui: &mut Ui) {
        let breaks = self.page_break_view;
        for (glyph, label, enabled, checked, view) in [
            (
                fluent::DOCUMENT_PAGE_BREAK,
                text::page_break_preview(),
                true,
                breaks,
                Some(true),
            ),
            (
                fluent::DOCUMENT_ONE_PAGE,
                text::page_layout_view(),
                false,
                false,
                None,
            ),
            (
                fluent::TABLE,
                text::normal_view(),
                true,
                !breaks,
                Some(false),
            ),
        ] {
            let size = vec2(theme::STATUS_BUTTON, theme::STATUS_BUTTON);
            let sense = if enabled {
                Sense::click()
            } else {
                Sense::hover()
            };
            let (rect, response) = ui.allocate_exact_size(size, sense);
            let painter = ui.painter();
            if checked {
                painter.rect_filled(rect, CornerRadius::same(3), theme::tool_checked());
            } else if enabled && response.hovered() {
                painter.rect_filled(rect, CornerRadius::same(3), theme::tab_hover());
            }
            let area = Rect::from_center_size(
                rect.center(),
                vec2(theme::STATUS_GLYPH, theme::STATUS_GLYPH),
            );
            if enabled {
                icons::paint(painter, area, Icon::Plain(glyph), theme::icon(), None);
            } else {
                icons::paint_muted(painter, area, Icon::Plain(glyph), theme::icon_disabled());
            }
            let tip = if enabled {
                label.to_owned()
            } else {
                format!("{label}\n{}", text::coming_soon())
            };
            if response.on_hover_text(tip).clicked()
                && let Some(view) = view
            {
                self.page_break_view = view;
            }
        }
    }

    fn zoom_controls(&mut self, ui: &mut Ui) {
        let label = text::percent_of((self.zoom * 100.0).round());
        if ui
            .add(Button::new(chrome_text(&label)).frame(false))
            .on_hover_text(text::zoom_level())
            .clicked()
        {
            self.open_zoom_dialog();
        }
        if step_button(ui, fluent::ADD, text::zoom_in()).clicked() {
            self.set_zoom(self.zoom + theme::ZOOM_STEP);
        }
        let size = vec2(theme::ZOOM_SLIDER, theme::STATUS_BUTTON);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter();
        let y = rect.center().y;
        painter.hline(
            rect.x_range(),
            y,
            Stroke::new(1.0, theme::chrome_text_muted()),
        );
        painter.vline(
            rect.center().x,
            y - 4.0..=y + 4.0,
            Stroke::new(1.0, theme::chrome_text_muted()),
        );
        let x = rect.left() + slider_position(self.zoom) * rect.width();
        let thumb = Rect::from_center_size(pos2(x, y), vec2(4.0, 12.0));
        painter.rect_filled(thumb, CornerRadius::same(1), theme::chrome_text());
        if (response.dragged() || response.clicked())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let mut zoom = slider_zoom((pointer.x - rect.left()) / rect.width());
            if (zoom - 1.0).abs() < 0.04 {
                zoom = 1.0;
            }
            self.set_zoom(zoom);
        }
        if step_button(ui, fluent::SUBTRACT, text::zoom_out()).clicked() {
            self.set_zoom(self.zoom - theme::ZOOM_STEP);
        }
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = clamp_zoom(zoom);
    }

    pub fn open_zoom_dialog(&mut self) {
        self.dialog = Some(Dialog::Zoom {
            percent: (self.zoom * 100.0).round() as u32,
        });
    }

    pub fn zoom_to_selection(&mut self) {
        let layout = self.sheet().layout();
        let cols = Axis::columns(layout, self.digit);
        let rows = Axis::rows(layout, 1.0);
        let range = self.selected();
        let width = cols.offset(range.start().col(), range.end().col() + 1);
        let height = rows.offset(range.start().row(), range.end().row() + 1);
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let fit = (self.grid_area.x / width).min(self.grid_area.y / height);
        self.set_zoom(fit);
        self.view.top = range.start().row().max(self.view.min_top);
        self.view.left = range.start().col().max(self.view.min_left);
    }
}

fn step_button(ui: &mut Ui, glyph: fluent::Glyph, tip: &str) -> eframe::egui::Response {
    let size = vec2(theme::STATUS_BUTTON, theme::STATUS_BUTTON);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(3), theme::tab_hover());
    }
    let area = Rect::from_center_size(
        rect.center(),
        vec2(theme::STATUS_GLYPH, theme::STATUS_GLYPH),
    );
    icons::paint(ui.painter(), area, Icon::Plain(glyph), theme::icon(), None);
    response.on_hover_text(tip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_maps_zoom_both_ways() {
        assert_eq!(slider_position(1.0), 0.5);
        assert_eq!(slider_zoom(0.5), 1.0);
        assert!((slider_zoom(slider_position(0.55)) - 0.55).abs() < 1e-5);
        assert!((slider_zoom(slider_position(2.5)) - 2.5).abs() < 1e-5);
        assert_eq!(slider_zoom(0.0), MIN_ZOOM);
        assert_eq!(slider_zoom(1.0), MAX_ZOOM);
        assert_eq!(clamp_zoom(7.0), MAX_ZOOM);
        assert_eq!(clamp_zoom(0.754), 0.75);
    }
}
