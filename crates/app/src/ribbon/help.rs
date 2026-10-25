use eframe::egui::Ui;

use super::widgets::{big, group};
use crate::app::Spreadsheet;
use crate::dialogs::Dialog;
use crate::icons::Icon;
use crate::{fluent, text, theme};

impl Spreadsheet {
    pub(super) fn help(&mut self, ui: &mut Ui) {
        group(ui, text::help(), |ui| {
            ui.horizontal_top(|ui| {
                let keys = Icon::Plain(fluent::KEYBOARD);
                if big(ui, keys, text::shortcuts(), text::shortcuts(), false).clicked() {
                    self.dialog = Some(Dialog::Shortcuts);
                }
                let about = Icon::Tinted(fluent::INFO, theme::tint_blue());
                if big(ui, about, text::about(), text::about(), false).clicked() {
                    self.dialog = Some(Dialog::About);
                }
            });
        });
    }
}
