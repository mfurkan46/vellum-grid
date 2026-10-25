use eframe::egui::{
    self, ComboBox, CornerRadius, FontFamily, FontId, RichText, ScrollArea, Sense, Stroke,
    TextEdit, Ui, pos2, vec2,
};
use vellum_grid_core::tools::{ColumnKind, DateOrder, Delimiters, Split, source_text, split_line};
use vellum_grid_core::{CellAddr, CellRange, current_region, guess_header};
use vellum_grid_io::encoding::{TextEncoding, decode_with};

use crate::app::{Selection, Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::{text, theme};

const PREVIEW_LINES: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualifier {
    Double,
    Single,
    Nothing,
}

impl Qualifier {
    pub const ALL: [Self; 3] = [Self::Double, Self::Single, Self::Nothing];

    pub fn label(self) -> &'static str {
        match self {
            Self::Double => "\"",
            Self::Single => "'",
            Self::Nothing => text::qualifier_none(),
        }
    }

    fn char(self) -> Option<char> {
        match self {
            Self::Double => Some('"'),
            Self::Single => Some('\''),
            Self::Nothing => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SplitState {
    pub source: Option<CellRange>,
    pub lines: Vec<String>,
    pub fixed: bool,
    pub delimiters: Delimiters,
    pub other: bool,
    pub other_text: String,
    pub merge: bool,
    pub qualifier: Qualifier,
    pub breaks: Vec<usize>,
    pub kinds: Vec<ColumnKind>,
    pub column: usize,
    pub dest: String,
    pub warning: Option<&'static str>,
}

impl SplitState {
    pub fn new(source: Option<CellRange>, lines: Vec<String>, dest: String) -> Self {
        let sample = lines.join("\n");
        let delimiters = Delimiters {
            tab: sample.contains('\t'),
            semicolon: sample.contains(';'),
            comma: !sample.contains(';') && sample.contains(','),
            ..Delimiters::default()
        };
        let delimiters = if delimiters == Delimiters::default() {
            Delimiters {
                tab: true,
                ..Delimiters::default()
            }
        } else {
            delimiters
        };
        Self {
            source,
            lines,
            fixed: false,
            delimiters,
            other: false,
            other_text: String::new(),
            merge: false,
            qualifier: Qualifier::Double,
            breaks: Vec::new(),
            kinds: Vec::new(),
            column: 0,
            dest,
            warning: None,
        }
    }

    pub fn split(&self) -> Split {
        if self.fixed {
            return Split::Fixed(self.breaks.clone());
        }
        Split::Delimited {
            delimiters: Delimiters {
                other: self.other.then(|| self.other_text.chars().next()).flatten(),
                ..self.delimiters.clone()
            },
            merge: self.merge,
            qualifier: self.qualifier.char(),
        }
    }

    pub fn rows(&self) -> Vec<Vec<String>> {
        let split = self.split();
        self.lines
            .iter()
            .take(PREVIEW_LINES)
            .map(|line| split_line(line, &split))
            .collect()
    }

    pub fn kind(&self, column: usize) -> ColumnKind {
        self.kinds
            .get(column)
            .copied()
            .unwrap_or(ColumnKind::General)
    }

    fn set_kind(&mut self, column: usize, kind: ColumnKind) {
        if self.kinds.len() <= column {
            self.kinds.resize(column + 1, ColumnKind::General);
        }
        self.kinds[column] = kind;
    }
}

pub fn kind_label(kind: ColumnKind) -> String {
    match kind {
        ColumnKind::General => text::kind_general().to_owned(),
        ColumnKind::Text => text::kind_text_column().to_owned(),
        ColumnKind::Date(order) => format!("{} ({})", text::kind_date(), order.code()),
        ColumnKind::Skip => text::kind_skip().to_owned(),
    }
}

fn fixed_preview(ui: &mut Ui, state: &mut SplitState) {
    let font = FontId::new(theme::PREVIEW_FONT, FontFamily::Monospace);
    let char_width = ui
        .painter()
        .layout_no_wrap("0".into(), font.clone(), theme::chrome_text())
        .size()
        .x;
    let longest = state
        .lines
        .iter()
        .take(PREVIEW_LINES)
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
        .max(theme::PREVIEW_MIN_CHARS);
    let lines = state.lines.len().min(PREVIEW_LINES);
    let size = vec2(
        longest as f32 * char_width + theme::PREVIEW_PADDING * 2.0,
        (lines as f32 + 1.0) * theme::PREVIEW_LINE,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let left = rect.left() + theme::PREVIEW_PADDING;
    for tick in (0..=longest).step_by(10) {
        let x = left + tick as f32 * char_width;
        painter.text(
            pos2(x, rect.top()),
            egui::Align2::CENTER_TOP,
            tick.to_string(),
            FontId::new(theme::PREVIEW_FONT * 0.8, FontFamily::Monospace),
            theme::chrome_text_muted(),
        );
    }
    for (index, line) in state.lines.iter().take(PREVIEW_LINES).enumerate() {
        painter.text(
            pos2(
                left,
                rect.top() + (index as f32 + 1.0) * theme::PREVIEW_LINE,
            ),
            egui::Align2::LEFT_TOP,
            line,
            font.clone(),
            theme::chrome_text(),
        );
    }
    for at in &state.breaks {
        let x = left + *at as f32 * char_width;
        painter.vline(x, rect.y_range(), Stroke::new(1.5, theme::accent()));
    }
    if let Some(pos) = response
        .interact_pointer_pos()
        .filter(|_| response.clicked())
    {
        let at = ((pos.x - left) / char_width).round().max(0.0) as usize;
        match state.breaks.iter().position(|b| b.abs_diff(at) <= 1) {
            Some(index) => {
                state.breaks.remove(index);
            }
            None if at > 0 => {
                state.breaks.push(at);
                state.breaks.sort_unstable();
            }
            None => {}
        }
    }
}

fn table_preview(ui: &mut Ui, state: &mut SplitState, rows: &[Vec<String>]) {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(1);
    egui::Grid::new("split_preview")
        .striped(true)
        .spacing(vec2(theme::PREVIEW_GAP, theme::PREVIEW_ROW_GAP))
        .min_col_width(theme::PREVIEW_COLUMN)
        .show(ui, |ui| {
            for column in 0..columns {
                let label = kind_label(state.kind(column));
                if ui
                    .selectable_label(state.column == column, RichText::new(label).strong())
                    .clicked()
                {
                    state.column = column;
                }
            }
            ui.end_row();
            for row in rows {
                for column in 0..columns {
                    let value = row.get(column).map_or("", String::as_str);
                    let muted = state.kind(column) == ColumnKind::Skip;
                    let color = if muted {
                        theme::chrome_text_muted()
                    } else {
                        theme::chrome_text()
                    };
                    ui.label(RichText::new(value).color(color));
                }
                ui.end_row();
            }
        });
}

pub fn split_ui(ui: &mut Ui, state: &mut SplitState) {
    ui.set_width(theme::SPLIT_DIALOG_WIDTH);
    ui.horizontal(|ui| {
        ui.radio_value(&mut state.fixed, false, text::delimited());
        ui.radio_value(&mut state.fixed, true, text::fixed_width());
    });
    if state.fixed {
        ui.label(RichText::new(text::fixed_hint()).color(theme::chrome_text_muted()));
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut state.delimiters.tab, text::delim_tab());
            ui.checkbox(&mut state.delimiters.semicolon, text::delim_semicolon());
            ui.checkbox(&mut state.delimiters.comma, text::delim_comma());
            ui.checkbox(&mut state.delimiters.space, text::delim_space());
            ui.checkbox(&mut state.other, text::delim_other());
            ui.add(
                TextEdit::singleline(&mut state.other_text)
                    .char_limit(1)
                    .desired_width(theme::NUMBER_INPUT_WIDTH / 2.0)
                    .background_color(theme::input_bg()),
            );
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.merge, text::merge_delimiters());
            ui.label(text::qualifier());
            ComboBox::from_id_salt("qualifier")
                .selected_text(state.qualifier.label())
                .show_ui(ui, |ui| {
                    for qualifier in Qualifier::ALL {
                        ui.selectable_value(&mut state.qualifier, qualifier, qualifier.label());
                    }
                });
        });
    }
    ui.label(text::data_preview());
    let rows = state.rows();
    egui::Frame::NONE
        .stroke(Stroke::new(1.0, theme::chrome_line()))
        .fill(theme::input_bg())
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::same(4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ScrollArea::both()
                .max_height(theme::PREVIEW_HEIGHT)
                .min_scrolled_height(theme::PREVIEW_HEIGHT)
                .show(ui, |ui| {
                    if state.fixed {
                        fixed_preview(ui, state);
                    } else {
                        table_preview(ui, state, &rows);
                    }
                });
        });
    let columns = if state.fixed {
        state.breaks.len() + 1
    } else {
        rows.iter().map(Vec::len).max().unwrap_or(1)
    };
    state.column = state.column.min(columns.saturating_sub(1));
    ui.horizontal(|ui| {
        ui.label(text::column_format());
        ComboBox::from_id_salt("split_column")
            .selected_text(format!("{} {}", text::column(), state.column + 1))
            .show_ui(ui, |ui| {
                for column in 0..columns {
                    let label = format!(
                        "{} {} — {}",
                        text::column(),
                        column + 1,
                        kind_label(state.kind(column))
                    );
                    ui.selectable_value(&mut state.column, column, label);
                }
            });
    });
    let column = state.column;
    let mut kind = state.kind(column);
    let mut order = match kind {
        ColumnKind::Date(order) => order,
        _ => DateOrder::Dmy,
    };
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut kind, ColumnKind::General, text::kind_general());
        ui.radio_value(&mut kind, ColumnKind::Text, text::kind_text_column());
        let dated = matches!(kind, ColumnKind::Date(_));
        if ui.radio(dated, text::kind_date()).clicked() {
            kind = ColumnKind::Date(order);
        }
        ComboBox::from_id_salt("date_order")
            .selected_text(order.code())
            .width(theme::NUMBER_INPUT_WIDTH)
            .show_ui(ui, |ui| {
                for option in DateOrder::ALL {
                    if ui
                        .selectable_value(&mut order, option, option.code())
                        .clicked()
                    {
                        kind = ColumnKind::Date(order);
                    }
                }
            });
        ui.radio_value(&mut kind, ColumnKind::Skip, text::kind_skip());
    });
    state.set_kind(column, kind);
    if state.source.is_some() {
        ui.horizontal(|ui| {
            ui.label(text::destination());
            ui.add(
                TextEdit::singleline(&mut state.dest)
                    .desired_width(theme::NAME_INPUT_WIDTH / 2.0)
                    .background_color(theme::input_bg()),
            );
        });
    }
    if let Some(warning) = state.warning {
        ui.label(RichText::new(warning).color(theme::error_text()));
    }
}

