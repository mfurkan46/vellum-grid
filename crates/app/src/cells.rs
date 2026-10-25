use vellum_grid_core::{Axis, CellAddr, CellRange, Shift, ShiftError};

use crate::app::{Selection, Spreadsheet};
use crate::dialogs::Dialog;
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftChoice {
    Across,
    Along,
    Rows,
    Cols,
}

impl ShiftChoice {
    pub const ALL: [Self; 4] = [Self::Across, Self::Along, Self::Rows, Self::Cols];

    pub fn label(self, insert: bool) -> &'static str {
        match (self, insert) {
            (Self::Across, true) => text::shift_right(),
            (Self::Across, false) => text::shift_left(),
            (Self::Along, true) => text::shift_down(),
            (Self::Along, false) => text::shift_up(),
            (Self::Rows, _) => text::entire_row(),
            (Self::Cols, _) => text::entire_column(),
        }
    }
}

impl Spreadsheet {
    pub fn finish_move(&mut self, to: CellAddr, copy: bool) {
        let from = self.selected();
        if to == from.start() {
            return;
        }
        let rows = i64::from(from.rows()) - 1;
        let cols = i64::from(from.cols()) - 1;
        let target = CellRange::new(to, to.offset(rows, cols));
        let occupied = self
            .sheet()
            .cells_in(target)
            .any(|(addr, cell)| cell.has_content() && (copy || !from.contains(addr)));
        if occupied {
            self.dialog = Some(Dialog::ReplaceData { from, to, copy });
        } else {
            self.move_now(from, to, copy);
        }
    }

    pub fn move_now(&mut self, from: CellRange, to: CellAddr, copy: bool) {
        self.clipboard = None;
        self.doc.move_cells(from, to, copy);
        let rows = i64::from(to.row()) - i64::from(from.start().row());
        let cols = i64::from(to.col()) - i64::from(from.start().col());
        let active = if from.contains(self.sel.active) {
            self.sel.active.offset(rows, cols)
        } else {
            to
        };
        self.select(Selection {
            active,
            anchor: to,
            corner: from.end().offset(rows, cols),
        });
    }

    pub fn insert_cells(&mut self) {
        self.shift_or_ask(true);
    }

    pub fn delete_cells(&mut self) {
        self.shift_or_ask(false);
    }

    fn shift_or_ask(&mut self, insert: bool) {
        let range = self.selected();
        if range.is_full_rows() {
            self.shift_selection(ShiftChoice::Rows, insert);
        } else if range.is_full_cols() {
            self.shift_selection(ShiftChoice::Cols, insert);
        } else {
            self.dialog = Some(Dialog::Shift {
                insert,
                choice: ShiftChoice::Along,
            });
        }
    }

    pub fn shift_selection(&mut self, choice: ShiftChoice, insert: bool) {
        let range = self.selected();
        let shift = match choice {
            ShiftChoice::Across => Shift::cells(range, Axis::Cols, insert),
            ShiftChoice::Along => Shift::cells(range, Axis::Rows, insert),
            ShiftChoice::Rows => Shift::rows(range.start().row(), range.rows(), insert),
            ShiftChoice::Cols => Shift::cols(range.start().col(), range.cols(), insert),
        };
        self.apply_shift(shift);
    }

    fn apply_shift(&mut self, shift: Shift) {
        self.clipboard = None;
        self.object = None;
        if let Err(error) = self.doc.shift(shift) {
            let message = match error {
                ShiftError::MergedCells => text::shift_merged(),
                ShiftError::OffSheet => text::shift_off_sheet(),
            };
            self.dialog = Some(Dialog::Error {
                title: text::app_name(),
                message: message.to_owned(),
            });
        }
    }
}
