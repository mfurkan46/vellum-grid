use eframe::egui::viewport::ResizeDirection;
use eframe::egui::{
    Color32, Context, CornerRadius, CursorIcon, Rect, Response, Sense, Ui, ViewportCommand, vec2,
};

use crate::icons::{self, Icon};
use crate::{fluent, theme};

pub fn quick(ui: &mut Ui, icon: Icon, tip: &str, enabled: bool) -> Response {
    let size = vec2(theme::TOOL_SIZE, theme::TOOL_SIZE);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if enabled && response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), theme::tab_hover());
    }
    let area = Rect::from_center_size(rect.center(), vec2(theme::ICON_SIZE, theme::ICON_SIZE));
    if enabled {
        icons::paint(ui.painter(), area, icon, theme::icon(), None);
    } else {
        icons::paint_muted(ui.painter(), area, icon, theme::icon_disabled());
    }
    response.on_hover_text(tip)
}

pub fn caption(ui: &mut Ui, glyph: fluent::Glyph, tip: &str, close: bool) -> Response {
    let size = vec2(theme::CAPTION_WIDTH, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let color = match (close, hovered) {
        (true, true) => {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, theme::CLOSE_HOVER);
            Color32::WHITE
        }
        (false, true) => {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, theme::tab_hover());
            theme::chrome_text()
        }
        _ => theme::chrome_text(),
    };
    let area = Rect::from_center_size(
        rect.center(),
        vec2(theme::CAPTION_GLYPH, theme::CAPTION_GLYPH),
    );
    icons::paint(ui.painter(), area, Icon::Plain(glyph), color, None);
    response.on_hover_text(tip)
}

pub fn resize_edges(ctx: &Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let screen = ctx.content_rect();
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let edge = theme::RESIZE_EDGE;
    let left = pos.x - screen.left() <= edge;
    let right = screen.right() - pos.x <= edge;
    let top = pos.y - screen.top() <= edge;
    let bottom = screen.bottom() - pos.y <= edge;
    let (direction, cursor) = match (left, right, top, bottom) {
        (true, _, true, _) => (ResizeDirection::NorthWest, CursorIcon::ResizeNwSe),
        (_, true, true, _) => (ResizeDirection::NorthEast, CursorIcon::ResizeNeSw),
        (true, _, _, true) => (ResizeDirection::SouthWest, CursorIcon::ResizeNeSw),
        (_, true, _, true) => (ResizeDirection::SouthEast, CursorIcon::ResizeNwSe),
        (true, ..) => (ResizeDirection::West, CursorIcon::ResizeHorizontal),
        (_, true, ..) => (ResizeDirection::East, CursorIcon::ResizeHorizontal),
        (_, _, true, _) => (ResizeDirection::North, CursorIcon::ResizeVertical),
        (_, _, _, true) => (ResizeDirection::South, CursorIcon::ResizeVertical),
        _ => return,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

pub fn maximized_inset(ctx: &Context) -> f32 {
    ctx.input(|i| {
        let viewport = i.viewport();
        match (viewport.maximized, viewport.inner_rect) {
            (Some(true), Some(inner)) => (-inner.min.x).max(0.0),
            _ => 0.0,
        }
    })
}
