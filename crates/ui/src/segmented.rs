use eframe::egui::{Align2, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::{motion, theme};

/// Width of a segmented control with these `(label, tip)` items.
pub fn width(ui: &Ui, items: &[(&str, &str)]) -> f32 {
    items
        .iter()
        .map(|(label, _)| segment_width(ui, label))
        .sum::<f32>()
        + 2.0 * theme::SEGMENT_INSET
}

fn segment_width(ui: &Ui, label: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        theme::tab_font_bold(),
        theme::chrome_text(),
    );
    galley.size().x + 2.0 * theme::SEGMENT_PADDING
}

/// A row of mutually exclusive `(label, tip)` choices drawn as one pill; the selected one is
/// filled with the accent color. Returns the index clicked, if any.
pub fn segmented(ui: &mut Ui, items: &[(&str, &str)], selected: usize) -> Option<usize> {
    let size = vec2(width(ui, items), theme::SEGMENT_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let radius = CornerRadius::same(theme::SEGMENT_RADIUS);
    ui.painter().rect(
        rect,
        radius,
        theme::surface(),
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
    let mut clicked = None;
    let mut x = rect.left() + theme::SEGMENT_INSET;
    for (index, (label, tip)) in items.iter().enumerate() {
        let width = segment_width(ui, label);
        let segment = Rect::from_min_max(
            pos2(x, rect.top() + theme::SEGMENT_INSET),
            pos2(x + width, rect.bottom() - theme::SEGMENT_INSET),
        );
        x += width;
        let response = ui.interact(
            segment,
            ui.scope_id().with(("segment", index)),
            Sense::click(),
        );
        let on = index == selected;
        let inner = CornerRadius::same(theme::SEGMENT_RADIUS - 2);
        let (font, color) = if on {
            ui.painter()
                .rect_filled(segment, inner, theme::accent_fill());
            (theme::tab_font_bold(), theme::ON_ACCENT)
        } else {
            let hover = motion::hover_color(ui, &response, response.hovered(), theme::tab_hover());
            ui.painter().rect_filled(segment, inner, hover);
            (theme::tab_font(), theme::chrome_text())
        };
        ui.painter()
            .text(segment.center(), Align2::CENTER_CENTER, *label, font, color);
        if response.on_hover_text(*tip).clicked() && !on {
            clicked = Some(index);
        }
    }
    clicked
}
