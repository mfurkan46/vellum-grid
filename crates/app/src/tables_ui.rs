use eframe::egui::{
    self, CornerRadius, Popup, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, vec2,
};
use vellum_grid_core::table_style::{Family, StyleId, TableLook, table_look};
use vellum_grid_core::{
    Borders, CellAddr, CellRange, Table, TableColumn, TotalFunction, current_region, guess_header,
};

use crate::app::{Selection, Spreadsheet};
use crate::dialogs::Dialog;
use crate::icons::Icon;
use crate::ribbon::widgets::{big, chrome_text, group, small};
use crate::{fluent, render, text, theme};

#[derive(Debug, Clone)]
pub struct TableForm {
    pub range: String,
    pub headers: bool,
    pub style: String,
    pub resize: Option<usize>,
    pub error: bool,
}

pub fn total_label(function: TotalFunction) -> &'static str {
    match function {
        TotalFunction::None | TotalFunction::Custom => text::total_none(),
        TotalFunction::Average => text::total_average(),
        TotalFunction::Count => text::total_count(),
        TotalFunction::CountNums => text::total_count_nums(),
        TotalFunction::Max => text::total_max(),
        TotalFunction::Min => text::total_min(),
        TotalFunction::Sum => text::total_sum(),
        TotalFunction::StdDev => text::total_stddev(),
        TotalFunction::Var => text::total_var(),
    }
}

fn style_label(id: StyleId) -> String {
    let family = match id.family {
        Family::Light => text::style_light(),
        Family::Medium => text::style_medium(),
        Family::Dark => text::style_dark(),
    };
    format!("{family} {}", id.number)
}

fn absolute(range: CellRange) -> String {
    let corner = |addr: CellAddr| {
        format!(
            "${}${}",
            vellum_grid_core::column_name(addr.col()),
            addr.row() + 1
        )
    };
    if range.is_single() {
        format!("={}", corner(range.start()))
    } else {
        format!("={}:{}", corner(range.start()), corner(range.end()))
    }
}

fn parse_range(text: &str) -> Option<CellRange> {
    let cleaned: String = text
        .trim()
        .trim_start_matches('=')
        .chars()
        .filter(|c| *c != '$')
        .collect();
    cleaned.parse().ok()
}

fn thumbnail(ui: &mut Ui, id: StyleId, selected: bool) -> egui::Response {
    let size = vec2(theme::TABLE_THUMB_WIDTH, theme::TABLE_THUMB_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, theme::cell_bg());
    let mut sample = Table::new(
        "",
        "A1:E5".parse().expect("valid range"),
        (0..5).map(|_| TableColumn::new("")).collect(),
    );
    sample.style = id.name();
    let inner = rect.shrink(theme::TABLE_THUMB_MARGIN);
    let cell = vec2(inner.width() / 5.0, inner.height() / 5.0);
    for row in 0..5 {
        for col in 0..5 {
            let addr = CellAddr::new(row, col).expect("thumbnail cell");
            let Some(look) = table_look(&sample, addr) else {
                continue;
            };
            let min = inner.min + vec2(col as f32 * cell.x, row as f32 * cell.y);
            let box_ = Rect::from_min_size(min, cell);
            if let Some(fill) = look.fill {
                painter.rect_filled(box_, CornerRadius::ZERO, render::color(fill));
            }
            let dash = Rect::from_center_size(box_.center(), vec2(cell.x * 0.5, 1.0));
            let ink = look.color.map_or(theme::chrome_text_muted(), render::color);
            painter.rect_filled(dash, CornerRadius::ZERO, ink.gamma_multiply(0.6));
            let edges = [
                (look.top, box_.left_top(), box_.right_top()),
                (look.bottom, box_.left_bottom(), box_.right_bottom()),
                (look.left, box_.left_top(), box_.left_bottom()),
                (look.right, box_.right_top(), box_.right_bottom()),
            ];
            for (border, a, b) in edges {
                if let Some(border) = border {
                    painter.line_segment([a, b], Stroke::new(1.0, render::color(border.color)));
                }
            }
        }
    }
    let line = if selected || response.hovered() {
        Stroke::new(2.0, theme::accent())
    } else {
        Stroke::new(1.0, theme::ribbon_line())
    };
    painter.rect_stroke(rect, CornerRadius::ZERO, line, StrokeKind::Inside);
    response.on_hover_text(style_label(id))
}

