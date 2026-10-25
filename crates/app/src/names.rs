use eframe::egui::{
    self, Align, ComboBox, CornerRadius, Layout, Rect, RichText, ScrollArea, Sense, TextEdit, Ui,
    pos2, vec2,
};
use vellum_grid_core::{CellRange, CellValue, DefinedName, NameError, fold, name_from_label};

use crate::app::{Selection, Spreadsheet};
use crate::dialogs::{Dialog, plain_button};
use crate::{text, theme};

#[derive(Debug, Clone)]
pub struct NameForm {
    pub replacing: Option<usize>,
    pub name: String,
    pub scope: Option<usize>,
    pub comment: String,
    pub refers: String,
    pub back: bool,
    pub error: Option<String>,
    pub focused: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ManagerState {
    pub selected: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct NameRow {
    pub name: String,
    pub value: String,
    pub refers: String,
    pub scope: String,
    pub comment: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerAction {
    New,
    Edit(usize),
    Delete(usize),
    Close,
}

pub fn scope_label(scope: Option<usize>, sheets: &[String]) -> String {
    scope
        .and_then(|index| sheets.get(index))
        .map_or_else(|| text::workbook_scope().to_owned(), Clone::clone)
}

pub fn error_text(error: &NameError) -> &'static str {
    match error {
        NameError::Invalid(_) => text::name_invalid(),
        NameError::Duplicate(_) => text::name_duplicate(),
        NameError::Formula => text::name_bad_reference(),
    }
}

pub fn form_ui(ui: &mut Ui, form: &mut NameForm, sheets: &[String]) {
    egui::Grid::new("define_name")
        .num_columns(2)
        .spacing(vec2(theme::DIALOG_SPACING, theme::DIALOG_SPACING))
        .show(ui, |ui| {
            ui.label(text::name_label());
            let name = ui.add(
                TextEdit::singleline(&mut form.name)
                    .desired_width(theme::NAME_INPUT_WIDTH)
                    .background_color(theme::input_bg()),
            );
            if !form.focused {
                name.request_focus();
                form.focused = true;
            }
            ui.end_row();
            ui.label(text::scope_label());
            ui.add_enabled_ui(form.replacing.is_none(), |ui| {
                ComboBox::from_id_salt("name_scope")
                    .selected_text(scope_label(form.scope, sheets))
                    .width(theme::NAME_INPUT_WIDTH)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut form.scope, None, text::workbook_scope());
                        for (index, sheet) in sheets.iter().enumerate() {
                            ui.selectable_value(&mut form.scope, Some(index), sheet);
                        }
                    });
            });
            ui.end_row();
            ui.label(text::comment_label());
            ui.add(
                TextEdit::multiline(&mut form.comment)
                    .desired_rows(3)
                    .desired_width(theme::NAME_INPUT_WIDTH)
                    .background_color(theme::input_bg()),
            );
            ui.end_row();
            ui.label(text::refers_to());
            ui.add(
                TextEdit::singleline(&mut form.refers)
                    .desired_width(theme::NAME_INPUT_WIDTH)
                    .background_color(theme::input_bg()),
            );
            ui.end_row();
        });
    if let Some(error) = &form.error {
        ui.label(RichText::new(error).color(theme::error_text()));
    }
}

const COLUMNS: [f32; 4] = [
    theme::NAME_COLUMN,
    theme::NAME_VALUE_COLUMN,
    theme::NAME_REFERS_COLUMN,
    theme::NAME_SCOPE_COLUMN,
];

fn row_texts(row: &NameRow) -> [&str; 5] {
    [&row.name, &row.value, &row.refers, &row.scope, &row.comment]
}

fn paint_columns(ui: &Ui, rect: Rect, texts: [&str; 5], muted: bool) {
    let color = if muted {
        theme::chrome_text_muted()
    } else {
        theme::chrome_text()
    };
    let mut left = rect.left() + theme::NAME_CELL_PADDING;
    for (index, part) in texts.iter().enumerate() {
        let width = COLUMNS.get(index).copied().unwrap_or(rect.right() - left);
        let clip = Rect::from_min_max(
            pos2(left, rect.top()),
            pos2(left + width - theme::NAME_CELL_PADDING, rect.bottom()),
        );
        let painter = ui.painter().with_clip_rect(clip.intersect(rect));
        let galley = painter.layout_no_wrap((*part).to_owned(), theme::menu_font(), color);
        let y = rect.center().y - galley.size().y / 2.0;
        painter.galley(pos2(left, y), galley, color);
        left += width;
    }
}

