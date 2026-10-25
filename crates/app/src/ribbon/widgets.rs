use eframe::egui::{Align2, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui, vec2};
use vellum_grid_core::Color;

pub use vellum_ui::widgets::*;

use crate::app::locale;
use crate::format::CellStyle;
use crate::{render, theme};

pub fn palette_ui(ui: &mut Ui, none_label: &str) -> Option<Option<Color>> {
    vellum_ui::widgets::palette_ui(ui, none_label).map(|choice| choice.map(Color))
}

pub fn style_sample(ui: &mut Ui, kind: CellStyle) -> Response {
    let size = vec2(theme::STYLE_SAMPLE_WIDTH, theme::SMALL_TOOL_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let sample = kind.sample();
    let fill = sample.fill.map_or(theme::cell_bg(), render::cell_color);
    let color = sample.color.map_or(theme::cell_text(), render::cell_color);
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    render::paint_borders(painter, rect.shrink(2.0), &sample.borders);
    let line = if response.hovered() {
        theme::accent()
    } else {
        theme::ribbon_line()
    };
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, line),
        StrokeKind::Inside,
    );
    painter.text(
        rect.left_center() + vec2(theme::CELL_PADDING + 2.0, 0.0),
        Align2::LEFT_CENTER,
        kind.label(),
        FontId::new(
            theme::CHROME_FONT_SIZE * (f32::from(sample.size) / 110.0).clamp(1.0, 1.3),
            crate::fonts::cell_font(&sample, 1.0).id.family,
        ),
        color,
    );
    response
}

pub fn points_text(size: u16) -> String {
    locale().format_number(f64::from(size) / 10.0)
}
