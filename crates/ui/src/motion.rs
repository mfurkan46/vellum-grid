use eframe::egui::emath::{TSTransform, easing};
use eframe::egui::{Color32, Context, CornerRadius, Id, Rect, Response, Ui, Vec2, pos2, vec2};

use crate::theme;

pub fn hover_color(ui: &Ui, response: &Response, hot: bool, color: Color32) -> Color32 {
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), hot, theme::HOVER_ANIMATION);
    color.gamma_multiply(t)
}

pub fn reveal(ctx: &Context, id: Id, shown: bool, time: f32) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, shown, time, easing::cubic_out)
}

pub fn revealed<R>(ui: &mut Ui, t: f32, offset: Vec2, add: impl FnOnce(&mut Ui) -> R) -> R {
    let transform = TSTransform::from_translation(offset * (1.0 - t));
    ui.with_visual_transform(transform, |ui| {
        ui.multiply_opacity(t);
        add(ui)
    })
    .inner
}

pub fn tab_marker(ui: &Ui, id: Id, tab: Rect, color: Color32) {
    let ctx = ui.ctx();
    let x = ctx.animate_value_with_time(id.with("x"), tab.center().x, theme::TAB_MARKER_ANIMATION);
    let width = ctx.animate_value_with_time(
        id.with("width"),
        tab.width() - 2.0 * theme::RIBBON_TAB_PADDING,
        theme::TAB_MARKER_ANIMATION,
    );
    let marker = Rect::from_center_size(
        pos2(x, tab.bottom() - theme::RIBBON_TAB_MARKER),
        vec2(width, theme::RIBBON_TAB_MARKER),
    );
    ui.painter()
        .rect_filled(marker, CornerRadius::same(2), color);
}