pub fn style_gallery(ui: &mut Ui, current: Option<&str>) -> Option<String> {
    let mut chosen = None;
    for (family, label) in [
        (Family::Light, text::style_light()),
        (Family::Medium, text::style_medium()),
        (Family::Dark, text::style_dark()),
    ] {
        ui.label(chrome_text(label));
        let ids: Vec<StyleId> = StyleId::all().filter(|id| id.family == family).collect();
        for row in ids.chunks(7) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::SWATCH_GAP * 2.0;
                for id in row {
                    let selected = current == Some(id.name().as_str());
                    if thumbnail(ui, *id, selected).clicked() {
                        chosen = Some(id.name());
                    }
                }
            });
        }
    }
    chosen
}

pub fn form_ui(ui: &mut Ui, form: &mut TableForm) {
    ui.set_width(theme::GO_TO_WIDTH);
    let prompt = if form.resize.is_some() {
        text::resize_table_prompt()
    } else {
        text::table_data_where()
    };
    ui.label(prompt);
    let field = ui.add(
        TextEdit::singleline(&mut form.range)
            .desired_width(f32::INFINITY)
            .background_color(theme::input_bg()),
    );
    if field.changed() {
        form.error = false;
    }
    if form.resize.is_none() {
        ui.checkbox(&mut form.headers, text::table_has_headers());
    }
    if form.error {
        ui.label(RichText::new(text::bad_table_range()).color(theme::error_text()));
    }
}

impl Spreadsheet {
    pub fn table_look_at(&self, addr: CellAddr) -> Option<TableLook> {
        self.doc
            .book()
            .table_at(self.doc.active(), addr)
            .and_then(|table| table_look(table, addr))
    }

    pub fn active_table(&self) -> Option<usize> {
        self.doc.table_index_at(self.sel.active)
    }

    pub fn open_create_table(&mut self, style: Option<String>) {
        let selected = self.selected();
        let range = if selected.is_single() {
            current_region(self.sheet(), selected.start())
        } else {
            selected
        };
        self.dialog = Some(Dialog::Table(TableForm {
            range: absolute(range),
            headers: guess_header(self.sheet(), range),
            style: style.unwrap_or_else(|| vellum_grid_core::table::DEFAULT_TABLE_STYLE.into()),
            resize: None,
            error: false,
        }));
    }

    fn open_resize_table(&mut self, index: usize) {
        let table = &self.sheet().layout().tables[index];
        self.dialog = Some(Dialog::Table(TableForm {
            range: absolute(table.range),
            headers: table.header,
            style: table.style.clone(),
            resize: Some(index),
            error: false,
        }));
    }

    pub fn apply_table_form(&mut self, mut form: TableForm) {
        let Some(range) = parse_range(&form.range) else {
            form.error = true;
            self.dialog = Some(Dialog::Table(form));
            return;
        };
        if let Some(index) = form.resize {
            if !self.doc.resize_table(index, range) {
                form.error = true;
                self.dialog = Some(Dialog::Table(form));
            }
            return;
        }
        let headers = form.headers;
        let style = form.style.clone();
        let mut created = None;
        self.doc.batch(|doc| {
            created = doc.create_table(range, headers);
            if let Some(index) = created {
                doc.update_table(index, |t| t.style = style);
            }
        });
        match created.map(|index| self.sheet().layout().tables[index].range) {
            Some(range) => self.select(Selection {
                active: range.start(),
                anchor: range.start(),
                corner: range.end(),
            }),
            None => {
                form.error = true;
                self.dialog = Some(Dialog::Table(form));
            }
        }
    }

    pub fn choose_table_style(&mut self, style: String) {
        match self.active_table() {
            Some(index) => {
                self.doc.update_table(index, |t| t.style = style);
            }
            None => self.open_create_table(Some(style)),
        }
    }

    pub fn total_source(&self) -> Option<(usize, usize)> {
        let addr = self.sel.active;
        let index = self.active_table()?;
        let table = &self.sheet().layout().tables[index];
        (table.totals_row() == Some(addr.row()))
            .then(|| (index, (addr.col() - table.range.start().col()) as usize))
    }