#[derive(Debug, Clone)]
pub struct DuplicateState {
    pub range: CellRange,
    pub header: bool,
    pub columns: Vec<(u32, String, String, bool)>,
}

pub fn duplicates_ui(ui: &mut Ui, state: &mut DuplicateState) {
    ui.set_width(theme::GO_TO_WIDTH);
    ui.label(text::duplicates_prompt());
    ui.horizontal(|ui| {
        if ui.button(text::select_all_columns()).clicked() {
            state.columns.iter_mut().for_each(|column| column.3 = true);
        }
        if ui.button(text::unselect_all_columns()).clicked() {
            state.columns.iter_mut().for_each(|column| column.3 = false);
        }
    });
    ui.checkbox(&mut state.header, text::has_headers());
    crate::dialogs::list_box(ui, |ui| {
        for (_, plain, named, checked) in &mut state.columns {
            let label = if state.header {
                named.as_str()
            } else {
                plain.as_str()
            };
            ui.checkbox(checked, label);
        }
    });
}

impl Spreadsheet {
    pub fn open_text_to_columns(&mut self) {
        let range = self.selected();
        if range.cols() > 1 {
            self.dialog = Some(Dialog::Error {
                title: text::text_to_columns(),
                message: text::one_column_only().to_owned(),
            });
            return;
        }
        let source = if range.is_single() {
            let region = current_region(self.sheet(), range.start());
            let col = range.start().col();
            let start = CellAddr::new(region.start().row(), col).expect("inside the sheet");
            let end = CellAddr::new(region.end().row(), col).expect("inside the sheet");
            CellRange::new(start, end)
        } else {
            self.sheet().trim_to_extent(range)
        };
        self.select(Selection {
            active: source.start(),
            anchor: source.start(),
            corner: source.end(),
        });
        let lines: Vec<String> = (source.start().row()..=source.end().row())
            .filter_map(|row| CellAddr::new(row, source.start().col()).ok())
            .map(|addr| source_text(self.sheet().get(addr), locale()))
            .collect();
        if lines.iter().all(String::is_empty) {
            self.dialog = Some(Dialog::Error {
                title: text::text_to_columns(),
                message: text::nothing_to_split().to_owned(),
            });
            return;
        }
        let dest = format!(
            "${}${}",
            vellum_grid_core::column_name(source.start().col()),
            source.start().row() + 1
        );
        self.dialog = Some(Dialog::TextToColumns(SplitState::new(
            Some(source),
            lines,
            dest,
        )));
    }

