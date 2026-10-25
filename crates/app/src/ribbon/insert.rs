use eframe::egui::Ui;

use super::widgets::{big, group, soon, tip};
use crate::app::Spreadsheet;
use crate::icons::Icon;
use crate::{fluent, text};

impl Spreadsheet {
    pub(super) fn insert_tab(&mut self, ui: &mut Ui) {
        group(ui, text::tables(), |ui| {
            ui.horizontal_top(|ui| {
                soon(ui, |ui| {
                    big(
                        ui,
                        Icon::Plain(fluent::TABLE_SIMPLE),
                        text::pivot_table(),
                        text::pivot_table(),
                        true,
                    );
                });
                let icon = Icon::Tinted(fluent::TABLE, crate::theme::tint_blue());
                let hint = tip(text::table(), "Ctrl+T");
                if big(ui, icon, text::table(), &hint, false).clicked() {
                    self.open_create_table(None);
                }
            });
        });
        group(ui, text::illustrations(), |ui| {
            ui.horizontal_top(|ui| {
                let icon = Icon::Plain(fluent::IMAGE);
                if big(ui, icon, text::pictures(), text::pictures(), false).clicked() {
                    let ctx = ui.ctx().clone();
                    self.pick_picture(&ctx);
                }
                self.shape_button(ui);
            });
        });
        self.chart_groups(ui);
        self.sparkline_buttons(ui);
        group(ui, text::links(), |ui| {
            let hint = tip(text::link(), "Ctrl+K");
            if big(ui, Icon::Plain(fluent::LINK), text::link(), &hint, false).clicked() {
                self.open_link_dialog();
            }
        });
        group(ui, text::comments(), |ui| {
            let icon = Icon::Plain(fluent::COMMENT_ADD);
            if big(ui, icon, text::comment(), text::comment(), false).clicked() {
                self.open_comment(self.sel.active);
            }
        });
        group(ui, text::text_group(), |ui| {
            ui.horizontal_top(|ui| {
                let icon = Icon::Plain(fluent::TEXTBOX);
                if big(ui, icon, text::text_box(), text::text_box(), false).clicked() {
                    self.start_shape_tool(vellum_grid_core::drawing::ShapeKind::TextBox);
                }
                soon(ui, |ui| {
                    big(
                        ui,
                        Icon::Plain(fluent::DOCUMENT_HEADER_FOOTER),
                        text::header_footer(),
                        text::header_footer(),
                        false,
                    );
                });
            });
        });
        group(ui, text::symbols(), |ui| {
            soon(ui, |ui| {
                big(
                    ui,
                    Icon::Plain(fluent::SYMBOLS),
                    text::symbol_insert(),
                    text::symbol_insert(),
                    false,
                );
            });
        });
    }
}
