use eframe::egui::{
    Align2, ComboBox, FontFamily, FontId, Popup, RichText, Ui, ViewportCommand, vec2,
};
use vellum_grid_core::{CellValue, HAlign, STACKED, Underline, VAlign, numfmt};

use super::widgets::{
    big, chrome_text, drop, group, launcher_group, points_text, small, small_menu, soon,
    style_sample, tip, tool,
};
use crate::app::{Spreadsheet, locale};
use crate::cells::ShiftChoice;
use crate::format::{self, CellStyle};
use crate::format_cells::FormatTab;
use crate::icons::Icon;
use crate::{fluent, fonts, menu, text, theme};

impl Spreadsheet {
    pub(super) fn home(&mut self, ui: &mut Ui) {
        self.clipboard_group(ui);
        self.font_group(ui);
        self.alignment_group(ui);
        self.number_group(ui);
        self.styles_group(ui);
        self.cells_group(ui);
        self.editing_group(ui);
    }

    fn clipboard_group(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        group(ui, text::clipboard(), |ui| {
            ui.horizontal_top(|ui| {
                let paste = big(
                    ui,
                    Icon::Paste,
                    text::paste(),
                    &tip(text::paste(), "Ctrl+V"),
                    true,
                );
                Popup::menu(&paste).width(theme::MENU_WIDTH).show(|ui| {
                    if menu::shortcut_item(ui, Some(Icon::Paste), text::paste(), "Ctrl+V") {
                        ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
                    }
                    self.paste_options(ui);
                    if menu::enabled_item(ui, self.can_paste_special(), None, text::paste_special())
                    {
                        self.open_paste_special();
                    }
                });
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if tool(ui, Icon::Cut, &tip(text::cut(), "Ctrl+X"), false).clicked() {
                        self.copy(&ctx, true);
                    }
                    if tool(ui, Icon::Copy, &tip(text::copy(), "Ctrl+C"), false).clicked() {
                        self.copy(&ctx, false);
                    }
                    let painting = self.format_painter.is_some();
                    if tool(ui, Icon::FormatPainter, text::format_painter(), painting).clicked() {
                        self.format_painter = (!painting).then(|| self.active_style());
                    }
                });
            });
        });
    }

    fn font_group(&mut self, ui: &mut Ui) {
        let style = self.active_style();
        let launch = launcher_group(ui, text::font(), text::format_cells_title(), |ui| {
            ui.add_space(theme::RIBBON_ROW_GAP);
            ui.horizontal(|ui| {
                let mut font = None;
                ComboBox::from_id_salt("font-name")
                    .width(theme::FONT_COMBO_WIDTH)
                    .selected_text(style.font.as_str())
                    .show_ui(ui, |ui| {
                        for name in fonts::available() {
                            let sample = RichText::new(name)
                                .font(FontId::new(14.0, FontFamily::Name(name.into())));
                            if ui.selectable_label(name == style.font, sample).clicked() {
                                font = Some(name);
                            }
                        }
                    })
                    .response
                    .on_hover_text(text::font());
                if let Some(name) = font {
                    self.restyle(|_, s| s.font = name.to_owned());
                }
                let mut size = None;
                ComboBox::from_id_salt("font-size")
                    .width(theme::SIZE_COMBO_WIDTH)
                    .selected_text(points_text(style.size))
                    .show_ui(ui, |ui| {
                        for points in theme::FONT_SIZES {
                            let tenths = points * 10;
                            if ui
                                .selectable_label(style.size == tenths, points.to_string())
                                .clicked()
                            {
                                size = Some(tenths);
                            }
                        }
                    })
                    .response
                    .on_hover_text(text::font_size());
                if tool(ui, Icon::GrowFont, text::grow_font(), false).clicked() {
                    size = Some(format::step_size(style.size, &theme::FONT_SIZES, true));
                }
                if tool(ui, Icon::ShrinkFont, text::shrink_font(), false).clicked() {
                    size = Some(format::step_size(style.size, &theme::FONT_SIZES, false));
                }
                if let Some(size) = size {
                    self.restyle(|_, s| s.size = size);
                }
            });
            ui.horizontal(|ui| {
                if tool(ui, Icon::Bold, &tip(text::bold(), "Ctrl+B"), style.bold).clicked() {
                    self.toggle_bold();
                }
                if tool(
                    ui,
                    Icon::Italic,
                    &tip(text::italic(), "Ctrl+I"),
                    style.italic,
                )
                .clicked()
                {
                    self.toggle_italic();
                }
                let underline = tip(text::underline(), "Ctrl+U");
                let underlined = style.underline != Underline::None;
                if tool(ui, Icon::Underline, &underline, underlined).clicked() {
                    self.toggle_underline();
                }
                let arrow = drop(ui, text::underline());
                Popup::menu(&arrow).width(theme::MENU_WIDTH).show(|ui| {
                    for (label, kind) in [
                        (text::underline(), Underline::Single),
                        (text::double_underline(), Underline::Double),
                    ] {
                        if menu::icon_item(ui, Icon::Underline, label) {
                            self.restyle(|_, s| s.underline = kind);
                        }
                    }
                });
                ui.add_space(theme::CHROME_SPACING);
                self.border_button(ui);
                self.color_button(ui, true);
                self.color_button(ui, false);
            });
        });
        if launch {
            self.open_format_tab(FormatTab::Font);
        }
    }

    fn alignment_group(&mut self, ui: &mut Ui) {
        let style = self.active_style();
        let launch = launcher_group(ui, text::alignment(), text::format_cells_title(), |ui| {
            ui.add_space(theme::RIBBON_ROW_GAP);
            ui.horizontal(|ui| {
                for (icon, label, align) in [
                    (Icon::AlignTop, text::align_top(), VAlign::Top),
                    (Icon::AlignMiddle, text::align_middle(), VAlign::Center),
                    (Icon::AlignBottom, text::align_bottom(), VAlign::Bottom),
                ] {
                    if tool(ui, icon, label, style.v_align == align).clicked() {
                        self.restyle(|_, s| s.v_align = align);
                    }
                }
                let icon = Icon::Plain(fluent::TEXT_DIRECTION);
                let turned = style.rotation != 0;
                let orientation = tool(ui, icon, text::orientation(), turned);
                Popup::menu(&orientation)
                    .width(theme::MENU_WIDTH)
                    .show(|ui| {
                        for (label, rotation) in [
                            (text::rotate_ccw(), 45),
                            (text::rotate_cw(), -45),
                            (text::rotate_vertical(), STACKED),
                            (text::rotate_up(), 90),
                            (text::rotate_down(), -90),
                        ] {
                            let was = style.rotation == rotation;
                            let mut on = was;
                            menu::check(ui, &mut on, label);
                            if on != was {
                                self.rotate_text(rotation);
                            }
                        }
                        menu::separator(ui);
                        if menu::icon_item(ui, Icon::CellFormat, text::format_alignment()) {
                            self.open_format_tab(FormatTab::Alignment);
                        }
                    });
                ui.add_space(theme::CHROME_SPACING);
                if small(
                    ui,
                    Icon::Wrap,
                    text::wrap_text(),
                    text::wrap_text(),
                    style.wrap,
                )
                .clicked()
                {
                    let wrap = !style.wrap;
                    self.restyle(|_, s| s.wrap = wrap);
                }
            });
            ui.horizontal(|ui| {
                for (icon, label, align) in [
                    (Icon::AlignLeft, text::align_left(), HAlign::Left),
                    (Icon::AlignCenter, text::align_center(), HAlign::Center),
                    (Icon::AlignRight, text::align_right(), HAlign::Right),
                ] {
                    let checked = style.h_align == align;
                    if tool(ui, icon, label, checked).clicked() {
                        let align = if checked { HAlign::General } else { align };
                        self.restyle(|_, s| s.h_align = align);
                    }
                }
                let decrease = Icon::Plain(fluent::TEXT_INDENT_DECREASE);
                let increase = Icon::Plain(fluent::TEXT_INDENT_INCREASE);
                if tool(ui, decrease, text::indent_decrease(), false).clicked() {
                    self.step_indent(-1);
                }
                if tool(ui, increase, text::indent_increase(), false).clicked() {
                    self.step_indent(1);
                }
                ui.add_space(theme::CHROME_SPACING);
                let merged = self.sheet().layout().merge_at(self.sel.active).is_some();
                let range = self.selected();
                let label = text::merge_center();
                if small(ui, Icon::Merge, label, label, merged).clicked() {
                    if merged {
                        self.doc.unmerge(range);
                    } else {
                        self.doc.merge(range, true);
                    }
                }
                let arrow = drop(ui, text::merge_center());
                Popup::menu(&arrow).width(theme::MENU_WIDTH).show(|ui| {
                    if menu::icon_item(ui, Icon::Merge, text::merge_center()) {
                        self.doc.merge(range, true);
                    }
                    let cells = Icon::Plain(fluent::TABLE_CELLS_MERGE);
                    if menu::icon_item(ui, cells, text::merge_cells()) {
                        self.doc.merge(range, false);
                    }
                    if menu::icon_item(ui, Icon::Plain(fluent::TABLE_SIMPLE), text::unmerge_cells())
                    {
                        self.doc.unmerge(range);
                    }
                });
            });
        });
        if launch {
            self.open_format_tab(FormatTab::Alignment);
        }
    }

    fn number_group(&mut self, ui: &mut Ui) {
        let style = self.active_style();
        let value = self.sheet().value(self.sel.active).clone();
        let mut code = None;
        let mut more = false;
        let launch = launcher_group(ui, text::number(), text::format_cells_title(), |ui| {
            ui.add_space(theme::RIBBON_ROW_GAP);
            ui.horizontal(|ui| {
                ComboBox::from_id_salt("number-format")
                    .width(theme::FORMAT_COMBO_WIDTH)
                    .selected_text(format::format_name(&style.number_format))
                    .show_ui(ui, |ui| {
                        for (name, quick) in format::QUICK_FORMATS {
                            let selected = style.number_format.eq_ignore_ascii_case(quick);
                            let response = ui.selectable_label(selected, name());
                            let sample = match &value {
                                CellValue::Empty => String::new(),
                                value => locale().render(value, quick).text,
                            };
                            ui.painter().text(
                                response.rect.right_center() - vec2(4.0, 0.0),
                                Align2::RIGHT_CENTER,
                                sample,
                                theme::small_font(),
                                theme::chrome_text_muted(),
                            );
                            if response.clicked() {
                                code = Some(quick.to_owned());
                            }
                        }
                        ui.separator();
                        if ui
                            .selectable_label(false, text::more_number_formats())
                            .clicked()
                        {
                            more = true;
                        }
                    })
                    .response
                    .on_hover_text(text::number_format());
            });
            ui.horizontal(|ui| {
                if tool(ui, Icon::Currency, text::accounting_format(), false).clicked() {
                    code = Some(numfmt::CURRENCY.to_owned());
                }
                if tool(ui, Icon::Percent, text::percent_style(), false).clicked() {
                    code = Some(numfmt::PERCENT.to_owned());
                }
                if tool(ui, Icon::Thousands, text::comma_style(), false).clicked() {
                    code = Some(numfmt::THOUSANDS.to_owned());
                }
                ui.add_space(theme::CHROME_SPACING);
                for (icon, label, delta) in [
                    (Icon::MoreDecimals, text::increase_decimal(), 1),
                    (Icon::FewerDecimals, text::decrease_decimal(), -1),
                ] {
                    if tool(ui, icon, label, false).clicked() {
                        let format = &style.number_format;
                        code = Some(format::step_decimals(format, &value, locale(), delta));
                    }
                }
            });
        });
        if let Some(code) = code {
            self.restyle(|_, s| s.number_format = code.clone());
        }
        if launch || more {
            self.open_format_cells();
        }
    }

    fn styles_group(&mut self, ui: &mut Ui) {
        group(ui, text::styles(), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let conditional = small_menu(ui, Icon::Conditional, text::conditional());
            Popup::menu(&conditional)
                .width(theme::MENU_WIDTH)
                .show(|ui| self.conditional_menu(ui));
            let tables = small_menu(ui, Icon::Plain(fluent::TABLE), text::format_as_table());
            Popup::menu(&tables).show(|ui| {
                let current = self
                    .active_table()
                    .map(|index| self.sheet().layout().tables[index].style.clone());
                if let Some(style) = crate::tables_ui::style_gallery(ui, current.as_deref()) {
                    self.choose_table_style(style);
                    ui.close();
                }
            });
            let styles = small_menu(ui, Icon::CellStyles, text::cell_styles());
            Popup::menu(&styles).show(|ui| {
                for (heading, kinds) in CellStyle::groups() {
                    ui.label(chrome_text(heading));
                    for row in kinds.chunks(theme::STYLE_GALLERY_COLUMNS) {
                        ui.horizontal(|ui| {
                            for kind in row {
                                if style_sample(ui, *kind).clicked() {
                                    self.restyle(|_, s| kind.apply(s));
                                    ui.close();
                                }
                            }
                        });
                    }
                }
            });
        });
    }

    fn cells_group(&mut self, ui: &mut Ui) {
        group(ui, text::cells(), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let ctx = ui.ctx().clone();
            let insert = small_menu(
                ui,
                Icon::Plain(fluent::TABLE_INSERT_ROW),
                text::insert_cells(),
            );
            Popup::menu(&insert).width(theme::MENU_WIDTH).show(|ui| {
                let cells = Some(Icon::Plain(fluent::TABLE_INSERT_ROW));
                if menu::shortcut_item(ui, cells, text::insert_cells_menu(), "Ctrl++") {
                    self.insert_cells();
                }
                menu::separator(ui);
                if menu::icon_item(
                    ui,
                    Icon::Plain(fluent::TABLE_INSERT_ROW),
                    text::insert_rows_menu(),
                ) {
                    self.shift_selection(ShiftChoice::Rows, true);
                }
                if menu::icon_item(
                    ui,
                    Icon::Plain(fluent::TABLE_INSERT_COLUMN),
                    text::insert_cols_menu(),
                ) {
                    self.shift_selection(ShiftChoice::Cols, true);
                }
                menu::separator(ui);
                if menu::shortcut_item(
                    ui,
                    Some(Icon::Plain(fluent::TAB_ADD)),
                    text::insert_sheet_menu(),
                    "Shift+F11",
                ) {
                    self.insert_sheet(&ctx);
                }
            });
            let delete = small_menu(
                ui,
                Icon::Plain(fluent::TABLE_DELETE_ROW),
                text::delete_cells(),
            );
            Popup::menu(&delete).width(theme::MENU_WIDTH).show(|ui| {
                let cells = Some(Icon::Plain(fluent::TABLE_DELETE_ROW));
                if menu::shortcut_item(ui, cells, text::delete_cells_menu(), "Ctrl+-") {
                    self.delete_cells();
                }
                menu::separator(ui);
                if menu::icon_item(
                    ui,
                    Icon::Plain(fluent::TABLE_DELETE_ROW),
                    text::delete_rows_menu(),
                ) {
                    self.shift_selection(ShiftChoice::Rows, false);
                }
                if menu::icon_item(
                    ui,
                    Icon::Plain(fluent::TABLE_DELETE_COLUMN),
                    text::delete_cols_menu(),
                ) {
                    self.shift_selection(ShiftChoice::Cols, false);
                }
                menu::separator(ui);
                if menu::icon_item(ui, Icon::Plain(fluent::DELETE), text::delete_sheet_menu()) {
                    self.delete_sheet(self.doc.active());
                }
            });
            let format = small_menu(ui, Icon::CellFormat, text::format());
            Popup::menu(&format).width(theme::MENU_WIDTH).show(|ui| {
                let row = Icon::Plain(fluent::TABLE_RESIZE_ROW);
                if menu::icon_item(ui, row, text::row_height()) {
                    self.open_size(false);
                }
                if menu::item(ui, text::autofit_row()) {
                    let rows = self.selected_rows();
                    self.autofit_rows(ui, rows);
                }
                menu::separator(ui);
                let column = Icon::Plain(fluent::TABLE_RESIZE_COLUMN);
                if menu::icon_item(ui, column, text::column_width()) {
                    self.open_size(true);
                }
                if menu::item(ui, text::autofit_column()) {
                    let cols = self.selected_cols();
                    self.autofit_columns(ui, cols, self.digit);
                }
                menu::separator(ui);
                if menu::icon_item(ui, Icon::Plain(fluent::RENAME), text::rename_sheet()) {
                    self.start_rename(self.doc.active());
                }
                menu::separator(ui);
                let lock = Icon::Plain(fluent::LOCK_CLOSED);
                let label = if self.doc.is_protected() {
                    text::unprotect_sheet_menu()
                } else {
                    text::protect_sheet_menu()
                };
                if menu::icon_item(ui, lock, label) {
                    self.toggle_sheet_protection();
                }
                let mut locked = self.active_style().locked;
                menu::check(ui, &mut locked, text::lock_cell());
                if locked != self.active_style().locked {
                    self.toggle_lock();
                }
                if menu::shortcut_item(ui, Some(Icon::CellFormat), text::format_cells(), "Ctrl+1") {
                    self.open_format_cells();
                }
            });
        });
    }

    fn editing_group(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        group(ui, text::editing(), |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let hint = tip(text::autosum(), "Alt+=");
                        if small(ui, Icon::AutoSum, text::autosum(), &hint, false).clicked() {
                            self.auto_sum(&ctx, "SUM");
                        }
                        self.autosum_menu(ui);
                    });
                    soon(ui, |ui| {
                        small_menu(ui, Icon::Plain(fluent::ARROW_IMPORT), text::fill());
                    });
                    let clear = small_menu(ui, Icon::Clear, text::clear_menu());
                    Popup::menu(&clear).width(theme::MENU_WIDTH).show(|ui| {
                        let range = self.selected();
                        if menu::icon_item(ui, Icon::Clear, text::clear_all()) {
                            self.doc.clear_all(range);
                        }
                        if menu::item(ui, text::clear_formats()) {
                            self.doc.clear_formats(range);
                        }
                        if menu::item(ui, text::clear()) {
                            self.clear_selection();
                        }
                    });
                });
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let sort = small_menu(ui, Icon::Sort, text::sort_filter_short());
                    Popup::menu(&sort)
                        .width(theme::MENU_WIDTH)
                        .show(|ui| self.sort_filter_menu(ui));
                    let find = small_menu(ui, Icon::Find, text::find_select());
                    Popup::menu(&find).width(theme::MENU_WIDTH).show(|ui| {
                        if menu::shortcut_item(ui, Some(Icon::Find), text::find_menu(), "Ctrl+F") {
                            self.open_find(false);
                        }
                        let replace = Some(Icon::Plain(fluent::ARROW_SYNC));
                        if menu::shortcut_item(ui, replace, text::replace_menu(), "Ctrl+H") {
                            self.open_find(true);
                        }
                        let go = Some(Icon::Plain(fluent::ARROW_FORWARD));
                        if menu::shortcut_item(ui, go, text::go_to_menu(), "Ctrl+G") {
                            self.open_go_to();
                        }
                        let special = Some(Icon::Plain(fluent::TARGET_ARROW));
                        if menu::shortcut_item(ui, special, text::go_to_special_menu(), "") {
                            self.open_go_to_special();
                        }
                    });
                });
            });
        });
    }
}