    pub fn apply_text_to_columns(&mut self, mut state: SplitState) {
        let Some(source) = state.source else {
            return;
        };
        let Some((_, dest)) = crate::goto::parse_reference(&state.dest) else {
            state.warning = Some(text::bad_destination());
            self.dialog = Some(Dialog::TextToColumns(state));
            return;
        };
        let split = state.split();
        let edits = self
            .doc
            .split_columns(source, &split, &state.kinds, dest.start());
        if state.warning != Some(text::replace_destination()) && self.doc.overwrites(&edits, source)
        {
            state.warning = Some(text::replace_destination());
            self.dialog = Some(Dialog::TextToColumns(state));
            return;
        }
        self.doc
            .text_to_columns(source, &split, &state.kinds, dest.start());
    }

    pub fn open_remove_duplicates(&mut self) {
        let range = self.selected();
        if range.is_single() {
            let region = current_region(self.sheet(), range.start());
            self.remove_duplicates_in(region);
            return;
        }
        let range = self.sheet().trim_to_extent(range);
        let region = current_region(self.sheet(), range.start());
        let wider =
            region.start().col() < range.start().col() || region.end().col() > range.end().col();
        if wider && region.contains(range.start()) && region.contains(range.end()) {
            self.dialog = Some(Dialog::ExpandSelection {
                range,
                region,
                expand: true,
            });
            return;
        }
        self.remove_duplicates_in(range);
    }

