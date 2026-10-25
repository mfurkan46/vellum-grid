use eframe::egui::{self, Color32, Popup, Ui};
use vellum_grid_core::{CellAddr, CellRange, CellValue, Color, HAlign, MAX_ROWS, Style, Underline};

use super::widgets::{chrome_text, drop, palette_ui, tool, tool_with_bar};
use crate::app::{EditMode, Selection, Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::format::{self, BorderKind, RuleKind};
use crate::icons::Icon;
use crate::insert_function::FunctionPicker;
use crate::{clock, fluent, menu, render, text, theme};

pub const AUTOSUM: [(text::Label, &str); 5] = [
    (text::autosum_sum, "SUM"),
    (text::autosum_average, "AVERAGE"),
    (text::autosum_count, "COUNT"),
    (text::autosum_max, "MAX"),
    (text::autosum_min, "MIN"),
];

impl Spreadsheet {
    pub fn open_rule_dialog(&mut self, kind: RuleKind) {
        let first = match (kind.initial(), kind.inputs()) {
            (Some(initial), _) => initial.to_owned(),
            (None, 0) => String::new(),
            (None, _) => self.doc.input_text(self.sel.active),
        };
        self.dialog = Some(Dialog::Rule {
            kind,
            range: self.selected(),
            first,
            second: String::new(),
            preset: 0,
            period: 1,
        });
    }

    pub(super) fn sort_filter_menu(&mut self, ui: &mut Ui) {
        if menu::icon_item(ui, Icon::SortAsc, text::sort_asc()) {
            self.quick_sort(false);
        }
        if menu::icon_item(ui, Icon::SortDesc, text::sort_desc()) {
            self.quick_sort(true);
        }
        if menu::icon_item(ui, Icon::Sort, text::custom_sort()) {
            self.open_sort_dialog();
        }
        menu::separator(ui);
        if menu::shortcut_item(ui, Some(Icon::Filter), text::filter(), "Ctrl+Shift+L") {
            self.toggle_filter();
        }
        if menu::icon_item(ui, Icon::ClearFilter, text::clear_filter()) {
            self.doc.clear_filters();
        }
        if menu::icon_item(ui, Icon::Reapply, text::reapply()) {
            self.doc.reapply_filter();
        }
    }

    pub(super) fn gridlines_checkbox(&mut self, ui: &mut Ui, label: &str) {
        let mut shown = !self.sheet().layout().hide_gridlines;
        if ui.checkbox(&mut shown, chrome_text(label)).changed() {
            self.doc.set_layout(|layout| layout.hide_gridlines = !shown);
        }
    }

    pub(super) fn headings_checkbox(&mut self, ui: &mut Ui, label: &str) {
        let mut shown = !self.sheet().layout().hide_headings;
        if ui.checkbox(&mut shown, chrome_text(label)).changed() {
            self.doc.set_layout(|layout| layout.hide_headings = !shown);
        }
    }

    pub(super) fn border_button(&mut self, ui: &mut Ui) {
        let label = self.border.label();
        if tool(ui, Icon::Borders, label, false).clicked() {
            self.apply_border(self.border);
        }
        let arrow = drop(ui, text::borders());
        Popup::menu(&arrow).width(theme::MENU_WIDTH).show(|ui| {
            for (index, kind) in BorderKind::MENU.into_iter().enumerate() {
                if matches!(index, 4 | 8) {
                    menu::separator(ui);
                }
                if menu::icon_item(ui, Icon::Plain(border_glyph(kind)), kind.label()) {
                    self.border = kind;
                    self.apply_border(kind);
                }
            }
            menu::separator(ui);
            if menu::icon_item(ui, Icon::Borders, text::more_borders()) {
                self.open_format_tab(crate::format_cells::FormatTab::Border);
            }
        });
    }

    pub(super) fn color_button(&mut self, ui: &mut Ui, fill: bool) {
        let (icon, label, current) = if fill {
            (Icon::Fill, text::fill_color(), self.fill_color)
        } else {
            (Icon::FontColor, text::font_color(), self.font_color)
        };
        let button = tool_with_bar(
            ui,
            icon,
            label,
            current.map_or(Color32::TRANSPARENT, render::color),
        );
        if button.clicked() {
            self.apply_color(fill, current);
        }
        let arrow = drop(ui, label);
        Popup::menu(&arrow).show(|ui| {
            let none = if fill {
                text::no_fill()
            } else {
                text::automatic()
            };
            if let Some(choice) = palette_ui(ui, none) {
                if fill {
                    self.fill_color = choice;
                } else {
                    self.font_color = choice;
                }
                self.apply_color(fill, choice);
            }
        });
    }

    pub(super) fn autosum_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let arrow = drop(ui, text::autosum());
        Popup::menu(&arrow).width(theme::MENU_WIDTH).show(|ui| {
            for (label, function) in AUTOSUM {
                if menu::item(ui, label()) {
                    self.auto_sum(&ctx, function);
                }
            }
        });
    }

    pub fn active_style(&self) -> Style {
        self.sheet()
            .get(self.sel.active)
            .map(|cell| cell.style().clone())
            .unwrap_or_default()
    }

    pub fn restyle(&mut self, f: impl Fn(CellAddr, &mut Style)) {
        let ranges = self.ranges();
        self.doc.style_ranges(&ranges, f);
    }

    pub fn toggle_bold(&mut self) {
        let on = !self.active_style().bold;
        self.restyle(|_, s| s.bold = on);
    }

    pub fn toggle_italic(&mut self) {
        let on = !self.active_style().italic;
        self.restyle(|_, s| s.italic = on);
    }

    pub fn toggle_underline(&mut self) {
        let kind = if self.active_style().underline == Underline::None {
            Underline::Single
        } else {
            Underline::None
        };
        self.restyle(|_, s| s.underline = kind);
    }

    pub fn rotate_text(&mut self, rotation: i16) {
        let rotation = if self.active_style().rotation == rotation {
            0
        } else {
            rotation
        };
        self.restyle(|_, s| s.rotation = rotation);
        self.queue_autofit(self.selected());
    }

    pub fn step_indent(&mut self, delta: i8) {
        self.restyle(|_, s| {
            s.indent = s.indent.saturating_add_signed(delta).min(theme::MAX_INDENT);
            if delta > 0 && !s.h_align.indents() {
                s.h_align = HAlign::Left;
            }
        });
    }

    pub fn queue_autofit(&mut self, range: CellRange) {
        if range.rows() > theme::AUTOFIT_ROW_LIMIT {
            return;
        }
        let heights = &self.sheet().layout().row_heights;
        let rows: Vec<u32> = (range.start().row()..=range.end().row())
            .filter(|row| !heights.contains_key(row))
            .collect();
        self.autofit_queue.extend(rows);
    }

    pub fn toggle_strike(&mut self) {
        let on = !self.active_style().strike;
        self.restyle(|_, s| s.strike = on);
    }

    pub(super) fn apply_border(&mut self, kind: BorderKind) {
        let range = self.selected();
        self.restyle(|addr, s| kind.apply(range, addr, &mut s.borders));
    }

    pub(super) fn apply_color(&mut self, fill: bool, color: Option<Color>) {
        if fill {
            self.restyle(|_, s| s.fill = color);
        } else {
            self.restyle(|_, s| s.color = color);
        }
    }

    pub fn selected_cols(&self) -> Vec<u32> {
        let range = self.selected();
        (range.start().col()..=range.end().col()).collect()
    }

    pub fn selected_rows(&self) -> Vec<u32> {
        let range = self.selected();
        let range = if range.is_full_cols() {
            self.sheet().trim_to_extent(range)
        } else {
            range
        };
        (range.start().row()..=range.end().row()).collect()
    }

    pub fn open_size(&mut self, columns: bool) {
        let layout = self.sheet().layout();
        let active = self.sel.active;
        let value = if columns {
            format::chars_from_width(layout.col_width(active.col()), self.digit)
        } else {
            layout.row_height(active.row())
        };
        self.dialog = Some(Dialog::Size {
            columns,
            value: locale().format_number(value),
        });
    }

    pub fn calculate(&mut self) {
        self.doc.calculate(clock::now());
    }

    pub fn calculate_sheet(&mut self) {
        let sheet = self.doc.active();
        self.doc.calculate_sheet(clock::now(), sheet);
    }

    pub fn insert_text(&mut self, ctx: &egui::Context, text: &str) {
        match self.edit.as_mut() {
            Some(edit) => {
                edit.text.push_str(text);
                edit.cursor_to_end = true;
                edit.focus_cell = true;
            }
            None => {
                self.start_edit(text.to_owned(), EditMode::Enter, true);
                if let Some(edit) = self.edit.as_mut() {
                    edit.cursor_to_end = true;
                }
            }
        }
        ctx.request_repaint();
    }

    pub fn open_insert_function(&mut self) {
        self.dialog = Some(Dialog::InsertFunction(FunctionPicker::default()));
    }

    pub fn insert_function(&mut self, ctx: &egui::Context, canonical: &str) {
        let name = locale().function_name(canonical);
        let call = if self.edit.is_some() {
            format!("{name}(")
        } else {
            format!("={name}(")
        };
        self.insert_text(ctx, &call);
    }

    pub fn auto_sum(&mut self, ctx: &egui::Context, function: &str) {
        if self.edit.is_some() && !self.commit_edit(ctx) {
            return;
        }
        let name = locale().function_name(function);
        let range = self.selected();
        let single = self
            .sheet()
            .layout()
            .merge_at(self.sel.active)
            .map_or(range.is_single(), |merge| merge == range);
        if single {
            let source = format::sum_source(self.sheet(), self.sel.active);
            self.insert_text(ctx, &format!("={name}("));
            if let (Some(edit), Some(source)) = (self.edit.as_mut(), source) {
                edit.point_at(source.start(), source.end());
            }
            return;
        }
        let (start, end) = (range.start(), range.end());
        let at = |row: u32, col: u32| CellAddr::new(row, col).expect("inside the sheet");
        let sheet = self.sheet();
        let bottom_free = range.rows() > 1
            && (start.col()..=end.col()).all(|col| {
                sheet
                    .get(at(end.row(), col))
                    .is_none_or(|c| !c.has_content())
            });
        let (target, last) = if bottom_free {
            (end.row(), end.row() - 1)
        } else {
            (end.row() + 1, end.row())
        };
        if target >= MAX_ROWS {
            return;
        }
        let edits: Vec<(CellAddr, vellum_grid_core::Cell)> = (start.col()..=end.col())
            .filter_map(|col| {
                let source = CellRange::new(at(start.row(), col), at(last, col));
                let numbers = sheet
                    .cells_in(source)
                    .any(|(_, cell)| matches!(cell.value, CellValue::Number(_)));
                numbers.then(|| {
                    let cell = at(target, col);
                    (
                        cell,
                        self.doc.styled_input(cell, &format!("={name}({source})")),
                    )
                })
            })
            .collect();
        self.doc.edit(edits);
        self.select(Selection {
            active: start,
            anchor: start,
            corner: at(target, end.col()),
        });
    }
}

fn border_glyph(kind: BorderKind) -> fluent::Glyph {
    match kind {
        BorderKind::Bottom => fluent::BORDER_BOTTOM,
        BorderKind::Top => fluent::BORDER_TOP,
        BorderKind::Left => fluent::BORDER_LEFT,
        BorderKind::Right => fluent::BORDER_RIGHT,
        BorderKind::None => fluent::BORDER_NONE,
        BorderKind::All => fluent::BORDER_ALL,
        BorderKind::Outside => fluent::BORDER_OUTSIDE,
        BorderKind::ThickOutside => fluent::BORDER_OUTSIDE_THICK,
        BorderKind::DoubleBottom => fluent::BORDER_BOTTOM_DOUBLE,
        BorderKind::ThickBottom => fluent::BORDER_BOTTOM_THICK,
        BorderKind::TopBottom => fluent::BORDER_TOP_BOTTOM,
    }
}