pub fn manager_ui(
    ui: &mut Ui,
    state: &mut ManagerState,
    rows: &[NameRow],
) -> Option<ManagerAction> {
    let mut action = None;
    if state.selected.is_some_and(|index| index >= rows.len()) {
        state.selected = None;
    }
    ui.horizontal(|ui| {
        if ui.add(plain_button(text::new_name_button())).clicked() {
            action = Some(ManagerAction::New);
        }
        let selected = state.selected;
        if ui
            .add_enabled(selected.is_some(), plain_button(text::edit_name_button()))
            .clicked()
        {
            action = selected.map(ManagerAction::Edit);
        }
        if ui
            .add_enabled(selected.is_some(), plain_button(text::delete_name()))
            .clicked()
        {
            action = selected.map(ManagerAction::Delete);
        }
    });
    egui::Frame::NONE
        .stroke(egui::Stroke::new(1.0, theme::chrome_line()))
        .fill(theme::input_bg())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let width = ui.available_width();
            let (header, _) =
                ui.allocate_exact_size(vec2(width, theme::NAME_ROW_HEIGHT), Sense::hover());
            paint_columns(
                ui,
                header,
                [
                    text::name_header(),
                    text::value_header(),
                    text::refers_header(),
                    text::scope_header(),
                    text::comment_header(),
                ],
                true,
            );
            ScrollArea::vertical()
                .max_height(theme::NAME_LIST_HEIGHT)
                .min_scrolled_height(theme::NAME_LIST_HEIGHT)
                .show(ui, |ui| {
                    ui.set_min_height(theme::NAME_LIST_HEIGHT);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if rows.is_empty() {
                        ui.add_space(theme::NAME_CELL_PADDING);
                        ui.label(RichText::new(text::no_names()).color(theme::chrome_text_muted()));
                    }
                    for (index, row) in rows.iter().enumerate() {
                        let (rect, response) = ui.allocate_exact_size(
                            vec2(width, theme::NAME_ROW_HEIGHT),
                            Sense::click(),
                        );
                        let selected = state.selected == Some(index);
                        if selected || response.hovered() {
                            let fill = if selected {
                                theme::list_selected()
                            } else {
                                theme::tool_hover()
                            };
                            ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
                        }
                        paint_columns(ui, rect, row_texts(row), false);
                        if response.clicked() {
                            state.selected = Some(index);
                        }
                        if response.double_clicked() {
                            action = Some(ManagerAction::Edit(index));
                        }
                    }
                });
        });
    ui.label(text::refers_to());
    let mut refers = state
        .selected
        .and_then(|index| rows.get(index))
        .map(|row| row.refers.clone())
        .unwrap_or_default();
    ui.add_enabled(
        false,
        TextEdit::singleline(&mut refers)
            .desired_width(f32::INFINITY)
            .background_color(theme::input_bg()),
    );
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        if ui.add(plain_button(text::close())).clicked() {
            action = Some(ManagerAction::Close);
        }
    });
    action
}

pub fn create_ui(ui: &mut Ui, edges: &mut [bool; 4]) {
    ui.label(text::create_names_from());
    for (value, label) in edges.iter_mut().zip([
        text::top_row(),
        text::left_column(),
        text::bottom_row(),
        text::right_column(),
    ]) {
        ui.checkbox(value, label);
    }
}

impl Spreadsheet {
    pub fn name_rows(&self) -> Vec<NameRow> {
        let sheets = self.sheet_names();
        self.doc
            .book()
            .names()
            .iter()
            .map(|defined| NameRow {
                name: defined.name.clone(),
                value: self.doc.name_value(defined),
                refers: self.doc.name_formula_text(&defined.formula),
                scope: scope_label(defined.scope, &sheets),
                comment: defined.comment.clone(),
            })
            .collect()
    }

    pub fn sheet_names(&self) -> Vec<String> {
        self.doc
            .book()
            .sheets()
            .iter()
            .map(|sheet| sheet.name().to_owned())
            .collect()
    }

    pub fn open_name_manager(&mut self) {
        self.dialog = Some(Dialog::NameManager(ManagerState::default()));
    }

