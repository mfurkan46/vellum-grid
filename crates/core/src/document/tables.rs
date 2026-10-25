use std::collections::BTreeMap;

use super::Document;
use crate::formula::{self, Expr};
use crate::locale::{EN, fold};
use crate::structure::Shift;
use crate::table::{Table, TableColumn, TotalFunction, rename_column, unique_columns};
use crate::table_style::table_look;
use crate::{
    AutoFilter, Axis, Cell, CellAddr, CellRange, CellValue, DefinedName, Layout, NameError,
    is_valid_name, overlaps,
};

type Edit = (usize, CellAddr, Cell);

fn at(row: u32, col: u32) -> CellAddr {
    CellAddr::new(row, col).expect("inside the sheet")
}

fn row_band(row: u32, range: CellRange) -> CellRange {
    CellRange::new(at(row, range.start().col()), at(row, range.end().col()))
}

fn text_of(cell: Option<&Cell>, locale: &crate::Locale) -> String {
    match cell.map(|c| &c.value) {
        None | Some(CellValue::Empty) => String::new(),
        Some(CellValue::Text(text)) => text.clone(),
        Some(value) => locale.display(value),
    }
}

fn styled(cell: Option<&Cell>, value: CellValue) -> Cell {
    Cell {
        value,
        formula: None,
        style: cell.and_then(|c| c.style.clone()),
    }
}

fn sync_filter(layout: &mut Layout, before: &Table, after: &Table) {
    let had = layout
        .filter
        .as_ref()
        .is_some_and(|f| f.range == before.filter_range());
    if after.filter && (had || layout.filter.is_none()) {
        let criteria = if had {
            layout.filter.take().map(|f| f.criteria).unwrap_or_default()
        } else {
            BTreeMap::new()
        };
        layout.filter = Some(AutoFilter {
            range: after.filter_range(),
            criteria,
        });
    } else if had && !after.filter {
        layout.filter = None;
    }
}

impl Document {
    pub fn table_index_at(&self, addr: CellAddr) -> Option<usize> {
        self.sheet()
            .layout()
            .tables
            .iter()
            .position(|t| t.range.contains(addr))
    }

    fn header_names(&self, range: CellRange, headers: bool) -> Vec<String> {
        let row = range.start().row();
        let raw = (range.start().col()..=range.end().col()).map(|col| {
            if headers {
                text_of(self.sheet().get(at(row, col)), self.locale)
            } else {
                String::new()
            }
        });
        unique_columns(raw, self.locale.table_words[1])
    }

    fn header_edits(&self, row: u32, left: u32, names: &[String]) -> Vec<Edit> {
        names
            .iter()
            .enumerate()
            .filter_map(|(i, name)| {
                let addr = at(row, left + i as u32);
                let cell = self.sheet().get(addr);
                (cell.map(|c| &c.value) != Some(&CellValue::Text(name.clone()))
                    || cell.is_some_and(|c| c.formula.is_some()))
                .then(|| {
                    (
                        self.active,
                        addr,
                        styled(cell, CellValue::Text(name.clone())),
                    )
                })
            })
            .collect()
    }

    pub fn create_table(&mut self, range: CellRange, headers: bool) -> Option<usize> {
        let layout = self.sheet().layout();
        if layout.tables.iter().any(|t| overlaps(t.range, range))
            || layout.merges.iter().any(|m| overlaps(*m, range))
        {
            return None;
        }
        let since = self.undo.len();
        let mut range = range;
        if !headers {
            let band = row_band(range.start().row(), range);
            if !self.shift(Shift::cells(band, Axis::Rows, true)).ok()? {
                return None;
            }
            range = CellRange::new(range.start(), range.end().offset(1, 0));
        }
        let names = self.header_names(range, headers);
        let edits = self.header_edits(range.start().row(), range.start().col(), &names);
        let name = self.book.next_table_name(self.locale.table_words[0]);
        let mut table = Table::new(
            name,
            range,
            names.into_iter().map(TableColumn::new).collect(),
        );
        let mut layout = self.sheet().layout().clone();
        let busy = layout
            .filter
            .as_ref()
            .is_some_and(|f| !f.criteria.is_empty() && !overlaps(f.range, range));
        if busy {
            table.filter = false;
        } else {
            layout.filter = Some(AutoFilter {
                range: table.filter_range(),
                criteria: BTreeMap::new(),
            });
        }
        layout.tables.push(table);
        let index = layout.tables.len() - 1;
        let sheet = self.active;
        self.record(edits, vec![(sheet, layout)]);
        self.squash(since);
        Some(index)
    }

