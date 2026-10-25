use std::collections::HashMap;

use vellum_grid_core::{Cell, CellAddr, CellRange, CellValue, Sheet, fold};

use crate::app::{Selection, Spreadsheet, editor_cursor, select_chars};
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteWhat {
    All,
    Formulas,
    Values,
    Formats,
}

impl PasteWhat {
    pub const ALL: [Self; 4] = [Self::All, Self::Formulas, Self::Values, Self::Formats];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => text::paste_all(),
            Self::Formulas => text::paste_formulas(),
            Self::Values => text::paste_values(),
            Self::Formats => text::paste_formats(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOp {
    None,
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl PasteOp {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => text::op_none(),
            Self::Add => text::op_add(),
            Self::Subtract => text::op_subtract(),
            Self::Multiply => text::op_multiply(),
            Self::Divide => text::op_divide(),
        }
    }

    fn apply(self, target: f64, source: f64) -> CellValue {
        match self {
            Self::None => CellValue::Number(source),
            Self::Add => CellValue::Number(target + source),
            Self::Subtract => CellValue::Number(target - source),
            Self::Multiply => CellValue::Number(target * source),
            Self::Divide if source == 0.0 => CellValue::Error(vellum_grid_core::CellError::Div0),
            Self::Divide => CellValue::Number(target / source),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasteOptions {
    pub what: PasteWhat,
    pub op: PasteOp,
    pub skip_blanks: bool,
    pub transpose: bool,
}

impl PasteOptions {
    pub const fn only(what: PasteWhat) -> Self {
        Self {
            what,
            op: PasteOp::None,
            skip_blanks: false,
            transpose: false,
        }
    }
}

impl Default for PasteOptions {
    fn default() -> Self {
        Self::only(PasteWhat::All)
    }
}

pub fn combine(
    source: &Cell,
    from: CellAddr,
    existing: &Cell,
    to: CellAddr,
    options: PasteOptions,
) -> Cell {
    let rows = i64::from(to.row()) - i64::from(from.row());
    let cols = i64::from(to.col()) - i64::from(from.col());
    let mut cell = match options.what {
        PasteWhat::All => source.shifted(rows, cols),
        PasteWhat::Formulas => Cell {
            style: existing.style.clone(),
            ..source.shifted(rows, cols)
        },
        PasteWhat::Values => Cell {
            value: source.value.clone(),
            formula: None,
            style: existing.style.clone(),
        },
        PasteWhat::Formats => Cell {
            style: source.style.clone(),
            ..existing.clone()
        },
    };
    if options.op != PasteOp::None && options.what != PasteWhat::Formats {
        let number = |cell: &Cell| match cell.value {
            CellValue::Number(n) => Some(n),
            CellValue::Empty => Some(0.0),
            _ => None,
        };
        if existing.formula.is_none()
            && let (Some(target), Some(value)) = (number(existing), number(source))
        {
            cell.value = options.op.apply(target, value);
            cell.formula = None;
        }
    }
    cell
}

pub fn toggle_reference(text: &str, cursor: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let part = |c: char| c.is_ascii_alphanumeric() || c == '$';
    let mut start = cursor.min(chars.len());
    while start > 0 && part(chars[start - 1]) {
        start -= 1;
    }
    let mut end = start;
    while end < chars.len() && part(chars[end]) {
        end += 1;
    }
    if end < chars.len() && chars[end] == '(' {
        return None;
    }
    let token: String = chars[start..end].iter().collect();
    let (col_abs, rest) = match token.strip_prefix('$') {
        Some(rest) => (true, rest),
        None => (false, token.as_str()),
    };
    let letters = rest.chars().take_while(char::is_ascii_alphabetic).count();
    let (col, rest) = rest.split_at(letters);
    let (row_abs, row) = match rest.strip_prefix('$') {
        Some(row) => (true, row),
        None => (false, rest),
    };
    let valid = (1..=3).contains(&col.len())
        && !row.is_empty()
        && row.chars().all(|c| c.is_ascii_digit())
        && !row.starts_with('0');
    if !valid {
        return None;
    }
    let (col_abs, row_abs) = match (col_abs, row_abs) {
        (false, false) => (true, true),
        (true, true) => (false, true),
        (false, true) => (true, false),
        (true, false) => (false, false),
    };
    let dollar = |on: bool| if on { "$" } else { "" };
    let written = format!("{}{col}{}{row}", dollar(col_abs), dollar(row_abs));
    let before: String = chars[..start].iter().collect();
    let after: String = chars[end..].iter().collect();
    let cursor = start + written.chars().count();
    Some((format!("{before}{written}{after}"), cursor))
}

pub fn completion(sheet: &Sheet, at: CellAddr, typed: &str) -> Option<String> {
    if typed.is_empty() || typed.starts_with('=') || typed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let column = |row: u32| CellAddr::new(row, at.col()).ok();
    let up = (0..at.row())
        .rev()
        .map_while(|row| column(row).filter(|addr| !sheet.value(*addr).is_empty()));
    let down =
        (at.row() + 1..).map_while(|row| column(row).filter(|addr| !sheet.value(*addr).is_empty()));
    let key = fold(typed);
    let length = typed.chars().count();
    let mut found: Option<String> = None;
    for addr in up.chain(down) {
        let CellValue::Text(value) = sheet.value(addr) else {
            continue;
        };
        if value.chars().count() <= length || !fold(value).starts_with(&key) {
            continue;
        }
        match &found {
            Some(other) if fold(other) != fold(value) => return None,
            Some(_) => {}
            None => found = Some(value.clone()),
        }
    }
    found.map(|value| value.chars().skip(length).collect())
}

impl Spreadsheet {
    fn editor_id(&self, in_bar: bool) -> Option<eframe::egui::Id> {
        let edit = self.edit.as_ref()?;
        Some(if in_bar {
            eframe::egui::Id::unique(crate::app::FORMULA_BAR_ID)
        } else {
            edit.id
        })
    }

    pub fn insert_in_editor(&mut self, ctx: &eframe::egui::Context, in_bar: bool, insert: &str) {
        let Some(id) = self.editor_id(in_bar) else {
            return;
        };
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let (start, end) = editor_cursor(ctx, id, &edit.text);
        let chars: Vec<char> = edit.text.chars().collect();
        let before: String = chars[..start].iter().collect();
        let after: String = chars[end..].iter().collect();
        edit.text = format!("{before}{insert}{after}");
        edit.suggestion = None;
        edit.typed = edit.text.clone();
        let cursor = start + insert.chars().count();
        select_chars(ctx, id, cursor, cursor);
    }

    pub fn toggle_editor_reference(&mut self, ctx: &eframe::egui::Context, in_bar: bool) {
        let Some(id) = self.editor_id(in_bar) else {
            return;
        };
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        if !edit.text.starts_with('=') {
            return;
        }
        let (_, cursor) = editor_cursor(ctx, id, &edit.text);
        if let Some((text, cursor)) = toggle_reference(&edit.text, cursor) {
            edit.text = text;
            edit.point = None;
            select_chars(ctx, id, cursor, cursor);
        }
    }

    pub fn paste_special(&mut self, options: PasteOptions) {
        let Some(clip) = self.clipboard.clone().filter(|clip| !clip.cut) else {
            return;
        };
        let origin = self.sel.range().start();
        let active = self.doc.active();
        let source: HashMap<(u32, u32), &Cell> = clip
            .cells
            .iter()
            .map(|(row, col, cell)| ((*row, *col), cell))
            .collect();
        let empty = Cell::default();
        let sheet = self.sheet();
        let mut edits = Vec::new();
        for row in 0..clip.range.rows() {
            for col in 0..clip.range.cols() {
                let cell = source.get(&(row, col)).copied().unwrap_or(&empty);
                if options.skip_blanks && !cell.has_content() {
                    continue;
                }
                let (r, c) = if options.transpose {
                    (col, row)
                } else {
                    (row, col)
                };
                let Ok(to) = CellAddr::new(origin.row() + r, origin.col() + c) else {
                    continue;
                };
                let from = clip.range.start().offset(i64::from(row), i64::from(col));
                let existing = sheet.get(to).unwrap_or(&empty);
                edits.push((active, to, combine(cell, from, existing, to, options)));
            }
        }
        self.doc.edit_cells(edits);
        let (rows, cols) = if options.transpose {
            (clip.range.cols(), clip.range.rows())
        } else {
            (clip.range.rows(), clip.range.cols())
        };
        let corner = origin.offset(i64::from(rows) - 1, i64::from(cols) - 1);
        self.select(Selection {
            active: origin,
            anchor: origin,
            corner,
        });
    }

    pub fn can_paste_special(&self) -> bool {
        self.clipboard.as_ref().is_some_and(|clip| !clip.cut)
    }

    pub fn fill_down(&mut self) {
        self.fill_copy(true);
    }

    pub fn fill_right(&mut self) {
        self.fill_copy(false);
    }

    fn fill_copy(&mut self, down: bool) {
        let range = self.selected();
        let (start, end) = (range.start(), range.end());
        let single = if down {
            range.rows() == 1
        } else {
            range.cols() == 1
        };
        let source = match (single, down) {
            (true, true) if start.row() > 0 => CellRange::new(
                start.offset(-1, 0),
                CellAddr::new(start.row() - 1, end.col()).expect("row above"),
            ),
            (true, false) if start.col() > 0 => CellRange::new(
                start.offset(0, -1),
                CellAddr::new(end.row(), start.col() - 1).expect("column left"),
            ),
            (true, _) => return,
            (false, true) => CellRange::new(
                start,
                CellAddr::new(start.row(), end.col()).expect("first row"),
            ),
            (false, false) => CellRange::new(
                start,
                CellAddr::new(end.row(), start.col()).expect("first column"),
            ),
        };
        let target = range.including(source.start());
        self.clipboard = None;
        self.doc.fill(source, target, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    #[test]
    fn f4_cycles_the_reference_at_the_cursor() {
        assert_eq!(toggle_reference("=A1+B2", 3), Some(("=$A$1+B2".into(), 5)));
        assert_eq!(toggle_reference("=$A$1", 5), Some(("=A$1".into(), 4)));
        assert_eq!(toggle_reference("=A$1", 4), Some(("=$A1".into(), 4)));
        assert_eq!(toggle_reference("=$A1", 4), Some(("=A1".into(), 3)));
        assert_eq!(
            toggle_reference("=SUM(B10)", 7),
            Some(("=SUM($B$10)".into(), 10))
        );
        assert_eq!(toggle_reference("=LOG10(2)", 6), None);
        assert_eq!(toggle_reference("=1+2", 4), None);
    }

    #[test]
    fn autocomplete_finds_a_single_match_in_the_column() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("A1"), "Elma");
        sheet.set_input(addr("A2"), "Erik");
        sheet.set_input(addr("A3"), "elmas");
        assert_eq!(completion(&sheet, addr("A4"), "Er"), Some("ik".into()));
        assert_eq!(completion(&sheet, addr("A4"), "elm"), None);
        assert_eq!(completion(&sheet, addr("A4"), "Elmas"), None);
        assert_eq!(completion(&sheet, addr("A4"), "x"), None);
        assert_eq!(completion(&sheet, addr("A5"), "Er"), None);
        assert_eq!(completion(&sheet, addr("B4"), "Er"), None);
    }

    #[test]
    fn paste_special_keeps_or_replaces_the_right_parts() {
        let mut source = Cell::parse("=B1*2");
        source.value = CellValue::Number(8.0);
        let existing = Cell::parse("5");
        let (from, to) = (addr("A1"), addr("A3"));
        let values = combine(
            &source,
            from,
            &existing,
            to,
            PasteOptions::only(PasteWhat::Values),
        );
        assert_eq!(
            (values.value, values.formula),
            (CellValue::Number(8.0), None)
        );
        let formulas = combine(
            &source,
            from,
            &existing,
            to,
            PasteOptions::only(PasteWhat::Formulas),
        );
        assert_eq!(formulas.formula.as_deref(), Some("=B3*2"));
        let add = PasteOptions {
            op: PasteOp::Add,
            ..PasteOptions::only(PasteWhat::Values)
        };
        assert_eq!(
            combine(&source, from, &existing, to, add).value,
            CellValue::Number(13.0)
        );
    }
}
