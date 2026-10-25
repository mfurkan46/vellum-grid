use eframe::egui::{self, CornerRadius, Sense, Stroke, Ui, pos2, vec2};

use crate::app::Spreadsheet;
use crate::icons::Icon;
use crate::{fluent, logo, text, theme};
pub use vellum_ui::window::{maximized_inset, resize_edges};
use vellum_ui::window::{quick, title_bar};

impl Spreadsheet {
    pub fn title_bar(&mut self, ui: &mut Ui) {
        let logo = logo::texture(ui.ctx());
        let book = self.book.clone();
        title_bar(ui, logo.as_ref(), &book, |ui| {
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