    fn total_cell(&self, table: &Table, col: usize) -> Cell {
        let column = &table.columns[col];
        let addr = at(
            table.range.end().row(),
            table.range.start().col() + col as u32,
        );
        let existing = self.sheet().get(addr);
        match (
            &column.label,
            column.total.formula(&table.name, &column.name),
        ) {
            (_, Some(text)) => {
                let mut cell = Cell::parse_in(&text, &EN).unwrap_or_default();
                cell.style = existing.and_then(|c| c.style.clone());
                cell
            }
            (Some(label), None) => styled(existing, CellValue::Text(label.clone())),
            (None, None) => styled(existing, CellValue::Empty),
        }
    }

    fn insert_band(&mut self, row: u32, range: CellRange) -> bool {
        let band = row_band(row, range);
        let empty = self.sheet().cells_in(band).all(|(_, c)| !c.has_content());
        empty
            || self
                .shift(Shift::cells(band, Axis::Rows, true))
                .unwrap_or(false)
    }

    pub fn update_table(&mut self, index: usize, f: impl FnOnce(&mut Table)) -> bool {
        let Some(old) = self.sheet().layout().tables.get(index).cloned() else {
            return false;
        };
        let mut wanted = old.clone();
        f(&mut wanted);
        if wanted == old {
            return false;
        }
        let since = self.undo.len();
        if wanted.totals && !old.totals && !self.insert_band(old.range.end().row() + 1, old.range) {
            return false;
        }
        let headed = wanted.header && !old.header;
        if headed {
            let top = old.range.start().row();
            let above_free = top > 0
                && self
                    .sheet()
                    .cells_in(row_band(top - 1, old.range))
                    .all(|(_, c)| !c.has_content());
            if !above_free && !self.insert_band(top, old.range) {
                return false;
            }
        }
        let current = self.sheet().layout().tables[index].clone();
        let mut table = Table {
            range: current.range,
            ..wanted
        };
        let mut edits = Vec::new();
        if table.totals != old.totals {
            if table.totals {
                table.range = CellRange::new(table.range.start(), table.range.end().offset(1, 0));
                if table
                    .columns
                    .iter()
                    .all(|c| c.total == TotalFunction::None && c.label.is_none())
                {
                    if table.columns.len() > 1 {
                        table.columns[0].label = Some(self.locale.table_words[2].to_owned());
                    }
                    if let Some(last) = table.columns.last_mut() {
                        last.total = TotalFunction::Sum;
                    }
                }
                for col in 0..table.columns.len() {
                    let addr = at(
                        table.range.end().row(),
                        table.range.start().col() + col as u32,
                    );
                    edits.push((self.active, addr, self.total_cell(&table, col)));
                }
            } else {
                let row = table.range.end().row();
                for (addr, cell) in self.sheet().cells_in(row_band(row, table.range)) {
                    edits.push((self.active, addr, styled(Some(cell), CellValue::Empty)));
                }
                table.range = CellRange::new(table.range.start(), table.range.end().offset(-1, 0));
            }
        }
        if table.header != old.header {
            if headed {
                let start = table.range.start().offset(-1, 0);
                table.range = CellRange::new(start, table.range.end());
                let names: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
                edits.extend(self.header_edits(start.row(), start.col(), &names));
            } else {
                let row = table.range.start().row();
                for (addr, cell) in self.sheet().cells_in(row_band(row, table.range)) {
                    edits.push((self.active, addr, styled(Some(cell), CellValue::Empty)));
                }
                table.range = CellRange::new(table.range.start().offset(1, 0), table.range.end());
            }
        }
        let mut layout = self.sheet().layout().clone();
        sync_filter(&mut layout, &current, &table);
        layout.tables[index] = table;
        let sheet = self.active;
        let changed = self.record(edits, vec![(sheet, layout)]);
        self.squash(since);
        changed
    }