    pub fn remove_duplicates_in(&mut self, range: CellRange) {
        if self.sheet().cells_in(range).next().is_none() {
            self.dialog = Some(Dialog::Error {
                title: text::remove_duplicates(),
                message: text::nothing_to_split().to_owned(),
            });
            return;
        }
        self.select(Selection {
            active: range.start(),
            anchor: range.start(),
            corner: range.end(),
        });
        let header = guess_header(self.sheet(), range);
        let columns = self
            .column_labels(range)
            .into_iter()
            .map(|(col, plain, named)| (col, plain, named, true))
            .collect();
        self.dialog = Some(Dialog::RemoveDuplicates(DuplicateState {
            range,
            header,
            columns,
        }));
    }

    pub fn apply_remove_duplicates(&mut self, state: DuplicateState) {
        let columns: Vec<u32> = state
            .columns
            .iter()
            .filter(|column| column.3)
            .map(|column| column.0)
            .collect();
        if columns.is_empty() {
            self.dialog = Some(Dialog::RemoveDuplicates(state));
            return;
        }
        let (removed, kept) = self
            .doc
            .remove_duplicates(state.range, &columns, state.header);
        let message = if removed == 0 {
            text::no_duplicates().to_owned()
        } else {
            format!(
                "{removed} {} {kept} {}",
                text::duplicates_removed(),
                text::unique_left()
            )
        };
        self.dialog = Some(Dialog::Error {
            title: text::remove_duplicates(),
            message,
        });
    }

