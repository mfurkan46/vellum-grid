use eframe::egui::{Button, CornerRadius, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

use crate::icons::{self, Icon};
use crate::widgets::chrome_text;
use crate::{fluent, text, theme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomInput {
    Set(f32),
    /// The percentage was clicked; the app opens its zoom dialog.
    Dialog,
}

/// Zoom percentage, zoom out, slider and zoom in, laid out right to left. 100% sits in the
/// middle of the slider; each half covers `min..1` and `1..max`.
pub fn zoom_controls(ui: &mut Ui, zoom: f32, min: f32, max: f32) -> Option<ZoomInput> {
    let mut input = None;
    let label = text::percent_of((zoom * 100.0).round());
    if ui
        .add(Button::new(chrome_text(&label)).frame(false))
        .on_hover_text(text::zoom_level())
        .clicked()
    {
        input = Some(ZoomInput::Dialog);
    }
    if step_button(ui, fluent::ADD, text::zoom_in()).clicked() {
        input = Some(ZoomInput::Set(zoom + theme::ZOOM_STEP));
    }
    let size = vec2(theme::ZOOM_SLIDER, theme::STATUS_BUTTON);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter();
    let y = rect.center().y;
    let line = Stroke::new(1.0, theme::chrome_text_muted());
    painter.hline(rect.x_range(), y, line);
    painter.vline(rect.center().x, y - 4.0..=y + 4.0, line);
    let x = rect.left() + slider_position(zoom, min, max) * rect.width();
    let thumb = Rect::from_center_size(pos2(x, y), vec2(4.0, 12.0));
    painter.rect_filled(thumb, CornerRadius::same(1), theme::chrome_text());
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let mut value = slider_zoom((pointer.x - rect.left()) / rect.width(), min, max);
        if (value - 1.0).abs() < 0.04 {
            value = 1.0;
        }
        input = Some(ZoomInput::Set(value));
    }
    if step_button(ui, fluent::SUBTRACT, text::zoom_out()).clicked() {
        input = Some(ZoomInput::Set(zoom - theme::ZOOM_STEP));
    }
    input
}

pub fn step_button(ui: &mut Ui, glyph: fluent::Glyph, tip: &str) -> Response {
    view_button(ui, glyph, tip, true, false)
}

/// A small square status bar button, shaded when `checked`.
pub fn view_button(
    ui: &mut Ui,
    glyph: fluent::Glyph,
    label: &str,
    enabled: bool,
    checked: bool,
) -> Response {
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
        response.on_hover_text(label)
    } else {
        icons::paint_muted(painter, area, Icon::Plain(glyph), theme::icon_disabled());
        response.on_hover_text(format!("{label}\n{}", text::coming_soon()))
    }
}

pub fn slider_position(zoom: f32, min: f32, max: f32) -> f32 {
    if zoom <= 1.0 {
        ((zoom - min) / (1.0 - min) * 0.5).max(0.0)
    } else {
        (0.5 + (zoom - 1.0) / (max - 1.0) * 0.5).min(1.0)
    }
}

pub fn slider_zoom(position: f32, min: f32, max: f32) -> f32 {
    let position = position.clamp(0.0, 1.0);
    if position <= 0.5 {
        min + position / 0.5 * (1.0 - min)
    } else {
        1.0 + (position - 0.5) / 0.5 * (max - 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_maps_zoom_both_ways() {
        let (min, max) = (0.1, 4.0);
        assert_eq!(slider_position(1.0, min, max), 0.5);
        assert_eq!(slider_zoom(0.5, min, max), 1.0);
        assert!((slider_zoom(slider_position(0.55, min, max), min, max) - 0.55).abs() < 1e-5);
        assert!((slider_zoom(slider_position(2.5, min, max), min, max) - 2.5).abs() < 1e-5);
        assert_eq!(slider_zoom(0.0, min, max), min);
        assert_eq!(slider_zoom(1.0, min, max), max);
        assert_eq!(slider_position(9.0, min, max), 1.0);
        assert_eq!(slider_position(0.0, min, max), 0.0);
    }
}