    pub fn set_table_total(&mut self, index: usize, col: usize, total: TotalFunction) -> bool {
        let Some(mut table) = self.sheet().layout().tables.get(index).cloned() else {
            return false;
        };
        if !table.totals || col >= table.columns.len() {
            return false;
        }
        table.columns[col].total = total;
        if total != TotalFunction::None {
            table.columns[col].label = None;
        }
        let addr = at(
            table.range.end().row(),
            table.range.start().col() + col as u32,
        );
        let cell = self.total_cell(&table, col);
        let mut layout = self.sheet().layout().clone();
        layout.tables[index] = table;
        let sheet = self.active;
        self.record(vec![(sheet, addr, cell)], vec![(sheet, layout)])
    }

    fn formula_edits(&self, f: impl Fn(usize, CellAddr, &Expr) -> Option<Expr>) -> Vec<Edit> {
        let mut edits = Vec::new();
        for (sheet, source) in self.book.sheets().iter().enumerate() {
            for (addr, cell) in source.cells() {
                let Some(text) = cell.formula.as_deref() else {
                    continue;
                };
                let Ok(expr) = formula::parse(&text[1..], &EN) else {
                    continue;
                };
                if let Some(expr) = f(sheet, addr, &expr) {
                    let written = format!("={}", expr.write(&EN));
                    if written != text {
                        edits.push((
                            sheet,
                            addr,
                            Cell {
                                formula: Some(written),
                                ..cell.clone()
                            },
                        ));
                    }
                }
            }
        }
        edits
    }

    pub fn rename_table(&mut self, index: usize, name: &str) -> Result<bool, NameError> {
        let Some(old) = self.sheet().layout().tables.get(index).cloned() else {
            return Ok(false);
        };
        let name = name.trim();
        if fold(name) == fold(&old.name) {
            return Ok(false);
        }
        if !is_valid_name(name) {
            return Err(NameError::Invalid(name.to_owned()));
        }
        if self.book.find_table(name).is_some() || self.book.find_name(name, self.active).is_some()
        {
            return Err(NameError::Duplicate(name.to_owned()));
        }
        let swap = |expr: &Expr| {
            expr.map_structured(&|r| {
                let ours = r
                    .table
                    .as_deref()
                    .is_some_and(|t| fold(t) == fold(&old.name));
                ours.then(|| {
                    Expr::Structured(formula::StructuredRef {
                        table: Some(name.to_owned()),
                        ..r.clone()
                    })
                })
            })
            .map_names(&|n| (fold(n) == fold(&old.name)).then(|| Expr::Name(name.to_owned())))
        };
        let edits = self.formula_edits(|_, _, expr| Some(swap(expr)));
        let names: Vec<DefinedName> = self
            .book
            .names()
            .iter()
            .map(|defined| {
                let mut defined = defined.clone();
                if let Ok(expr) = formula::parse(&defined.formula, &EN) {
                    defined.formula = swap(&expr).write(&EN);
                }
                defined
            })
            .collect();
        let mut layout = self.sheet().layout().clone();
        layout.tables[index].name = name.to_owned();
        let sheet = self.active;
        Ok(self.commit(edits, vec![(sheet, layout)], Some(names)))
    }