    pub fn group_selection(&mut self, group: bool, rows: Option<bool>) {
        let range = self.selected();
        let rows = match rows {
            Some(rows) => rows,
            None if range.is_full_rows() && !range.is_full_cols() => true,
            None if range.is_full_cols() && !range.is_full_rows() => false,
            None => {
                self.dialog = Some(Dialog::GroupAxis { group, rows: true });
                return;
            }
        };
        let (start, end) = if rows {
            (range.start().row(), range.end().row())
        } else {
            (range.start().col(), range.end().col())
        };
        if group {
            self.doc.group(rows, start, end);
        } else {
            self.doc.ungroup(rows, start, end);
        }
    }

    pub fn detail(&mut self, show: bool) {
        let layout = self.sheet().layout();
        let row = self.sel.active.row();
        let found = vellum_grid_core::outline::groups(&layout.row_levels)
            .into_iter()
            .filter(|group| (group.start..=group.end + 1).contains(&row))
            .max_by_key(|group| group.level);
        let Some(group) = found else {
            return;
        };
        let collapsed = vellum_grid_core::outline::is_collapsed(&layout.hidden_rows, group);
        if collapsed == show {
            self.doc.toggle_group(true, group);
        }
    }

    pub fn open_subtotal(&mut self) {
        let range = current_region(self.sheet(), self.sel.active);
        if range.rows() < 2 {
            self.dialog = Some(Dialog::Error {
                title: text::subtotal(),
                message: text::subtotal_no_list().to_owned(),
            });
            return;
        }
        self.select(Selection {
            active: range.start(),
            anchor: range.start(),
            corner: range.end(),
        });
        let columns: Vec<(u32, String, String, bool)> = self
            .column_labels(range)
            .into_iter()
            .map(|(col, plain, named)| (col, plain, named, col == range.end().col()))
            .collect();
        self.dialog = Some(Dialog::Subtotal(SubtotalState {
            range,
            group_col: range.start().col(),
            function: 0,
            columns,
            replace: true,
            page_breaks: false,
        }));
    }

    pub fn apply_subtotal(&mut self, state: SubtotalState) {
        let (_, code, label, grand) = SUBTOTAL_FUNCTIONS[state.function];
        let columns: Vec<u32> = state
            .columns
            .iter()
            .filter(|column| column.3)
            .map(|column| column.0)
            .collect();
        if columns.is_empty() {
            self.dialog = Some(Dialog::Subtotal(state));
            return;
        }
        self.doc.subtotal(
            state.range,
            state.group_col,
            code,
            &columns,
            (label(), grand()),
            state.replace,
            state.page_breaks,
        );
    }

    pub fn remove_all_subtotals(&mut self, state: &SubtotalState) {
        self.doc.remove_subtotals(state.range);
    }
}

pub const SUBTOTAL_FUNCTIONS: [(text::Label, u8, text::Label, text::Label); 11] = [
    (text::fn_sum, 9, text::label_sum, text::grand_sum),
    (text::fn_count, 3, text::label_count, text::grand_count),
    (
        text::fn_average,
        1,
        text::label_average,
        text::grand_average,
    ),
    (text::fn_max, 4, text::label_max, text::grand_max),
    (text::fn_min, 5, text::label_min, text::grand_min),
    (
        text::fn_product,
        6,
        text::label_product,
        text::grand_product,
    ),
    (
        text::fn_count_numbers,
        2,
        text::label_count,
        text::grand_count,
    ),
    (text::fn_stdev, 7, text::label_stdev, text::grand_stdev),
    (text::fn_stdevp, 8, text::label_stdevp, text::grand_stdevp),
    (text::fn_var, 10, text::label_var, text::grand_var),
    (text::fn_varp, 11, text::label_varp, text::grand_varp),
];

