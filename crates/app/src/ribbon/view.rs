use eframe::egui::{Popup, Ui};

use super::widgets::{big, big_checked, chrome_text, group, soon};
use crate::app::Spreadsheet;
use crate::icons::Icon;
use crate::{fluent, menu, text, theme};

impl Spreadsheet {
    pub(super) fn view_tab(&mut self, ui: &mut Ui) {
        group(ui, text::workbook_views(), |ui| {
            ui.horizontal_top(|ui| {
                let breaks = self.page_break_view;
                if big_checked(
                    ui,
                    Icon::Plain(fluent::TABLE),
                    text::normal_view(),
                    text::normal_view(),
                    !breaks,
                )
                .clicked()
                {
                    self.page_break_view = false;
                }
                if big_checked(
                    ui,
                    Icon::Plain(fluent::DOCUMENT_PAGE_BREAK),
                    text::page_break_preview(),
                    text::page_break_preview(),
                    breaks,
                )
                .clicked()
                {
                    self.page_break_view = true;
                }
                soon(ui, |ui| {
                    big(
                        ui,
                        Icon::Plain(fluent::DOCUMENT_ONE_PAGE),
                        text::page_layout_view(),
                        text::page_layout_view(),
                        false,
                    );
                });
            });
        });
        group(ui, text::show(), |ui| {
            ui.add_space(theme::RIBBON_ROW_GAP);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    soon(ui, |ui| {
                        ui.checkbox(&mut false, chrome_text(text::ruler()));
                    });
                    self.gridlines_checkbox(ui, text::gridlines());
                });
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.show_formula_bar, chrome_text(text::formula_bar()));
                    self.headings_checkbox(ui, text::headings());
                });
            });
        });
        group(ui, text::zoom(), |ui| {
            ui.horizontal_top(|ui| {
                let zoom = Icon::Plain(fluent::ZOOM_IN);
                if big(ui, zoom, text::zoom(), text::zoom(), false).clicked() {
                    self.open_zoom_dialog();
                }
                let reset = Icon::Plain(fluent::SEARCH);
                if big(ui, reset, text::zoom_100(), text::zoom_100(), false).clicked() {
                    self.set_zoom(1.0);
                }
                let fit = Icon::Plain(fluent::ZOOM_FIT);
                let label = text::zoom_selection();
                if big(ui, fit, label, label, false).clicked() {
                    self.zoom_to_selection();
                }
            });
        });
        group(ui, text::window(), |ui| {
            ui.horizontal_top(|ui| {
                soon(ui, |ui| {
                    big(
                        ui,
                        Icon::Plain(fluent::WINDOW_NEW),
                        text::new_window(),
                        text::new_window(),
                        false,
                    );
                });
                let freeze = big(
                    ui,
                    Icon::Freeze,
                    text::freeze_panes(),
                    text::freeze_panes(),
                    true,
                );
                Popup::menu(&freeze).width(theme::MENU_WIDTH).show(|ui| {
                    let layout = self.sheet().layout();
                    let frozen = layout.freeze_rows > 0 || layout.freeze_cols > 0;
                    if frozen {
                        if menu::icon_item(ui, Icon::Freeze, text::unfreeze_panes()) {
                            self.freeze(0, 0);
                        }
                    } else if menu::icon_item(ui, Icon::Freeze, text::freeze_panes()) {
                        let active = self.sel.active;
                        self.freeze(active.row(), active.col());
                    }
                    let row = Icon::Plain(fluent::TABLE_FREEZE_ROW);
                    if menu::icon_item(ui, row, text::freeze_top_row()) {
                        self.freeze(1, 0);
                    }
                    let column = Icon::Plain(fluent::TABLE_FREEZE_COLUMN);
                    if menu::icon_item(ui, column, text::freeze_first_column()) {
                        self.freeze(0, 1);
                    }
                });
                soon(ui, |ui| {
                    big(
                        ui,
                        Icon::Plain(fluent::SPLIT_VERTICAL),
                        text::split(),
                        text::split(),
                        false,
                    );
                });
            });
        });
    }
}
