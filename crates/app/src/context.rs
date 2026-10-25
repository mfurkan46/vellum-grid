use eframe::egui::{Popup, Ui, ViewportCommand};
use vellum_grid_core::{CellRange, numfmt};

use crate::app::{Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::format;
use crate::icons::Icon;
use crate::paste::{PasteOptions, PasteWhat};
use crate::ribbon::chrome_text;
use crate::ribbon::widgets::paste_option;
use crate::ribbon::widgets::{drop, palette_ui, tool, tool_with_bar};
use crate::{fluent, menu, render, text, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTarget {
    Cells,
    Rows,
    Cols,
    Object(crate::objects::Object),
}

impl Spreadsheet {
    pub fn context_menu(&mut self, ui: &mut Ui, target: ContextTarget) {
        ui.set_min_width(theme::CONTEXT_MENU_WIDTH);
        match target {
            ContextTarget::Object(crate::objects::Object::Chart(index)) => {
                self.chart_menu(ui, index);
                return;
            }
            ContextTarget::Object(object) => {
                self.object_menu(ui, object);
                return;
            }
            _ => {}
        }
        self.mini_toolbar(ui);
        menu::separator(ui);
        let ctx = ui.ctx().clone();
        if menu::shortcut_item(ui, Some(Icon::Cut), text::cut(), "Ctrl+X") {
            self.copy(&ctx, true);
        }
        if menu::shortcut_item(ui, Some(Icon::Copy), text::copy(), "Ctrl+C") {
            self.copy(&ctx, false);
        }
        if menu::shortcut_item(ui, Some(Icon::Paste), text::paste(), "Ctrl+V") {
            ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
        }
        self.paste_options(ui);
        if menu::enabled_item(ui, self.can_paste_special(), None, text::paste_special()) {
            self.open_paste_special();
        }
        menu::separator(ui);
        let (insert, delete) = match target {
            ContextTarget::Rows | ContextTarget::Cols => {
                (text::insert_plain(), text::delete_plain())
            }
            _ => (text::insert_menu(), text::delete_menu()),
        };
        let (insert_icon, delete_icon) = if target == ContextTarget::Cols {
            (fluent::TABLE_INSERT_COLUMN, fluent::TABLE_DELETE_COLUMN)
        } else {
            (fluent::TABLE_INSERT_ROW, fluent::TABLE_DELETE_ROW)
        };
        if menu::icon_item(ui, Icon::Plain(insert_icon), insert) {
            self.insert_cells();
        }
        if menu::icon_item(ui, Icon::Plain(delete_icon), delete) {
            self.delete_cells();
        }
        if menu::icon_item(ui, Icon::Clear, text::clear()) {
            self.clear_selection();
        }
        menu::separator(ui);
        if target == ContextTarget::Cells {
            menu::sub(ui, Some(Icon::Filter), text::filter(), |ui| {
                if menu::icon_item(ui, Icon::Filter, text::filter_toggle()) {
                    self.toggle_filter();
                }
                if menu::icon_item(ui, Icon::ClearFilter, text::clear_filter()) {
                    self.doc.clear_filters();
                }
            });
            menu::sub(ui, Some(Icon::Sort), text::sort(), |ui| {
                if menu::icon_item(ui, Icon::SortAsc, text::sort_asc()) {
                    self.quick_sort(false);
                }
                if menu::icon_item(ui, Icon::SortDesc, text::sort_desc()) {
                    self.quick_sort(true);
                }
                if menu::icon_item(ui, Icon::Sort, text::custom_sort()) {
                    self.open_sort_dialog();
                }
            });
            menu::separator(ui);
            let addr = self.sel.active;
            let has_comment = self.sheet().layout().comments.contains_key(&addr);
            let note = self.sheet().layout().notes.get(&addr).cloned();
            let comment_label = if has_comment {
                text::reply_comment()
            } else {
                text::new_comment()
            };
            if menu::icon_item(ui, Icon::Plain(fluent::COMMENT_ADD), comment_label) {
                self.open_comment(addr);
            }
            if has_comment
                && menu::icon_item(ui, Icon::Plain(fluent::DISMISS), text::delete_comment())
            {
                self.doc.set_layout(|layout| {
                    layout.comments.remove(&addr);
                });
            }
            let note_label = if note.is_some() {
                text::edit_note()
            } else {
                text::new_note()
            };
            if menu::shortcut_item(
                ui,
                Some(Icon::Plain(fluent::NOTE_ADD)),
                note_label,
                "Shift+F2",
            ) {
                self.edit_note();
            }
            if let Some(note) = note {
                if menu::icon_item(ui, Icon::Plain(fluent::DISMISS), text::delete_note()) {
                    self.delete_note();
                }
                let label = if note.visible {
                    text::hide_note()
                } else {
                    text::show_note()
                };
                if menu::item(ui, label) {
                    self.toggle_note();
                }
            }
            menu::separator(ui);
        }
        if menu::shortcut_item(ui, Some(Icon::CellFormat), text::format_cells(), "Ctrl+1") {
            self.open_format_cells();
        }
        match target {
            ContextTarget::Rows => {
                let icon = Icon::Plain(fluent::TABLE_RESIZE_ROW);
                if menu::icon_item(ui, icon, text::row_height()) {
                    self.open_size(false);
                }
                self.hide_menu(ui, false);
            }
            ContextTarget::Cols => {
                let icon = Icon::Plain(fluent::TABLE_RESIZE_COLUMN);
                if menu::icon_item(ui, icon, text::column_width()) {
                    self.open_size(true);
                }
                self.hide_menu(ui, true);
            }
            _ => {
                if menu::icon_item(ui, Icon::Plain(fluent::TAG), text::define_name_menu()) {
                    self.open_define_name(false);
                }
                let addr = self.sel.active;
                let linked = self.sheet().layout().links.contains_key(&addr);
                let label = if linked {
                    text::edit_link_menu()
                } else {
                    text::link_menu()
                };
                if menu::shortcut_item(ui, Some(Icon::Plain(fluent::LINK)), label, "Ctrl+K") {
                    self.open_link_dialog();
                }
                if linked {
                    if menu::item(ui, text::open_link()) {
                        self.follow_link(addr);
                    }
                    if menu::item(ui, text::remove_link()) {
                        self.remove_link(addr);
                    }
                }
            }
        }
    }

    fn mini_toolbar(&mut self, ui: &mut Ui) {
        let style = self.active_style();
        let value = self.sheet().value(self.sel.active).clone();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if tool(ui, Icon::Bold, text::bold(), style.bold).clicked() {
                self.toggle_bold();
            }
            if tool(ui, Icon::Italic, text::italic(), style.italic).clicked() {
                self.toggle_italic();
            }
            let fill = self.fill_color.map_or(theme::surface(), render::color);
            if tool_with_bar(ui, Icon::Fill, text::fill_color(), fill).clicked() {
                let color = self.fill_color;
                self.restyle(|_, s| s.fill = color);
            }
            let arrow = drop(ui, text::fill_color());
            Popup::menu(&arrow).show(|ui| {
                if let Some(choice) = palette_ui(ui, text::no_fill()) {
                    self.fill_color = choice;
                    self.restyle(|_, s| s.fill = choice);
                }
            });
            let font = self.font_color.map_or(theme::chrome_text(), render::color);
            if tool_with_bar(ui, Icon::FontColor, text::font_color(), font).clicked() {
                let color = self.font_color;
                self.restyle(|_, s| s.color = color);
            }
            if tool(ui, Icon::Borders, self.border.label(), false).clicked() {
                let kind = self.border;
                let range = self.selected();
                self.restyle(|addr, s| kind.apply(range, addr, &mut s.borders));
            }
            if tool(ui, Icon::Percent, text::percent_style(), false).clicked() {
                self.restyle(|_, s| s.number_format = numfmt::PERCENT.to_owned());
            }
            if tool(ui, Icon::Thousands, text::comma_style(), false).clicked() {
                self.restyle(|_, s| s.number_format = numfmt::THOUSANDS.to_owned());
            }
            for (icon, label, delta) in [
                (Icon::MoreDecimals, text::increase_decimal(), 1),
                (Icon::FewerDecimals, text::decrease_decimal(), -1),
            ] {
                if tool(ui, icon, label, false).clicked() {
                    let code = format::step_decimals(&style.number_format, &value, locale(), delta);
                    self.restyle(|_, s| s.number_format = code.clone());
                }
            }
            let painting = self.format_painter.is_some();
            if tool(ui, Icon::FormatPainter, text::format_painter(), painting).clicked() {
                self.format_painter = (!painting).then(|| self.active_style());
                ui.close();
            }
        });
    }

    pub fn paste_options(&mut self, ui: &mut Ui) {
        if !self.can_paste_special() {
            return;
        }
        ui.add_space(theme::MENU_SECTION_GAP);
        ui.horizontal(|ui| {
            ui.add_space(theme::MENU_ICON_COLUMN);
            ui.label(chrome_text(text::paste_options()));
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(theme::MENU_ICON_COLUMN);
            for (icon, tip, options) in [
                (
                    Icon::Paste,
                    text::paste_plain_tip(),
                    PasteOptions::only(PasteWhat::All),
                ),
                (
                    Icon::Tinted(fluent::CLIPBOARD_NUMBER, theme::tint_clipboard()),
                    text::paste_values_tip(),
                    PasteOptions::only(PasteWhat::Values),
                ),
                (
                    Icon::Tinted(fluent::CLIPBOARD_FORMULA, theme::tint_clipboard()),
                    text::paste_formulas_tip(),
                    PasteOptions::only(PasteWhat::Formulas),
                ),
                (
                    Icon::Tinted(fluent::TABLE_SWITCH, theme::tint_blue()),
                    text::paste_transpose_tip(),
                    PasteOptions {
                        transpose: true,
                        ..PasteOptions::default()
                    },
                ),
                (
                    Icon::Tinted(fluent::CLIPBOARD_BRUSH, theme::tint_clipboard()),
                    text::paste_formats_tip(),
                    PasteOptions::only(PasteWhat::Formats),
                ),
            ] {
                if paste_option(ui, icon, tip).clicked() {
                    self.paste_special(options);
                    ui.close();
                }
            }
        });
        ui.add_space(theme::MENU_SECTION_GAP);
    }

    pub fn open_paste_special(&mut self) {
        if self.can_paste_special() {
            self.dialog = Some(Dialog::PasteSpecial(PasteOptions::default()));
        }
    }

    fn hide_menu(&mut self, ui: &mut Ui, columns: bool) {
        let range = self.selected();
        if menu::icon_item(ui, Icon::Plain(fluent::EYE_OFF), text::hide()) {
            self.set_hidden(range, columns, true);
        }
        if menu::icon_item(ui, Icon::Plain(fluent::EYE), text::show_hidden()) {
            self.set_hidden(range, columns, false);
        }
    }

    pub fn set_hidden(&mut self, range: CellRange, columns: bool, hide: bool) {
        let (start, end) = if columns {
            (range.start().col(), range.end().col())
        } else {
            (range.start().row(), range.end().row())
        };
        self.doc.set_layout(|layout| {
            let set = if columns {
                &mut layout.hidden_cols
            } else {
                &mut layout.hidden_rows
            };
            if hide {
                set.extend(start..=end);
            } else {
                set.retain(|i| !(start..=end).contains(i));
            }
        });
    }

    fn chart_menu(&mut self, ui: &mut Ui, index: usize) {
        if menu::icon_item(ui, Icon::Plain(fluent::DELETE), text::delete_chart()) {
            self.object = Some(crate::objects::Object::Chart(index));
            self.delete_object();
        }
        self.object = Some(crate::objects::Object::Chart(index));
        menu::sub(
            ui,
            Some(Icon::ChartColumn),
            text::change_chart_type(),
            |ui| {
                for (icon, label, _, types) in crate::chart_ui::families() {
                    menu::sub(ui, Some(icon), label, |ui| self.chart_type_menu(ui, types));
                }
            },
        );
        if menu::icon_item(ui, Icon::Plain(fluent::RENAME), text::chart_title_menu()) {
            self.open_chart_title(index);
        }
    }
}