    pub fn open_define_name(&mut self, back: bool) {
        let range = self.selected();
        let formula = self.doc.absolute_reference(range.start(), range.end());
        self.dialog = Some(Dialog::DefineName(NameForm {
            replacing: None,
            name: self.suggested_name(range).unwrap_or_default(),
            scope: None,
            comment: String::new(),
            refers: self.doc.name_formula_text(&formula),
            back,
            error: None,
            focused: false,
        }));
    }

    pub fn open_edit_name(&mut self, index: usize) {
        let Some(defined) = self.doc.book().names().get(index) else {
            return;
        };
        self.dialog = Some(Dialog::DefineName(NameForm {
            replacing: Some(index),
            name: defined.name.clone(),
            scope: defined.scope,
            comment: defined.comment.clone(),
            refers: self.doc.name_formula_text(&defined.formula),
            back: true,
            error: None,
            focused: false,
        }));
    }

    pub fn open_create_names(&mut self) {
        let range = self.selected();
        let below = range.start().offset(1, 0);
        let left = range.cols() > 1
            && matches!(self.sheet().value(below), CellValue::Text(_))
            && range.contains(below);
        self.dialog = Some(Dialog::CreateNames {
            range,
            edges: [range.rows() > 1, left, false, false],
        });
    }

    fn suggested_name(&self, range: CellRange) -> Option<String> {
        let start = range.start();
        [start.offset(-1, 0), start.offset(0, -1)]
            .into_iter()
            .filter(|addr| *addr != start)
            .find_map(|addr| match self.sheet().value(addr) {
                CellValue::Text(label) => name_from_label(label),
                _ => None,
            })
    }

    pub fn submit_name(&mut self, mut form: NameForm) {
        let result = self
            .doc
            .parse_name_formula(&form.refers)
            .and_then(|formula| {
                let defined = DefinedName {
                    name: form.name.trim().to_owned(),
                    scope: form.scope,
                    formula,
                    comment: form.comment.trim().to_owned(),
                };
                self.doc.define_name(form.replacing, defined)
            });
        match result {
            Ok(_) if form.back => {
                let key = fold(form.name.trim());
                let selected =
                    self.doc.book().names().iter().position(|defined| {
                        fold(&defined.name) == key && defined.scope == form.scope
                    });
                self.dialog = Some(Dialog::NameManager(ManagerState { selected }));
            }
            Ok(_) => {}
            Err(error) => {
                form.error = Some(error_text(&error).to_owned());
                self.dialog = Some(Dialog::DefineName(form));
            }
        }
    }

    pub fn manager_action(&mut self, action: ManagerAction) {
        match action {
            ManagerAction::New => self.open_define_name(true),
            ManagerAction::Edit(index) => self.open_edit_name(index),
            ManagerAction::Delete(index) => {
                self.doc.delete_names(&[index]);
                let left = self.doc.book().names().len();
                let selected = (left > 0).then(|| index.min(left - 1));
                self.dialog = Some(Dialog::NameManager(ManagerState { selected }));
            }
            ManagerAction::Close => {}
        }
    }

    pub fn create_names(&mut self, range: CellRange, edges: [bool; 4]) {
        if self.doc.create_names(range, edges) == 0 {
            self.dialog = Some(Dialog::Error {
                title: text::app_name(),
                message: text::no_names_created().to_owned(),
            });
        }
    }

    pub fn go_to_name(&mut self, name: &str) -> bool {
        let Some((sheet, range)) = self.doc.book().name_range(name, self.doc.active()) else {
            return false;
        };
        if sheet != self.doc.active() {
            self.switch_sheet(sheet);
        }
        self.select(Selection {
            active: range.start(),
            anchor: range.start(),
            corner: range.end(),
        });
        true
    }

    pub fn selection_name(&self) -> Option<String> {
        let range = self.selected();
        let active = self.doc.active();
        let book = self.doc.book();
        book.names()
            .iter()
            .filter(|defined| defined.scope.is_none_or(|scope| scope == active))
            .find(|defined| book.name_range(&defined.name, active) == Some((active, range)))
            .map(|defined| defined.name.clone())
    }

    pub fn insert_name(&mut self, ctx: &egui::Context, name: &str) {
        let text = if self.edit.is_some() {
            name.to_owned()
        } else {
            format!("={name}")
        };
        self.insert_text(ctx, &text);
    }
}
