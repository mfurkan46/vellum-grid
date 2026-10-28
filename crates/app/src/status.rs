use eframe::egui::{Align, Label, Layout, Sense, Ui};
use vellum_grid_core::CellValue;
use vellum_ui::status::{self, ZoomInput};

use crate::app::{EditMode, Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::geometry::Axis;
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
            if status::view_button(ui, glyph, label, enabled, checked).clicked()
                && let Some(view) = view
            {
                self.page_break_view = view;
            }
        }
    }

    fn zoom_controls(&mut self, ui: &mut Ui) {
        match status::zoom_controls(ui, self.zoom, MIN_ZOOM, MAX_ZOOM) {
            Some(ZoomInput::Set(zoom)) => self.set_zoom(zoom),
            Some(ZoomInput::Dialog) => self.open_zoom_dialog(),
            None => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_is_rounded_and_clamped() {
        assert_eq!(clamp_zoom(7.0), MAX_ZOOM);
        assert_eq!(clamp_zoom(0.754), 0.75);
    }
}