    pub fn convert_table(&mut self, index: usize) -> bool {
        let Some(table) = self.sheet().layout().tables.get(index).cloned() else {
            return false;
        };
        let sheet = self.active;
        let mut edits: BTreeMap<(usize, CellAddr), Cell> = BTreeMap::new();
        let range = table.range;
        for row in range.start().row()..=range.end().row() {
            for col in range.start().col()..=range.end().col() {
                let addr = at(row, col);
                let Some(look) = table_look(&table, addr) else {
                    continue;
                };
                let mut cell = self.sheet().get(addr).cloned().unwrap_or_default();
                let mut style = cell.style().clone();
                look.apply(&mut style);
                cell.set_style(style);
                edits.insert((sheet, addr), cell);
            }
        }
        let book = &self.book;
        let expanded = self.formula_edits(|host, addr, expr| {
            let mentions = expr.structured().iter().any(|r| match &r.table {
                Some(t) => fold(t) == fold(&table.name),
                None => host == sheet && range.contains(addr),
            }) || expr.names().iter().any(|n| fold(n) == fold(&table.name));
            mentions.then(|| book.expand_table_refs(expr, host, addr, Some(&table.name)))
        });
        for (host, addr, cell) in expanded {
            match edits.get_mut(&(host, addr)) {
                Some(existing) => existing.formula = cell.formula,
                None => {
                    edits.insert((host, addr), cell);
                }
            }
        }
        let mut layout = self.sheet().layout().clone();
        if layout
            .filter
            .as_ref()
            .is_some_and(|f| f.range == table.filter_range())
        {
            layout.filter = None;
        }
        layout.tables.remove(index);
        let edits: Vec<Edit> = edits.into_iter().map(|((s, a), c)| (s, a, c)).collect();
        self.record(edits, vec![(sheet, layout)])
    }