#[derive(Debug, Clone)]
pub struct SubtotalState {
    pub range: CellRange,
    pub group_col: u32,
    pub function: usize,
    pub columns: Vec<(u32, String, String, bool)>,
    pub replace: bool,
    pub page_breaks: bool,
}

pub fn subtotal_ui(ui: &mut Ui, state: &mut SubtotalState) -> bool {
    ui.set_width(theme::GO_TO_WIDTH);
    let label_of = |state: &SubtotalState, col: u32| {
        state
            .columns
            .iter()
            .find(|column| column.0 == col)
            .map_or_else(String::new, |column| column.2.clone())
    };
    ui.label(text::at_each_change());
    ComboBox::from_id_salt("subtotal_group")
        .selected_text(label_of(state, state.group_col))
        .width(theme::NAME_INPUT_WIDTH)
        .show_ui(ui, |ui| {
            for (col, _, named, _) in &state.columns {
                ui.selectable_value(&mut state.group_col, *col, named);
            }
        });
    ui.label(text::use_function());
    ComboBox::from_id_salt("subtotal_function")
        .selected_text(SUBTOTAL_FUNCTIONS[state.function].0())
        .width(theme::NAME_INPUT_WIDTH)
        .show_ui(ui, |ui| {
            for (index, (label, ..)) in SUBTOTAL_FUNCTIONS.iter().enumerate() {
                ui.selectable_value(&mut state.function, index, label());
            }
        });
    ui.label(text::add_subtotal_to());
    crate::dialogs::list_box(ui, |ui| {
        for (_, _, named, checked) in &mut state.columns {
            ui.checkbox(checked, named.as_str());
        }
    });
    ui.checkbox(&mut state.replace, text::replace_subtotals());
    ui.checkbox(&mut state.page_breaks, text::page_break_groups());
    let mut below = true;
    ui.add_enabled(
        false,
        egui::Checkbox::new(&mut below, text::summary_below()),
    );
    ui.add(crate::dialogs::plain_button(text::remove_all()))
        .clicked()
}

const IMPORT_PREVIEW: usize = 40;

#[derive(Debug, Clone)]
pub struct ImportState {
    pub name: String,
    pub bytes: Vec<u8>,
    pub encoding: TextEncoding,
    pub start: usize,
    pub split: SplitState,
    pub new_sheet: bool,
    pub dest: String,
    seen: (TextEncoding, usize),
}

fn encoding_label(encoding: TextEncoding) -> &'static str {
    match encoding {
        TextEncoding::Auto => text::encoding_auto(),
        TextEncoding::Utf8 => text::encoding_utf8(),
        TextEncoding::Windows1254 => text::encoding_turkish(),
        TextEncoding::Utf16 => text::encoding_utf16(),
    }
}

impl ImportState {
    fn lines(&self) -> Vec<String> {
        decode_with(&self.bytes, self.encoding)
            .lines()
            .skip(self.start.saturating_sub(1))
            .map(str::to_owned)
            .collect()
    }

    fn refresh(&mut self) {
        if self.seen != (self.encoding, self.start) {
            self.split.lines = self.lines().into_iter().take(IMPORT_PREVIEW).collect();
            self.seen = (self.encoding, self.start);
        }
    }
}