    pub fn design_tab(&mut self, ui: &mut Ui) {
        let Some(index) = self.active_table() else {
            return;
        };
        let table = self.sheet().layout().tables[index].clone();
        group(ui, text::properties(), |ui| {
            ui.vertical(|ui| {
                ui.label(chrome_text(text::table_name()));
                let id = ui.scope_id().with("table-name");
                let focused = ui.memory(|m| m.has_focus(id));
                let mut name = match (&self.table_name_edit, focused) {
                    (Some(buffer), true) => buffer.clone(),
                    _ => table.name.clone(),
                };
                let response = ui.add(
                    TextEdit::singleline(&mut name)
                        .id(id)
                        .desired_width(theme::TABLE_NAME_WIDTH)
                        .background_color(theme::input_bg()),
                );
                if focused || response.changed() {
                    self.table_name_edit = Some(name);
                }
                if response.lost_focus()
                    && let Some(name) = self.table_name_edit.take()
                    && self.doc.rename_table(index, &name).is_err()
                {
                    self.dialog = Some(Dialog::Error {
                        title: text::app_name(),
                        message: text::table_name_invalid().to_owned(),
                    });
                }
                let icon = Icon::Plain(fluent::TABLE_RESIZE_ROW);
                let label = text::resize_table();
                if small(ui, icon, label, label, false).clicked() {
                    self.open_resize_table(index);
                }
            });
        });
        group(ui, text::tools(), |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let icon = Icon::Plain(fluent::TABLE_DELETE_ROW);
                let label = text::remove_duplicates();
                if small(ui, icon, label, label, false).clicked() {
                    self.remove_duplicates_in(table.filter_range());
                }
                let icon = Icon::Plain(fluent::TABLE_SIMPLE);
                let label = text::convert_to_range();
                if small(ui, icon, label, label, false).clicked() {
                    self.dialog = Some(Dialog::ConvertTable(index));
                }
            });
        });
        group(ui, text::table_style_options(), |ui| {
            let mut edited = table.clone();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = theme::RIBBON_COLUMN_GAP;
                ui.vertical(|ui| {
                    ui.checkbox(&mut edited.header, chrome_text(text::header_row()));
                    ui.checkbox(&mut edited.totals, chrome_text(text::total_row()));
                    ui.checkbox(&mut edited.banded_rows, chrome_text(text::banded_rows()));
                });
                ui.vertical(|ui| {
                    ui.checkbox(&mut edited.first_col, chrome_text(text::first_column()));
                    ui.checkbox(&mut edited.last_col, chrome_text(text::last_column()));
                    ui.checkbox(&mut edited.banded_cols, chrome_text(text::banded_columns()));
                });
                ui.vertical(|ui| {
                    ui.add_enabled(
                        edited.header,
                        egui::Checkbox::new(&mut edited.filter, chrome_text(text::filter_button())),
                    );
                });
            });
            if edited != table {
                self.doc.update_table(index, |t| *t = edited);
            }
        });
        group(ui, text::table_styles(), |ui| {
            let icon = Icon::Tinted(fluent::TABLE, theme::tint_blue());
            let button = big(ui, icon, text::quick_styles(), text::quick_styles(), false);
            Popup::menu(&button).show(|ui| {
                if let Some(style) = style_gallery(ui, Some(&table.style)) {
                    self.doc.update_table(index, |t| t.style = style);
                    ui.close();
                }
            });
        });
    }

    pub fn paint_table_backgrounds(
        &self,
        visible: CellRange,
        rect_of: impl Fn(CellAddr) -> Rect,
        skip: impl Fn(CellAddr) -> bool,
    ) -> Vec<(Rect, Option<vellum_grid_core::Color>, Borders)> {
        let sheet = self.sheet();
        let mut out = Vec::new();
        for table in &sheet.layout().tables {
            let top = table.range.start().row().max(visible.start().row());
            let bottom = table.range.end().row().min(visible.end().row());
            let left = table.range.start().col().max(visible.start().col());
            let right = table.range.end().col().min(visible.end().col());
            for row in top..=bottom.max(top) {
                for col in left..=right.max(left) {
                    let Ok(addr) = CellAddr::new(row, col) else {
                        continue;
                    };
                    if row > bottom || col > right || sheet.get(addr).is_some() || skip(addr) {
                        continue;
                    }
                    if let Some(look) = table_look(table, addr) {
                        out.push((rect_of(addr), look.fill, look.borders()));
                    }
                }
            }
        }
        out
    }
}
