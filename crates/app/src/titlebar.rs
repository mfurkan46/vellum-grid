use eframe::egui::{
    self, Align, Align2, CornerRadius, Layout, PointerButton, Sense, Stroke, Ui, ViewportCommand,
    pos2, vec2,
};

use crate::app::Spreadsheet;
use crate::icons::Icon;
use crate::{fluent, logo, text, theme};
use vellum_ui::window::{caption, quick};
pub use vellum_ui::window::{maximized_inset, resize_edges};

impl Spreadsheet {
    pub fn title_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let rect = ui.max_rect();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let drag = ui.interact(
            rect,
            ui.scope_id().with("title-drag"),
            Sense::click_and_drag(),
        );
        if drag.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        } else if drag.drag_started_by(PointerButton::Primary) {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            &self.book,
            theme::title_font(),
            theme::chrome_text(),
        );
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(theme::CHROME_SPACING);
            if let Some(logo) = logo::texture(&ctx) {
                ui.image((logo.id(), vec2(theme::TITLE_LOGO, theme::TITLE_LOGO)));
            }
            ui.add_space(theme::CHROME_SPACING);
            self.autosave_switch(ui);
            ui.add_space(theme::CHROME_SPACING);
            if quick(ui, Icon::Save, &format!("{} (Ctrl+S)", text::save()), true).clicked() {
                self.save(false);
            }
            let undo = format!("{} (Ctrl+Z)", text::undo());
            if quick(ui, Icon::Undo, &undo, self.doc.can_undo()).clicked() {
                self.undo();
            }
            let redo = format!("{} (Ctrl+Y)", text::redo());
            if quick(ui, Icon::Redo, &redo, self.doc.can_redo()).clicked() {
                self.redo();
            }
            let print = Icon::Plain(fluent::DOCUMENT_PRINT);
            if quick(
                ui,
                print,
                &format!("{} (Ctrl+P)", text::print_preview()),
                true,
            )
            .clicked()
            {
                self.backstage = Some(crate::backstage::Page::Print);
            }
            if cfg!(target_arch = "wasm32") {
                return;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if caption(ui, fluent::DISMISS, text::close(), true).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                let (glyph, label) = if maximized {
                    (fluent::SQUARE_MULTIPLE, text::restore())
                } else {
                    (fluent::SQUARE, text::maximize())
                };
                if caption(ui, glyph, label, false).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                }
                if caption(ui, fluent::SUBTRACT, text::minimize(), false).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                }
            });
        });
    }

    fn autosave_switch(&mut self, ui: &mut Ui) {
        ui.label(
            egui::RichText::new(text::autosave())
                .font(theme::small_font())
                .color(theme::chrome_text()),
        );
        ui.add_space(theme::SWITCH_GAP);
        let size = vec2(theme::SWITCH_WIDTH, theme::SWITCH_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let on = self.autosave;
        let t = ui
            .ctx()
            .animate_bool_with_time(response.id, on, theme::SWITCH_ANIMATION);
        let radius = CornerRadius::same((theme::SWITCH_HEIGHT / 2.0) as u8);
        let fill = theme::surface().lerp_to_gamma(theme::accent_fill(), t);
        let line = theme::chrome_text_muted().lerp_to_gamma(theme::accent_fill(), t);
        let knob = theme::chrome_text_muted().lerp_to_gamma(theme::ON_ACCENT, t);
        let painter = ui.painter();
        painter.rect(
            rect,
            radius,
            fill,
            Stroke::new(1.0, line),
            egui::StrokeKind::Inside,
        );
        let half = rect.height() / 2.0;
        let x = egui::lerp(rect.left() + half..=rect.right() - half, t);
        let grow = if response.hovered() {
            theme::SWITCH_KNOB_GROW
        } else {
            0.0
        };
        painter.circle_filled(pos2(x, rect.center().y), half - 4.0 + grow, knob);
        let state = if on { text::on() } else { text::off() };
        let response = response.on_hover_text(format!(
            "{} — {state}\n{}",
            text::autosave(),
            text::autosave_tip()
        ));
        if response.clicked() {
            self.toggle_autosave();
        }
    }
}