    pub fn resize_table(&mut self, index: usize, range: CellRange) -> bool {
        let Some(old) = self.sheet().layout().tables.get(index).cloned() else {
            return false;
        };
        if range.start().row() != old.range.start().row()
            || range.rows() < 1 + u32::from(old.header) + u32::from(old.totals)
            || self
                .sheet()
                .layout()
                .tables
                .iter()
                .enumerate()
                .any(|(i, t)| i != index && overlaps(t.range, range))
        {
            return false;
        }
        let mut table = old.clone();
        table.range = range;
        let raw = (range.start().col()..=range.end().col()).map(|col| {
            old.column_at(col)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| {
                    text_of(self.sheet().get(at(range.start().row(), col)), self.locale)
                })
        });
        let names = unique_columns(raw, self.locale.table_words[1]);
        table.columns = names
            .iter()
            .zip(range.start().col()..)
            .map(|(name, col)| {
                let mut column = old
                    .column_at(col)
                    .cloned()
                    .unwrap_or_else(|| TableColumn::new(""));
                column.name = name.clone();
                column
            })
            .collect();
        let edits = if table.header {
            self.header_edits(range.start().row(), range.start().col(), &names)
        } else {
            Vec::new()
        };
        let mut layout = self.sheet().layout().clone();
        sync_filter(&mut layout, &old, &table);
        layout.tables[index] = table;
        let sheet = self.active;
        self.record(edits, vec![(sheet, layout)])
    }

    pub(super) fn table_effects(&self, edits: &mut Vec<Edit>, layouts: &mut Vec<(usize, Layout)>) {
        if edits.is_empty() {
            return;
        }
        let single = edits.len() == 1;
        let mut sheets: Vec<usize> = edits.iter().map(|e| e.0).collect();
        sheets.dedup();
        let mut extra: Vec<Edit> = Vec::new();
        let mut renames: Vec<(usize, String, CellRange, String, String)> = Vec::new();
        for sheet in sheets {
            let base = layouts
                .iter()
                .find(|(s, _)| *s == sheet)
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| self.book.sheets()[sheet].layout().clone());
            if base.tables.is_empty() {
                continue;
            }
            let source = &self.book.sheets()[sheet];
            let mut layout = base.clone();
            let mut filters = Vec::new();
            for table in &mut layout.tables {
                let before = table.clone();
                let left = table.range.start().col();
                if let Some(row) = table.header_row() {
                    for edit in edits.iter_mut() {
                        if edit.0 != sheet || edit.1.row() != row || !table.range.contains(edit.1) {
                            continue;
                        }
                        let i = (edit.1.col() - left) as usize;
                        let typed = if edit.2.formula.is_some() {
                            String::new()
                        } else {
                            text_of(Some(&edit.2), self.locale)
                        };
                        let name = if typed.trim().is_empty() {
                            format!("{}{}", self.locale.table_words[1], i + 1)
                        } else {
                            typed
                        };
                        let taken = |candidate: &str| {
                            table
                                .columns
                                .iter()
                                .enumerate()
                                .any(|(j, c)| j != i && fold(&c.name) == fold(candidate))
                        };
                        let mut unique = name.clone();
                        let mut n = 2;
                        while taken(&unique) {
                            unique = format!("{name}{n}");
                            n += 1;
                        }
                        if text_of(Some(&edit.2), self.locale) != unique || edit.2.formula.is_some()
                        {
                            edit.2 = styled(Some(&edit.2), CellValue::Text(unique.clone()));
                        }
                        let old = std::mem::replace(&mut table.columns[i].name, unique.clone());
                        if fold(&old) != fold(&unique) {
                            renames.push((sheet, table.name.clone(), table.range, old, unique));
                        }
                    }
                }
                if single
                    && !table.totals
                    && edits[0].0 == sheet
                    && edits[0].2.has_content()
                    && edits[0].1.row() == table.range.end().row() + 1
                    && (left..=table.range.end().col()).contains(&edits[0].1.col())
                {
                    let row = edits[0].1.row();
                    for col in left..=table.range.end().col() {
                        let addr = at(row, col);
                        if addr == edits[0].1 || source.get(addr).is_some_and(Cell::has_content) {
                            continue;
                        }
                        if let Some(above) = source
                            .get(at(row - 1, col))
                            .filter(|c| c.formula.is_some() && row > table.data_rows().0)
                        {
                            extra.push((sheet, addr, above.shifted(1, 0)));
                        }
                    }
                    table.range =
                        CellRange::new(table.range.start(), table.range.end().offset(1, 0));
                } else if single
                    && edits[0].0 == sheet
                    && edits[0].2.has_content()
                    && edits[0].1.col() == table.range.end().col() + 1
                    && (table.range.start().row()..=table.range.end().row())
                        .contains(&edits[0].1.row())
                    && !layout_has_table_at(&base, edits[0].1)
                {
                    let header = table.header_row();
                    let col = edits[0].1.col();
                    let typed = (header == Some(edits[0].1.row()))
                        .then(|| text_of(Some(&edits[0].2), self.locale));
                    let mut names: Vec<String> =
                        table.columns.iter().map(|c| c.name.clone()).collect();
                    names.push(typed.clone().unwrap_or_default());
                    let names = unique_columns(names, self.locale.table_words[1]);
                    let name = names.last().cloned().unwrap_or_default();
                    match (header, typed) {
                        (Some(row), None) => {
                            let addr = at(row, col);
                            extra.push((
                                sheet,
                                addr,
                                styled(source.get(addr), CellValue::Text(name.clone())),
                            ));
                        }
                        (Some(_), Some(text)) if text != name => {
                            edits[0].2 = styled(Some(&edits[0].2), CellValue::Text(name.clone()));
                        }
                        _ => {}
                    }
                    table.columns.push(TableColumn::new(name));
                    table.range =
                        CellRange::new(table.range.start(), table.range.end().offset(0, 1));
                }
                let (data_top, data_bottom) = table.data_rows();
                if single
                    && edits[0].0 == sheet
                    && edits[0].2.formula.is_some()
                    && table.range.contains(edits[0].1)
                    && (data_top..=data_bottom).contains(&edits[0].1.row())
                {
                    let (row, col) = (edits[0].1.row(), edits[0].1.col());
                    let empty = (data_top..=data_bottom)
                        .filter(|r| *r != row)
                        .all(|r| !source.get(at(r, col)).is_some_and(Cell::has_content));
                    if empty {
                        for r in (data_top..=data_bottom).filter(|r| *r != row) {
                            let delta = i64::from(r) - i64::from(row);
                            let mut cell = edits[0].2.shifted(delta, 0);
                            cell.style = source.get(at(r, col)).and_then(|c| c.style.clone());
                            extra.push((sheet, at(r, col), cell));
                        }
                    }
                }
                if *table != before {
                    filters.push((before, table.clone()));
                }
            }
            for (before, after) in filters {
                sync_filter(&mut layout, &before, &after);
            }
            if layout != base {
                match layouts.iter_mut().find(|(s, _)| *s == sheet) {
                    Some((_, existing)) => *existing = layout,
                    None => layouts.push((sheet, layout)),
                }
            }
        }
        edits.extend(extra);
        for (sheet, table, range, old, new) in renames {
            let fixed = self.formula_edits(|host, addr, expr| {
                let inside = host == sheet && range.contains(addr);
                Some(rename_column(expr, &table, inside, &old, &new))
            });
            for (host, addr, cell) in fixed {
                match edits.iter_mut().find(|e| e.0 == host && e.1 == addr) {
                    Some(existing) => existing.2.formula = cell.formula,
                    None => edits.push((host, addr, cell)),
                }
            }
        }
    }
}