pub fn import_ui(ui: &mut Ui, state: &mut ImportState) {
    state.refresh();
    ui.label(RichText::new(&state.name).strong());
    ui.horizontal(|ui| {
        ui.label(text::file_origin());
        ComboBox::from_id_salt("import_encoding")
            .selected_text(encoding_label(state.encoding))
            .show_ui(ui, |ui| {
                for encoding in TextEncoding::ALL {
                    ui.selectable_value(&mut state.encoding, encoding, encoding_label(encoding));
                }
            });
        ui.label(text::start_row());
        ui.add(egui::DragValue::new(&mut state.start).range(1..=usize::MAX));
    });
    split_ui(ui, &mut state.split);
    ui.horizontal(|ui| {
        ui.radio_value(&mut state.new_sheet, false, text::existing_sheet());
        ui.add_enabled(
            !state.new_sheet,
            TextEdit::singleline(&mut state.dest)
                .desired_width(theme::NUMBER_INPUT_WIDTH)
                .background_color(theme::input_bg()),
        );
        ui.radio_value(&mut state.new_sheet, true, text::import_new_sheet());
    });
}

impl Spreadsheet {
    pub fn open_text_import(&mut self) {
        let Some((path, bytes)) = crate::files::pick_text() else {
            return;
        };
        let name = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let dest = format!(
            "${}${}",
            vellum_grid_core::column_name(self.sel.active.col()),
            self.sel.active.row() + 1
        );
        let mut state = ImportState {
            name,
            bytes,
            encoding: TextEncoding::Auto,
            start: 1,
            split: SplitState::new(None, Vec::new(), String::new()),
            new_sheet: false,
            dest,
            seen: (TextEncoding::Auto, 0),
        };
        state.refresh();
        state.split = SplitState::new(None, state.split.lines.clone(), String::new());
        self.dialog = Some(Dialog::ImportText(state));
    }

    pub fn apply_text_import(&mut self, mut state: ImportState) {
        let dest = if state.new_sheet {
            let index = self.doc.add_sheet(text::new_sheet_prefix());
            self.switch_sheet(index);
            vellum_grid_core::CellAddr::new(0, 0).expect("A1 is valid")
        } else {
            match crate::goto::parse_reference(&state.dest) {
                Some((_, range)) => range.start(),
                None => {
                    state.split.warning = Some(text::bad_destination());
                    self.dialog = Some(Dialog::ImportText(state));
                    return;
                }
            }
        };
        let split = state.split.split();
        let edits = vellum_grid_core::tools::split_rows(
            state.lines(),
            &split,
            &state.split.kinds,
            dest,
            locale(),
        );
        self.doc.edit(edits);
    }

    pub fn flash_fill(&mut self) {
        let target = self.sel.active.col();
        let region = current_region(self.sheet(), self.sel.active);
        let sheet = self.sheet();
        let header = guess_header(sheet, region);
        let first = region.start().row() + u32::from(header);
        let sources: Vec<u32> = (region.start().col()..=region.end().col())
            .filter(|col| *col != target)
            .collect();
        let text_at = |row: u32, col: u32| {
            CellAddr::new(row, col)
                .map(|addr| source_text(sheet.get(addr), locale()))
                .unwrap_or_default()
        };
        let rows: Vec<(Vec<String>, Option<String>)> = (first..=region.end().row())
            .map(|row| {
                let inputs = sources.iter().map(|col| text_at(row, *col)).collect();
                let example = Some(text_at(row, target)).filter(|text| !text.is_empty());
                (inputs, example)
            })
            .collect();
        let Some(filled) = vellum_grid_core::tools::flash_fill(&rows, locale()) else {
            self.dialog = Some(Dialog::Error {
                title: text::flash_fill(),
                message: text::flash_fill_failed().to_owned(),
            });
            return;
        };
        let edits: Vec<(CellAddr, vellum_grid_core::Cell)> = filled
            .into_iter()
            .enumerate()
            .filter_map(|(offset, value)| {
                let addr = CellAddr::new(first + offset as u32, target).ok()?;
                Some((addr, self.doc.styled_input(addr, &value?)))
            })
            .collect();
        self.doc.edit(edits);
    }
}
