use eframe::egui::{Popup, Ui};

use super::widgets::{big, big_checked, group, small, tip, tool};
use crate::app::{Pending, Spreadsheet};
use crate::icons::Icon;
use crate::{fluent, menu, text, theme};

impl Spreadsheet {
    pub(super) fn data_tab(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        group(ui, text::get_data(), |ui| {
            let icon = Icon::Tinted(fluent::DOCUMENT_TABLE, theme::tint_green());
            if big(ui, icon, text::from_text(), text::from_text_tip(), false).clicked() {
                if cfg!(target_arch = "wasm32") {
                    self.request(&ctx, Pending::Open);
                } else {
                    self.open_text_import();
                }
            }
        });
        group(ui, text::sort_filter(), |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.add_space(theme::RIBBON_ROW_GAP);
                    if tool(ui, Icon::SortAsc, text::sort_asc(), false).clicked() {
                        self.quick_sort(false);
                    }
                    if tool(ui, Icon::SortDesc, text::sort_desc(), false).clicked() {
                        self.quick_sort(true);
                    }
                });
                if big(ui, Icon::Sort, text::sort(), text::custom_sort(), false).clicked() {
                    self.open_sort_dialog();
                }
                let filtering = self.sheet().layout().filter.is_some();
                let hint = tip(text::filter(), "Ctrl+Shift+L");
                if big_checked(ui, Icon::Filter, text::filter(), &hint, filtering).clicked() {
                    self.toggle_filter();
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let label = text::clear_filter();
                    if small(ui, Icon::ClearFilter, label, label, false).clicked() {
                        self.clear_any_filter();
                    }
                    if small(ui, Icon::Reapply, text::reapply(), text::reapply(), false).clicked() {
                        self.doc.reapply_filter();
                    }
                    let advanced = Icon::Plain(fluent::FILTER);
                    let label = text::advanced_filter();
                    if small(ui, advanced, label, label, false).clicked() {
                        self.open_advanced_filter();
                    }
                });
            });
        });
        group(ui, text::data_tools(), |ui| {
            ui.horizontal_top(|ui| {
                let split = Icon::Plain(fluent::TEXT_COLUMN_TWO);
                let label = text::text_to_columns();
                if big(ui, split, label, label, false).clicked() {
                    self.open_text_to_columns();
                }
                let flash = Icon::Tinted(fluent::FLASH, theme::tint_yellow());
                let hint = tip(text::flash_fill(), "Ctrl+E");
                if big(ui, flash, text::flash_fill(), &hint, false).clicked() {
                    self.flash_fill();
                }
                let duplicates = Icon::Plain(fluent::TABLE_DELETE_ROW);
                let label = text::remove_duplicates();
                if big(ui, duplicates, label, label, false).clicked() {
                    self.open_remove_duplicates();
                }
                let validation = Icon::Plain(fluent::TABLE_SETTINGS);
                let label = text::data_validation();
                let button = big(ui, validation, label, label, true);
                Popup::menu(&button).show(|ui| {
                    if menu::item(ui, text::validation_menu()) {
                        self.open_validation();
                    }
                    if menu::item(ui, text::circle_invalid()) {
                        self.circles = true;
                        self.circle_key = None;
                    }
                    if menu::item(ui, text::clear_circles()) {
                        self.circles = false;
                    }
                });
            });
        });
        group(ui, text::outline(), |ui| {
            ui.horizontal_top(|ui| {
                let hint = tip(text::group(), "Shift+Alt+→");
                let button = big(ui, Icon::Plain(fluent::GROUP), text::group(), &hint, true);
                Popup::menu(&button).show(|ui| {
                    if menu::item(ui, text::group_menu()) {
                        self.group_selection(true, None);
                    }
                });
                let hint = tip(text::ungroup(), "Shift+Alt+←");
                let button = big(
                    ui,
                    Icon::Plain(fluent::TABLE_SIMPLE),
                    text::ungroup(),
                    &hint,
                    true,
                );
                Popup::menu(&button).show(|ui| {
                    if menu::item(ui, text::ungroup_menu()) {
                        self.group_selection(false, None);
                    }
                    if menu::item(ui, text::clear_outline()) {
                        self.doc.clear_outline();
                    }
                });
                let label = text::subtotal();
                if big(ui, Icon::Plain(fluent::AUTOSUM), label, label, false).clicked() {
                    self.open_subtotal();
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let label = text::show_detail();
                    if small(ui, Icon::Plain(fluent::ADD), label, label, false).clicked() {
                        self.detail(true);
                    }
                    let label = text::hide_detail();
                    if small(ui, Icon::Plain(fluent::SUBTRACT), label, label, false).clicked() {
                        self.detail(false);
                    }
                });
            });
        });
    }
}