fn layout_has_table_at(layout: &Layout, addr: CellAddr) -> bool {
    layout.tables.iter().any(|t| t.range.contains(addr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sheet, TR, Workbook};

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn doc() -> Document {
        let mut doc = Document::with_locale(
            Workbook::from_sheets(vec![Sheet::new("Veri")]).unwrap(),
            &TR,
        );
        for (cell, input) in [
            ("A1", "Ürün"),
            ("B1", "Adet"),
            ("C1", "Fiyat"),
            ("A2", "Kalem"),
            ("B2", "3"),
            ("C2", "2"),
            ("A3", "Defter"),
            ("B3", "5"),
            ("C3", "4"),
        ] {
            doc.enter(addr(cell), input).unwrap();
        }
        doc
    }

    fn number(doc: &Document, s: &str) -> f64 {
        match doc.sheet().value(addr(s)) {
            CellValue::Number(n) => *n,
            other => panic!("{s}: {other:?}"),
        }
    }

    #[test]
    fn tables_drive_structured_formulas_and_grow() {
        let mut doc = doc();
        let index = doc.create_table("A1:C3".parse().unwrap(), true).unwrap();
        let table = &doc.sheet().layout().tables[index];
        assert_eq!(table.name, "Tablo1");
        assert_eq!(
            doc.sheet().layout().filter.as_ref().unwrap().range,
            "A1:C3".parse().unwrap()
        );
        doc.enter(addr("D1"), "Tutar").unwrap();
        assert_eq!(
            doc.sheet().layout().tables[0].range,
            "A1:D3".parse().unwrap()
        );
        doc.enter(addr("D2"), "=[@Adet]*[@Fiyat]").unwrap();
        doc.enter(addr("D3"), "=[@Adet]*[@Fiyat]").unwrap();
        doc.enter(addr("F1"), "=TOPLA(Tablo1[Tutar])").unwrap();
        assert_eq!(number(&doc, "F1"), 26.0);
        doc.enter(addr("E1"), "Çift").unwrap();
        doc.enter(addr("E2"), "=[@Adet]*2").unwrap();
        assert_eq!(number(&doc, "E3"), 10.0);
        doc.enter(addr("B4"), "10").unwrap();
        assert_eq!(
            doc.sheet().layout().tables[0].range,
            "A1:E4".parse().unwrap()
        );
        doc.enter(addr("C4"), "1").unwrap();
        assert_eq!(number(&doc, "D4"), 10.0);
        assert_eq!(number(&doc, "F1"), 36.0);
        doc.enter(addr("B1"), "Miktar").unwrap();
        assert_eq!(doc.sheet().layout().tables[0].columns[1].name, "Miktar");
        assert_eq!(doc.input_text(addr("D2")), "=[@Miktar]*[@Fiyat]");
        doc.enter(addr("C1"), "Miktar").unwrap();
        assert_eq!(doc.input_text(addr("C1")), "Miktar2");
        doc.undo();
        assert_eq!(doc.sheet().layout().tables[0].columns[2].name, "Fiyat");
    }

    #[test]
    fn totals_rename_and_convert() {
        let mut doc = doc();
        doc.create_table("A1:C3".parse().unwrap(), true).unwrap();
        doc.enter(addr("E1"), "=TOPLA(Tablo1[Adet])").unwrap();
        assert!(doc.update_table(0, |t| t.totals = true));
        let table = doc.sheet().layout().tables[0].clone();
        assert_eq!(table.range, "A1:C4".parse().unwrap());
        assert_eq!(doc.input_text(addr("A4")), "Toplam");
        assert_eq!(number(&doc, "C4"), 6.0);
        assert!(doc.set_table_total(0, 1, TotalFunction::Average));
        assert_eq!(number(&doc, "B4"), 4.0);
        assert!(doc.rename_table(0, "Satış").unwrap());
        assert_eq!(doc.input_text(addr("E1")), "=TOPLA(Satış[Adet])");
        assert!(doc.rename_table(0, "1x").is_err());
        assert!(doc.convert_table(0));
        assert!(doc.sheet().layout().tables.is_empty());
        assert_eq!(doc.input_text(addr("E1")), "=TOPLA($B$2:$B$3)");
        assert_eq!(number(&doc, "E1"), 8.0);
        assert!(doc.sheet().get(addr("A1")).unwrap().style().bold);
        assert!(!doc.update_table(0, |t| t.totals = false));
    }

    #[test]
    fn headerless_tables_get_a_header_row() {
        let mut doc = doc();
        doc.create_table("A2:C3".parse().unwrap(), false).unwrap();
        let table = &doc.sheet().layout().tables[0];
        assert_eq!(table.range, "A2:C4".parse().unwrap());
        assert_eq!(doc.input_text(addr("A2")), "Sütun1");
        assert_eq!(doc.input_text(addr("A3")), "Kalem");
        assert_eq!(doc.input_text(addr("A1")), "Ürün");
        assert!(doc.update_table(0, |t| t.header = false));
        assert_eq!(
            doc.sheet().layout().tables[0].range,
            "A3:C4".parse().unwrap()
        );
        assert!(doc.update_table(0, |t| t.header = true));
        assert_eq!(
            doc.sheet().layout().tables[0].range,
            "A2:C4".parse().unwrap()
        );
        assert_eq!(doc.input_text(addr("B2")), "Sütun2");
    }

    #[test]
    fn inserted_rows_and_columns_reshape_tables() {
        let mut doc = doc();
        doc.create_table("A1:C3".parse().unwrap(), true).unwrap();
        doc.shift(Shift::rows(2, 1, true)).unwrap();
        assert_eq!(
            doc.sheet().layout().tables[0].range,
            "A1:C4".parse().unwrap()
        );
        doc.shift(Shift::cols(1, 1, true)).unwrap();
        let table = &doc.sheet().layout().tables[0];
        assert_eq!(table.range, "A1:D4".parse().unwrap());
        assert_eq!(table.columns[2].name, "Adet");
        assert_eq!(table.columns.len(), 4);
    }
}
