mod tables;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use crate::deps::{Dependencies, Key};
use crate::filter::{self as filters, ColumnFilter};
use crate::formula::{self, Evaluator, Expr, FormulaError};
use crate::locale::{EN, fold};
use crate::names::check_names;
use crate::outline;
use crate::tools::{self, ColumnKind, Split};
use crate::validation::{self as rules, Validation};
use crate::{
    Cell, CellAddr, CellError, CellRange, CellValue, DefinedName, HAlign, Layout, Locale,
    NameError, Sheet, Shift, ShiftError, SortError, SortKey, Style, Workbook, WorkbookError,
    column_name, name_from_label, overlaps,
    structure::{Move, inside},
};

pub const UNDO_LIMIT: usize = 100;
const LINK_COLOR: u32 = 0x0563C1;
const NAME_PREVIEW: usize = 3;
const STYLE_LIMIT: u64 = 100_000;

#[derive(Debug, Clone, PartialEq)]
struct Change {
    sheet: usize,
    addr: CellAddr,
    before: Option<Cell>,
    after: Option<Cell>,
}

#[derive(Debug, Clone, PartialEq)]
struct LayoutChange {
    sheet: usize,
    before: Layout,
    after: Layout,
}

#[derive(Debug, Clone, Default)]
struct Step {
    cells: Vec<Change>,
    layouts: Vec<LayoutChange>,
    names: Option<(Vec<DefinedName>, Vec<DefinedName>)>,
}

#[derive(Debug, Clone)]
pub struct Document {
    book: Workbook,
    active: usize,
    locale: &'static Locale,
    today: f64,
    now: f64,
    formulas: HashMap<Key, Expr>,
    volatile: HashSet<Key>,
    deps: Dependencies,
    circular: BTreeSet<Key>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    modified: bool,
    revision: u64,
    stale: bool,
    guard_off: bool,
    blocked: bool,
}

impl Document {
    pub fn new(book: Workbook) -> Self {
        Self::with_locale(book, &EN)
    }

    pub fn with_locale(book: Workbook, locale: &'static Locale) -> Self {
        let mut doc = Self {
            book,
            active: 0,
            locale,
            today: 0.0,
            now: 0.0,
            formulas: HashMap::new(),
            volatile: HashSet::new(),
            deps: Dependencies::default(),
            circular: BTreeSet::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            modified: false,
            revision: 0,
            stale: false,
            guard_off: false,
            blocked: false,
        };
        doc.rebuild();
        doc
    }

    pub fn book(&self) -> &Workbook {
        &self.book
    }

    pub fn sheet(&self) -> &Sheet {
        &self.book.sheets()[self.active]
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn set_active(&mut self, index: usize) {
        self.active = index.min(self.book.sheets().len() - 1);
    }

    pub fn locale(&self) -> &'static Locale {
        self.locale
    }

    pub fn set_today(&mut self, today: f64) {
        self.set_now(today);
    }

    pub fn tick(&mut self, now: f64) {
        self.now = now;
        self.today = now.floor();
    }

    pub fn today(&self) -> f64 {
        self.today
    }

    pub fn set_now(&mut self, now: f64) {
        self.now = now;
        self.today = now.floor();
        if self.book.manual_calculation() {
            return;
        }
        let volatile: Vec<_> = self.volatile.iter().copied().collect();
        self.recalc(volatile);
    }

    pub fn set_link(
        &mut self,
        addr: CellAddr,
        link: Option<crate::review::Link>,
        display: Option<&str>,
    ) -> bool {
        let mut layout = self.sheet().layout().clone();
        let linked = link.is_some();
        match link {
            Some(link) => layout.links.insert(addr, link),
            None => layout.links.remove(&addr),
        };
        let mut cell = self.sheet().get(addr).cloned().unwrap_or_default();
        if let Some(text) = display.filter(|text| !text.is_empty())
            && cell.formula.is_none()
        {
            cell.value = CellValue::Text(text.to_owned());
        }
        let mut style = cell.style().clone();
        style.color = linked.then_some(crate::Color(LINK_COLOR));
        style.underline = if linked {
            crate::Underline::Single
        } else {
            crate::Underline::None
        };
        cell.set_style(style);
        let edits = [(self.active, addr, cell)];
        self.record(edits, vec![(self.active, layout)])
    }

    pub fn validation_at(&self, addr: CellAddr) -> Option<&Validation> {
        self.sheet()
            .layout()
            .validations
            .iter()
            .find(|validation| validation.covers(addr))
    }

    pub fn list_items(&self, addr: CellAddr) -> Vec<String> {
        match self.validation_at(addr) {
            Some(validation) if validation.allow == rules::Allow::List => {
                rules::list_items(&self.book, self.active, validation, self.locale)
            }
            _ => Vec::new(),
        }
    }

    pub fn violated(&mut self, addr: CellAddr, cell: &Cell) -> Option<Validation> {
        if cell.formula.is_some() {
            return None;
        }
        let validation = self.validation_at(addr)?.clone();
        let sheet = self.active;
        let old = self.book.sheets()[sheet].get(addr).cloned();
        self.book.sheet_at_mut(sheet).set(addr, cell.clone());
        let valid = rules::is_valid(
            &self.book,
            sheet,
            addr,
            &cell.value,
            &validation,
            self.locale,
        );
        self.book
            .sheet_at_mut(sheet)
            .set(addr, old.unwrap_or_default());
        (!valid).then_some(validation)
    }

    pub fn set_validation(&mut self, ranges: &[CellRange], validation: Option<Validation>) -> bool {
        self.set_layout(|layout| {
            let mut kept = Vec::new();
            for mut existing in std::mem::take(&mut layout.validations) {
                for cut in ranges {
                    existing.ranges = existing
                        .ranges
                        .iter()
                        .flat_map(|range| rules::subtract(*range, *cut))
                        .collect();
                }
                if !existing.ranges.is_empty() {
                    kept.push(existing);
                }
            }
            if let Some(mut validation) = validation {
                validation.ranges = ranges.to_vec();
                kept.push(validation);
            }
            layout.validations = kept;
        })
    }

    pub fn invalid_cells(&self) -> Vec<CellAddr> {
        let sheet = self.sheet();
        let mut out = Vec::new();
        for validation in &sheet.layout().validations {
            for range in &validation.ranges {
                for (addr, cell) in sheet.cells_in(sheet.trim_to_extent(*range)) {
                    let valid = rules::is_valid(
                        &self.book,
                        self.active,
                        addr,
                        &cell.value,
                        validation,
                        self.locale,
                    );
                    if !valid && !out.contains(&addr) {
                        out.push(addr);
                    }
                }
            }
        }
        out
    }

    pub fn split_columns(
        &self,
        source: CellRange,
        split: &Split,
        kinds: &[ColumnKind],
        dest: CellAddr,
    ) -> Vec<(CellAddr, Cell)> {
        tools::text_to_columns(self.sheet(), source, split, kinds, dest, self.locale)
    }

    pub fn overwrites(&self, edits: &[(CellAddr, Cell)], source: CellRange) -> bool {
        edits.iter().any(|(addr, _)| {
            !source.contains(*addr) && self.sheet().get(*addr).is_some_and(Cell::has_content)
        })
    }

    pub fn text_to_columns(
        &mut self,
        source: CellRange,
        split: &Split,
        kinds: &[ColumnKind],
        dest: CellAddr,
    ) -> bool {
        let edits: Vec<_> = self
            .split_columns(source, split, kinds, dest)
            .into_iter()
            .map(|(addr, cell)| (addr, self.keep_style(addr, cell)))
            .collect();
        self.edit(edits)
    }

    pub fn batch(&mut self, f: impl FnOnce(&mut Self)) {
        let since = self.undo.len();
        f(self);
        self.squash(since);
    }

    fn squash(&mut self, since: usize) {
        let since = since.min(self.undo.len());
        if self.undo.len() <= since + 1 {
            return;
        }
        let mut merged = Step::default();
        for step in self.undo.drain(since..) {
            merged.cells.extend(step.cells);
            merged.layouts.extend(step.layouts);
            merged.names = match (merged.names.take(), step.names) {
                (Some((before, _)), Some((_, after))) => Some((before, after)),
                (first, second) => first.or(second),
            };
        }
        self.undo.push(merged);
    }

    fn subtotal_rows(&self, range: CellRange) -> Vec<u32> {
        let sheet = self.sheet();
        (range.start().row()..=range.end().row())
            .filter(|row| {
                (range.start().col()..=range.end().col()).any(|col| {
                    CellAddr::new(*row, col)
                        .ok()
                        .and_then(|addr| sheet.get(addr))
                        .and_then(|cell| cell.formula.as_deref())
                        .is_some_and(|formula| formula.contains("SUBTOTAL("))
                })
            })
            .collect()
    }

    pub fn remove_subtotals(&mut self, range: CellRange) -> CellRange {
        let since = self.undo.len();
        let rows = self.subtotal_rows(range);
        for row in rows.iter().rev() {
            self.shift(Shift::rows(*row, 1, false)).ok();
        }
        let end = range.end().row().saturating_sub(rows.len() as u32);
        let range = CellRange::new(
            range.start(),
            CellAddr::new(end, range.end().col()).expect("inside the sheet"),
        );
        self.set_layout(|layout| {
            layout
                .row_levels
                .retain(|row, _| *row < range.start().row() || *row > range.end().row() + 1);
            layout
                .collapsed_rows
                .retain(|row| *row < range.start().row() || *row > range.end().row());
        });
        self.squash(since);
        range
    }

    #[allow(clippy::too_many_arguments)]
    pub fn subtotal(
        &mut self,
        range: CellRange,
        group_col: u32,
        function: u8,
        columns: &[u32],
        labels: (&str, &str),
        replace: bool,
        page_breaks: bool,
    ) -> bool {
        let since = self.undo.len();
        let range = if replace {
            self.remove_subtotals(range)
        } else {
            range
        };
        let first = range.start().row() + 1;
        if first > range.end().row() {
            return false;
        }
        let mut groups: Vec<(u32, u32, String)> = Vec::new();
        for row in first..=range.end().row() {
            let label = self.filter_text(CellAddr::new(row, group_col).expect("inside the sheet"));
            match groups.last_mut() {
                Some((_, end, previous)) if fold(previous) == fold(&label) => *end = row,
                _ => groups.push((row, row, label)),
            }
        }
        for (_, end, _) in groups.iter().rev() {
            self.shift(Shift::rows(end + 1, 1, true)).ok();
        }
        let last = range.end().row() + groups.len() as u32;
        self.shift(Shift::rows(last + 1, 1, true)).ok();
        let at = |row: u32, col: u32| CellAddr::new(row, col).expect("inside the sheet");
        let bold = |cell: Cell| {
            let mut style = cell.style().clone();
            style.bold = true;
            let mut cell = cell;
            cell.set_style(style);
            cell
        };
        let formula = |from: u32, to: u32, col: u32| {
            let target = CellRange::new(at(from, col), at(to, col));
            Cell {
                formula: Some(format!("=SUBTOTAL({function},{target})")),
                ..Cell::default()
            }
        };
        let mut edits = Vec::new();
        let mut levels = Vec::new();
        let mut breaks = Vec::new();
        for (index, (start, end, label)) in groups.iter().enumerate() {
            let offset = index as u32;
            let (start, end) = (start + offset, end + offset);
            let summary = end + 1;
            levels.extend((start..=end).map(|row| (row, 2u8)));
            levels.push((summary, 1));
            breaks.push(summary + 1);
            let text = format!("{label} {}", labels.0);
            edits.push((at(summary, group_col), bold(CellValue::Text(text).into())));
            for col in columns.iter().filter(|col| **col != group_col) {
                edits.push((at(summary, *col), bold(formula(start, end, *col))));
            }
        }
        let grand = last + 1;
        edits.push((
            at(grand, group_col),
            bold(CellValue::Text(labels.1.to_owned()).into()),
        ));
        for col in columns.iter().filter(|col| **col != group_col) {
            edits.push((at(grand, *col), bold(formula(first, last, *col))));
        }
        self.edit(edits);
        self.set_layout(|layout| {
            layout.row_levels.extend(levels);
            if page_breaks {
                breaks.pop();
                layout.page.row_breaks.extend(breaks);
            }
        });
        self.squash(since);
        true
    }

    pub fn group(&mut self, rows: bool, start: u32, end: u32) -> bool {
        self.set_layout(|layout| {
            let levels = if rows {
                &mut layout.row_levels
            } else {
                &mut layout.col_levels
            };
            outline::group(levels, start, end);
        })
    }

    pub fn ungroup(&mut self, rows: bool, start: u32, end: u32) -> bool {
        let mut layout = self.sheet().layout().clone();
        let changed = if rows {
            outline::ungroup(&mut layout.row_levels, start, end)
        } else {
            outline::ungroup(&mut layout.col_levels, start, end)
        };
        changed && self.record(Vec::new(), vec![(self.active, layout)])
    }

    pub fn clear_outline(&mut self) -> bool {
        self.set_layout(|layout| {
            layout.row_levels.clear();
            layout.col_levels.clear();
            layout.collapsed_rows.clear();
            layout.collapsed_cols.clear();
        })
    }

    pub fn toggle_group(&mut self, rows: bool, group: outline::Group) -> bool {
        self.set_layout(|layout| {
            let (hidden, collapsed, levels) = if rows {
                (
                    &mut layout.hidden_rows,
                    &mut layout.collapsed_rows,
                    &layout.row_levels,
                )
            } else {
                (
                    &mut layout.hidden_cols,
                    &mut layout.collapsed_cols,
                    &layout.col_levels,
                )
            };
            if outline::is_collapsed(hidden, group) {
                outline::expand(hidden, collapsed, levels, group);
            } else {
                outline::collapse(hidden, collapsed, group);
            }
        })
    }

    pub fn show_level(&mut self, rows: bool, level: u8) -> bool {
        self.set_layout(|layout| {
            if rows {
                outline::show_level(
                    &mut layout.hidden_rows,
                    &mut layout.collapsed_rows,
                    &layout.row_levels,
                    level,
                );
            } else {
                outline::show_level(
                    &mut layout.hidden_cols,
                    &mut layout.collapsed_cols,
                    &layout.col_levels,
                    level,
                );
            }
        })
    }

    pub fn advanced_filter(
        &mut self,
        list: CellRange,
        criteria: Option<CellRange>,
        unique: bool,
        dest: Option<CellAddr>,
    ) -> usize {
        let rows = tools::advanced_rows(self.sheet(), list, criteria, unique, self.locale);
        let count = rows.len();
        let first = list.start().row() + 1;
        let Some(dest) = dest else {
            let shown: HashSet<u32> = rows.into_iter().collect();
            self.set_layout(|layout| {
                for row in first..=list.end().row() {
                    if shown.contains(&row) {
                        layout.hidden_rows.remove(&row);
                    } else {
                        layout.hidden_rows.insert(row);
                    }
                }
            });
            return count;
        };
        let sheet = self.sheet();
        let mut edits: BTreeMap<CellAddr, Cell> = BTreeMap::new();
        for row in 0..list.rows() {
            for col in 0..list.cols() {
                if let Ok(addr) = CellAddr::new(dest.row() + row, dest.col() + col) {
                    edits.insert(addr, Cell::default());
                }
            }
        }
        let sources = std::iter::once(list.start().row()).chain(rows);
        for (offset, source) in sources.enumerate() {
            for (index, col) in (list.start().col()..=list.end().col()).enumerate() {
                let (Ok(from), Ok(to)) = (
                    CellAddr::new(source, col),
                    CellAddr::new(dest.row() + offset as u32, dest.col() + index as u32),
                ) else {
                    continue;
                };
                let rows = i64::from(to.row()) - i64::from(from.row());
                let cols = i64::from(to.col()) - i64::from(from.col());
                let cell = sheet
                    .get(from)
                    .map_or_else(Cell::default, |cell| cell.shifted(rows, cols));
                edits.insert(to, cell);
            }
        }
        self.edit(edits);
        count
    }

    pub fn show_rows(&mut self, range: CellRange) -> bool {
        self.set_layout(|layout| {
            layout
                .hidden_rows
                .retain(|row| *row < range.start().row() || *row > range.end().row());
        })
    }

    pub fn remove_duplicates(
        &mut self,
        range: CellRange,
        columns: &[u32],
        header: bool,
    ) -> (usize, usize) {
        let sheet = self.sheet();
        let range = sheet.trim_to_extent(range);
        let first = range.start().row() + u32::from(header);
        if first > range.end().row() {
            return (0, 0);
        }
        let kept = tools::unique_rows(sheet, range, columns, header);
        let total = (range.end().row() - first + 1) as usize;
        let mut edits = Vec::new();
        for (index, row) in (first..=range.end().row()).enumerate() {
            let source = kept.get(index).copied();
            for col in range.start().col()..=range.end().col() {
                let to = CellAddr::new(row, col).expect("inside the range");
                let cell = source
                    .and_then(|source| {
                        let from = CellAddr::new(source, col).ok()?;
                        let shift = i64::from(row) - i64::from(source);
                        Some(sheet.get(from)?.shifted(shift, 0))
                    })
                    .unwrap_or_default();
                edits.push((to, cell));
            }
        }
        let unique = kept.len();
        self.edit(edits);
        (total - unique, unique)
    }

    pub fn precedents(&self, sheet: usize, addr: CellAddr) -> Vec<(usize, CellRange)> {
        let Some(expr) = self.formulas.get(&(sheet, addr)) else {
            return Vec::new();
        };
        let mut out: Vec<(usize, CellRange)> = Vec::new();
        for (name, range) in expr.references() {
            let index = match name {
                Some(name) => self.book.index_of(name),
                None => Some(sheet),
            };
            if let Some(index) = index
                && !out.contains(&(index, range))
            {
                out.push((index, range));
            }
        }
        out
    }

    pub fn dependents(&self, sheet: usize, addr: CellAddr) -> Vec<(usize, CellAddr)> {
        let mut out = Vec::new();
        self.deps.dependents((sheet, addr), &mut out);
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn is_manual(&self) -> bool {
        self.book.manual_calculation()
    }

    pub fn needs_calculation(&self) -> bool {
        self.stale
    }

    pub fn set_manual(&mut self, manual: bool) {
        if manual == self.is_manual() {
            return;
        }
        self.book.set_manual_calculation(manual);
        if !manual && self.stale {
            self.calculate(self.now);
        }
        self.mark_modified();
    }

    pub fn calculate(&mut self, now: f64) {
        self.now = now;
        self.today = now.floor();
        let all: Vec<Key> = self.formulas.keys().copied().collect();
        self.recalc(all);
        self.stale = false;
    }

    pub fn calculate_sheet(&mut self, now: f64, sheet: usize) {
        self.now = now;
        self.today = now.floor();
        let keys: Vec<Key> = self
            .formulas
            .keys()
            .copied()
            .filter(|(index, _)| *index == sheet)
            .collect();
        self.recalc(keys);
    }

    pub fn parse(&self, input: &str) -> Result<Cell, FormulaError> {
        let mut cell = Cell::parse_in(input, self.locale)?;
        if let Some(text) = cell.formula.as_deref()
            && let Ok(expr) = formula::parse(&text[1..], &EN)
            && !expr.names().is_empty()
        {
            let spelled = expr.map_names(&|name| {
                self.book
                    .find_name(name, self.active)
                    .map(|defined| Expr::Name(defined.name.clone()))
            });
            cell.formula = Some(format!("={}", spelled.write(&EN)));
        }
        Ok(cell)
    }

    pub fn name_formula_text(&self, formula: &str) -> String {
        match formula::parse(formula, &EN) {
            Ok(expr) => format!("={}", expr.write(self.locale)),
            Err(_) => format!("={formula}"),
        }
    }

    pub fn parse_name_formula(&self, input: &str) -> Result<String, NameError> {
        let input = input.trim();
        let body = input.strip_prefix('=').unwrap_or(input);
        let expr = formula::parse(body, self.locale).map_err(|_| NameError::Formula)?;
        Ok(expr.qualify(self.sheet().name()).write(&EN))
    }

    pub fn name_value(&self, defined: &DefinedName) -> String {
        let sheet = defined.scope.unwrap_or(self.active);
        if let Some((index, range)) = self.book.name_range(&defined.name, sheet)
            && !range.is_single()
        {
            let source = &self.book.sheets()[index];
            let (start, end) = (range.start(), range.end());
            let values: Vec<String> = (start.row()..=end.row())
                .flat_map(|row| (start.col()..=end.col()).map(move |col| (row, col)))
                .take(NAME_PREVIEW)
                .filter_map(|(row, col)| CellAddr::new(row, col).ok())
                .map(|addr| self.locale.display(source.value(addr)))
                .collect();
            let cells = u64::from(range.rows()) * u64::from(range.cols());
            let more = if cells > NAME_PREVIEW as u64 {
                format!("{}…", self.locale.list)
            } else {
                String::new()
            };
            return format!("{{{}{more}}}", values.join(&self.locale.list.to_string()));
        }
        let expr = self
            .book
            .expand_names(&Expr::Name(defined.name.clone()), sheet);
        let evaluator = Evaluator {
            book: &self.book,
            sheet,
            locale: self.locale,
            at: CellAddr::new(0, 0).expect("A1 is valid"),
            today: self.today,
            now: self.now,
        };
        self.locale.display(&evaluator.evaluate(&expr))
    }

    pub fn define_name(
        &mut self,
        replacing: Option<usize>,
        defined: DefinedName,
    ) -> Result<bool, NameError> {
        let mut names = self.book.names().to_vec();
        let old = replacing.and_then(|index| names.get(index).cloned());
        match replacing.filter(|&index| index < names.len()) {
            Some(index) => names[index] = defined.clone(),
            None => names.push(defined.clone()),
        }
        check_names(&names)?;
        let edits = match old.filter(|old| old.name != defined.name) {
            Some(old) => self.renamed_name_edits(&old, &defined.name),
            None => Vec::new(),
        };
        Ok(self.commit(edits, Vec::new(), Some(names)))
    }

    pub fn delete_names(&mut self, indices: &[usize]) -> bool {
        let names = self
            .book
            .names()
            .iter()
            .enumerate()
            .filter(|(index, _)| !indices.contains(index))
            .map(|(_, defined)| defined.clone())
            .collect();
        self.commit(Vec::new(), Vec::new(), Some(names))
    }

    pub fn create_names(&mut self, range: CellRange, edges: [bool; 4]) -> usize {
        let [top, left, bottom, right] = edges;
        let (r0, c0) = (range.start().row(), range.start().col());
        let (r1, c1) = (range.end().row(), range.end().col());
        let rows = (r0 + u32::from(top), r1.saturating_sub(u32::from(bottom)));
        let cols = (c0 + u32::from(left), c1.saturating_sub(u32::from(right)));
        if rows.0 > rows.1 || cols.0 > cols.1 {
            return 0;
        }
        let at = |row, col| CellAddr::new(row, col).expect("inside the selection");
        let mut created = Vec::new();
        let mut label = |label_at: CellAddr, from: CellAddr, to: CellAddr| {
            if let Some(name) = name_from_label(&self.filter_text(label_at)) {
                created.push((name, self.absolute_reference(from, to)));
            }
        };
        for col in cols.0..=cols.1 {
            if top {
                label(at(r0, col), at(rows.0, col), at(rows.1, col));
            }
            if bottom {
                label(at(r1, col), at(rows.0, col), at(rows.1, col));
            }
        }
        for row in rows.0..=rows.1 {
            if left {
                label(at(row, c0), at(row, cols.0), at(row, cols.1));
            }
            if right {
                label(at(row, c1), at(row, cols.0), at(row, cols.1));
            }
        }
        let mut names = self.book.names().to_vec();
        let count = created.len();
        for (name, formula) in created {
            let key = fold(&name);
            names.retain(|defined| defined.scope.is_some() || fold(&defined.name) != key);
            names.push(DefinedName::new(&name, &formula));
        }
        if check_names(&names).is_err() {
            return 0;
        }
        self.commit(Vec::new(), Vec::new(), Some(names));
        count
    }

    pub fn absolute_reference(&self, from: CellAddr, to: CellAddr) -> String {
        let cell = |addr: CellAddr| format!("${}${}", column_name(addr.col()), addr.row() + 1);
        let text = if from == to {
            cell(from)
        } else {
            format!("{}:{}", cell(from), cell(to))
        };
        formula::parse(&text, &EN)
            .expect("absolute references parse")
            .qualify(self.sheet().name())
            .write(&EN)
    }

    fn renamed_name_edits(&self, old: &DefinedName, new: &str) -> Vec<(usize, CellAddr, Cell)> {
        let key = fold(&old.name);
        let mut edits = Vec::new();
        for (index, sheet) in self.book.sheets().iter().enumerate() {
            if old.scope.is_some_and(|scope| scope != index) {
                continue;
            }
            for (addr, cell) in sheet.cells() {
                let Some(text) = cell.formula.as_deref() else {
                    continue;
                };
                let Ok(expr) = formula::parse(&text[1..], &EN) else {
                    continue;
                };
                let renamed =
                    expr.map_names(&|name| (fold(name) == key).then(|| Expr::Name(new.to_owned())));
                let written = format!("={}", renamed.write(&EN));
                if written != text {
                    let cell = Cell {
                        formula: Some(written),
                        ..cell.clone()
                    };
                    edits.push((index, addr, cell));
                }
            }
        }
        edits
    }

    fn mapped_names(&self, f: impl Fn(&Expr) -> Expr) -> Vec<DefinedName> {
        self.book
            .names()
            .iter()
            .map(|defined| match formula::parse(&defined.formula, &EN) {
                Ok(expr) => DefinedName {
                    formula: f(&expr).write(&EN),
                    ..defined.clone()
                },
                Err(_) => defined.clone(),
            })
            .collect()
    }

    pub fn styled_input(&self, addr: CellAddr, input: &str) -> Cell {
        let parsed = self
            .parse(input)
            .unwrap_or_else(|_| CellValue::Text(input.to_owned()).into());
        self.keep_style(addr, parsed)
    }

    pub fn enter(&mut self, addr: CellAddr, input: &str) -> Result<bool, FormulaError> {
        let mut cell = self.keep_style(addr, self.parse(input)?);
        if input.contains('\n') && cell.formula.is_none() && !cell.style().wrap {
            let mut style = cell.style().clone();
            style.wrap = true;
            cell.set_style(style);
        }
        Ok(self.edit([(addr, cell)]))
    }

    fn keep_style(&self, addr: CellAddr, mut cell: Cell) -> Cell {
        let existing = self.sheet().get(addr).and_then(|c| c.style.clone());
        cell.style = match (existing, cell.style.take()) {
            (Some(old), Some(new)) if old.has_general_format() => Some(Arc::new(Style {
                number_format: new.number_format.clone(),
                ..(*old).clone()
            })),
            (Some(old), _) => Some(old),
            (None, new) => new,
        };
        cell
    }

    pub fn input_text(&self, addr: CellAddr) -> String {
        self.sheet()
            .get(addr)
            .map(|cell| cell.input_text_in(self.locale))
            .unwrap_or_default()
    }

    pub fn circular(&self) -> Option<(usize, CellAddr)> {
        self.circular.first().copied()
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    pub fn mark_modified(&mut self) {
        self.modified = true;
        self.revision += 1;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn edit(&mut self, edits: impl IntoIterator<Item = (CellAddr, Cell)>) -> bool {
        let sheet = self.active;
        self.edit_cells(edits.into_iter().map(|(addr, cell)| (sheet, addr, cell)))
    }

    pub fn edit_cells(&mut self, edits: impl IntoIterator<Item = (usize, CellAddr, Cell)>) -> bool {
        self.record(edits, Vec::new())
    }

    pub fn clear(&mut self, range: CellRange) -> bool {
        self.rewrite(range, |cell| Cell {
            style: cell.style.clone(),
            ..Cell::default()
        })
    }

    pub fn clear_formats(&mut self, range: CellRange) -> bool {
        self.rewrite(range, |cell| Cell {
            style: None,
            ..cell.clone()
        })
    }

    pub fn clear_all(&mut self, range: CellRange) -> bool {
        self.rewrite(range, |_| Cell::default())
    }

    fn rewrite(&mut self, range: CellRange, f: impl Fn(&Cell) -> Cell) -> bool {
        let edits: Vec<(CellAddr, Cell)> = self
            .sheet()
            .cells_in(range)
            .map(|(addr, cell)| (addr, f(cell)))
            .collect();
        self.edit(edits)
    }

    pub fn style_cells(&mut self, range: CellRange, f: impl Fn(CellAddr, &mut Style)) -> bool {
        self.style_ranges(&[range], f)
    }

    pub fn style_ranges(&mut self, ranges: &[CellRange], f: impl Fn(CellAddr, &mut Style)) -> bool {
        let sheet = self.sheet();
        let mut edits = BTreeMap::new();
        for &range in ranges {
            let size = u64::from(range.rows()) * u64::from(range.cols());
            let target = if size <= STYLE_LIMIT {
                range
            } else {
                sheet.trim_to_extent(range)
            };
            for row in target.start().row()..=target.end().row() {
                for col in target.start().col()..=target.end().col() {
                    let addr = CellAddr::new(row, col).expect("inside the range");
                    let mut cell = sheet.get(addr).cloned().unwrap_or_default();
                    let mut style = cell.style().clone();
                    f(addr, &mut style);
                    cell.set_style(style);
                    edits.insert(addr, cell);
                }
            }
        }
        self.edit(edits)
    }

    pub fn clear_ranges(&mut self, ranges: &[CellRange]) -> bool {
        let sheet = self.sheet();
        let edits: BTreeMap<CellAddr, Cell> = ranges
            .iter()
            .flat_map(|&range| sheet.cells_in(range))
            .map(|(addr, cell)| {
                let cleared = Cell {
                    style: cell.style.clone(),
                    ..Cell::default()
                };
                (addr, cleared)
            })
            .collect();
        self.edit(edits)
    }

    pub fn move_cells(&mut self, from: CellRange, to: CellAddr, copy: bool) -> bool {
        let sheet = self.active;
        let shift = Move { from, to };
        let target = shift.target();
        if target == from {
            return false;
        }
        let rows = i64::from(to.row()) - i64::from(from.start().row());
        let cols = i64::from(to.col()) - i64::from(from.start().col());
        let source = self.sheet();
        let moved: Vec<(CellAddr, Cell)> = source
            .cells_in(from)
            .map(|(addr, cell)| (addr.offset(rows, cols), cell.clone()))
            .collect();
        let mut edits: BTreeMap<(usize, CellAddr), Cell> = BTreeMap::new();
        for (addr, _) in source.cells_in(target) {
            edits.insert((sheet, addr), Cell::default());
        }
        if !copy {
            for (addr, _) in source.cells_in(from) {
                edits.insert((sheet, addr), Cell::default());
            }
        }
        if copy {
            for (addr, cell) in moved {
                edits.insert((sheet, addr), cell.shifted(rows, cols));
            }
            let mut layout = self.sheet().layout().clone();
            layout.merges.retain(|m| !overlaps(*m, target));
            let copied: Vec<CellRange> = self
                .sheet()
                .layout()
                .merges
                .iter()
                .filter(|m| inside(**m, from))
                .map(|m| CellRange::new(m.start().offset(rows, cols), m.end().offset(rows, cols)))
                .collect();
            layout.merges.extend(copied);
            let edits = edits.into_iter().map(|((s, a), c)| (s, a, c));
            return self.record(edits, vec![(sheet, layout)]);
        }
        let name = fold(self.sheet().name());
        let rewrite = |cell: &Cell, host: usize| -> Option<Cell> {
            let text = cell.formula.as_deref()?;
            let expr = formula::parse(&text[1..], &EN).ok()?;
            let on_sheet = |other: Option<&str>| match other {
                Some(other) => fold(other) == name,
                None => host == sheet,
            };
            let written = format!("={}", shift.expr(&expr, on_sheet).write(&EN));
            (written != text).then(|| Cell {
                formula: Some(written),
                ..cell.clone()
            })
        };
        for (addr, cell) in &moved {
            let cell = rewrite(cell, sheet).unwrap_or_else(|| cell.clone());
            edits.insert((sheet, *addr), cell);
        }
        for (index, other) in self.book.sheets().iter().enumerate() {
            for (addr, cell) in other.cells() {
                let gone = index == sheet && (from.contains(addr) || target.contains(addr));
                if gone {
                    continue;
                }
                if let Some(cell) = rewrite(cell, index) {
                    edits.insert((index, addr), cell);
                }
            }
        }
        let mut layout = self.sheet().layout().clone();
        layout.merges = layout
            .merges
            .iter()
            .filter(|m| inside(**m, from) || !overlaps(**m, target))
            .map(|m| {
                if inside(*m, from) {
                    CellRange::new(m.start().offset(rows, cols), m.end().offset(rows, cols))
                } else {
                    *m
                }
            })
            .collect();
        let moved = |range: CellRange| {
            CellRange::new(
                range.start().offset(rows, cols),
                range.end().offset(rows, cols),
            )
        };
        for table in &mut layout.tables {
            if inside(table.range, from) {
                if let Some(filter) = layout
                    .filter
                    .as_mut()
                    .filter(|f| f.range == table.filter_range())
                {
                    filter.range = moved(filter.range);
                }
                table.range = moved(table.range);
            }
        }
        let edits: Vec<_> = edits.into_iter().map(|((s, a), c)| (s, a, c)).collect();
        let names = self.mapped_names(|expr| {
            shift.expr(expr, |other| other.is_some_and(|other| fold(other) == name))
        });
        self.commit(edits, vec![(sheet, layout)], Some(names))
    }

    pub fn fill(&mut self, source: CellRange, target: CellRange, copy: bool) -> bool {
        let edits = crate::fill::fill_series(self.sheet(), source, target, self.locale, copy);
        self.edit(edits)
    }

    pub fn sort(
        &mut self,
        range: CellRange,
        keys: &[SortKey],
        header: bool,
    ) -> Result<bool, SortError> {
        let sheet = self.sheet();
        if sheet.layout().merges.iter().any(|m| overlaps(*m, range)) {
            return Err(SortError::MergedCells);
        }
        if let Some(protection) = &sheet.layout().protection {
            let locked = sheet.cells_in(range).any(|(_, cell)| cell.style().locked);
            if !protection.allow.sort || locked {
                self.blocked = true;
                return Ok(false);
            }
        }
        let range = sheet.trim_to_extent(range);
        let first = range.start().row() + u32::from(header);
        if first > range.end().row() {
            return Ok(false);
        }
        let rows: Vec<u32> = (first..=range.end().row()).collect();
        let order = crate::sort::sorted_rows(sheet, &rows, keys, self.locale);
        let mut edits = Vec::new();
        for (target, source) in rows.iter().zip(order) {
            let shift = i64::from(*target) - i64::from(source);
            for col in range.start().col()..=range.end().col() {
                let from = CellAddr::new(source, col).expect("inside the range");
                let to = CellAddr::new(*target, col).expect("inside the range");
                let cell = sheet
                    .get(from)
                    .map_or_else(Cell::default, |cell| cell.shifted(shift, 0));
                edits.push((to, cell));
            }
        }
        Ok(self.edit(edits))
    }

    pub fn filter_text(&self, addr: CellAddr) -> String {
        self.sheet()
            .get(addr)
            .map(|cell| self.locale.format_value(&cell.value, cell.number_format()))
            .unwrap_or_default()
    }

    pub fn filter_values(&self, col: u32) -> Vec<String> {
        let Some(filter) = &self.sheet().layout().filter else {
            return Vec::new();
        };
        let mut rows: Vec<(CellValue, String)> = Vec::new();
        for row in filter.range.start().row() + 1..=filter.range.end().row() {
            let addr = CellAddr::new(row, col).expect("inside the filter");
            let text = self.filter_text(addr);
            if !rows.iter().any(|(_, seen)| *seen == text) {
                rows.push((self.sheet().value(addr).clone(), text));
            }
        }
        rows.sort_by(|(a, _), (b, _)| match (a.is_empty(), b.is_empty()) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => crate::compare_values(a, b, self.locale),
        });
        rows.into_iter().map(|(_, text)| text).collect()
    }

    pub fn toggle_filter(&mut self, range: CellRange) -> bool {
        let mut layout = self.sheet().layout().clone();
        match layout.filter.take() {
            Some(filter) => {
                let rows = filter.range.start().row()..=filter.range.end().row();
                layout.hidden_rows.retain(|row| !rows.contains(row));
            }
            None => {
                layout.filter = Some(crate::AutoFilter {
                    range,
                    criteria: Default::default(),
                })
            }
        }
        self.record(Vec::new(), vec![(self.active, layout)])
    }

    pub fn set_filter_column(&mut self, col: u32, shown: Option<ColumnFilter>) -> bool {
        let mut layout = self.sheet().layout().clone();
        let Some(filter) = layout.filter.as_mut() else {
            return false;
        };
        match shown {
            Some(criteria) => filter.criteria.insert(col, criteria),
            None => filter.criteria.remove(&col),
        };
        self.filter_rows(&mut layout);
        self.record(Vec::new(), vec![(self.active, layout)])
    }

    pub fn clear_filters(&mut self) -> bool {
        let mut layout = self.sheet().layout().clone();
        if let Some(filter) = layout.filter.as_mut() {
            filter.criteria.clear();
        }
        self.filter_rows(&mut layout);
        self.record(Vec::new(), vec![(self.active, layout)])
    }

    pub fn reapply_filter(&mut self) -> bool {
        let mut layout = self.sheet().layout().clone();
        self.filter_rows(&mut layout);
        self.record(Vec::new(), vec![(self.active, layout)])
    }

    fn filter_rows(&self, layout: &mut Layout) {
        let Some(filter) = &layout.filter else {
            return;
        };
        let range = filter.range;
        let rows = range.start().row() + 1..=range.end().row();
        let sheet = self.sheet();
        let contexts: BTreeMap<u32, filters::Context> = filter
            .criteria
            .iter()
            .map(|(col, criteria)| {
                let numbers: Vec<f64> = rows
                    .clone()
                    .filter_map(|row| match sheet.value(CellAddr::new(row, *col).ok()?) {
                        CellValue::Number(n) => Some(*n),
                        _ => None,
                    })
                    .collect();
                (*col, filters::context(criteria, &numbers, self.today))
            })
            .collect();
        let hidden: Vec<(u32, bool)> = rows
            .map(|row| {
                let hide = filter.criteria.iter().any(|(col, criteria)| {
                    let addr = CellAddr::new(row, *col).expect("inside the sheet");
                    let text = self.filter_text(addr);
                    let style = sheet.get(addr).map(|cell| cell.style());
                    let entry = filters::Entry {
                        value: sheet.value(addr),
                        text: &text,
                        fill: style.and_then(|style| style.fill),
                        color: style.and_then(|style| style.color),
                    };
                    !filters::matches(criteria, &entry, &contexts[col], self.locale)
                });
                (row, hide)
            })
            .collect();
        for (row, hide) in hidden {
            if hide {
                layout.hidden_rows.insert(row);
            } else {
                layout.hidden_rows.remove(&row);
            }
        }
    }

    pub fn replace_all(
        &mut self,
        query: &str,
        replacement: &str,
        options: crate::find::FindOptions,
    ) -> usize {
        let options = crate::find::FindOptions {
            values: false,
            ..options
        };
        let edits: Vec<(CellAddr, Cell)> = self
            .sheet()
            .cells()
            .filter(|(_, cell)| cell.has_content())
            .filter_map(|(addr, cell)| {
                let text = cell.input_text_in(self.locale);
                crate::find::cell_matches(&text, query, options).then(|| {
                    let replaced = crate::find::replace_text(&text, query, replacement, options);
                    (addr, self.styled_input(addr, &replaced))
                })
            })
            .collect();
        let count = edits.len();
        self.edit(edits);
        count
    }

    pub fn set_layout(&mut self, f: impl FnOnce(&mut Layout)) -> bool {
        let mut layout = self.sheet().layout().clone();
        f(&mut layout);
        self.record(Vec::new(), vec![(self.active, layout)])
    }

    pub fn shift(&mut self, shift: Shift) -> Result<bool, ShiftError> {
        if let Some(protection) = &self.sheet().layout().protection {
            let allow = protection.allow;
            let allowed = shift.span.is_none()
                && match (shift.axis, shift.insert) {
                    (crate::Axis::Rows, true) => allow.insert_rows,
                    (crate::Axis::Rows, false) => allow.delete_rows,
                    (crate::Axis::Cols, true) => allow.insert_columns,
                    (crate::Axis::Cols, false) => allow.delete_columns,
                };
            if !allowed {
                self.blocked = true;
                return Ok(false);
            }
            self.guard_off = true;
            let result = self.shift_unguarded(shift);
            self.guard_off = false;
            return result;
        }
        self.shift_unguarded(shift)
    }

    fn shift_unguarded(&mut self, shift: Shift) -> Result<bool, ShiftError> {
        let sheet = self.active;
        let target = self.sheet();
        shift.check_merges(&target.layout().merges)?;
        let moving: Vec<(CellAddr, Cell)> = target
            .cells()
            .filter(|(addr, _)| shift.moves(*addr))
            .map(|(addr, cell)| (addr, cell.clone()))
            .collect();
        if moving
            .iter()
            .any(|(addr, cell)| shift.addr(*addr).is_none() && shift.insert && cell.has_content())
        {
            return Err(ShiftError::OffSheet);
        }
        let name = fold(target.name());
        let rewrite = |cell: &Cell, host: usize| -> Option<Cell> {
            let text = cell.formula.as_deref()?;
            let expr = formula::parse(&text[1..], &EN).ok()?;
            let on_sheet = |name_: Option<&str>| match name_ {
                Some(other) => fold(other) == name,
                None => host == sheet,
            };
            let written = format!("={}", shift.expr(&expr, on_sheet).write(&EN));
            (written != text).then(|| Cell {
                formula: Some(written),
                ..cell.clone()
            })
        };
        let mut edits: BTreeMap<(usize, CellAddr), Cell> = BTreeMap::new();
        for (addr, _) in &moving {
            edits.insert((sheet, *addr), Cell::default());
        }
        for (addr, cell) in &moving {
            if let Some(to) = shift.addr(*addr) {
                let cell = rewrite(cell, sheet).unwrap_or_else(|| cell.clone());
                edits.insert((sheet, to), cell);
            }
        }
        for (index, other) in self.book.sheets().iter().enumerate() {
            for (addr, cell) in other.cells() {
                if index == sheet && shift.moves(addr) {
                    continue;
                }
                if let Some(cell) = rewrite(cell, index) {
                    edits.insert((index, addr), cell);
                }
            }
        }
        let layout = shift.layout(self.sheet().layout());
        let edits: Vec<_> = edits
            .into_iter()
            .map(|((sheet, addr), cell)| (sheet, addr, cell))
            .collect();
        let names = self.mapped_names(|expr| {
            shift.expr(expr, |other| other.is_some_and(|other| fold(other) == name))
        });
        Ok(self.commit(edits, vec![(sheet, layout)], Some(names)))
    }

    pub fn merge(&mut self, range: CellRange, center: bool) -> bool {
        let range = self.sheet().layout().expand(range);
        if range.is_single() {
            return false;
        }
        let sheet = self.active;
        let mut edits: Vec<(usize, CellAddr, Cell)> = self
            .sheet()
            .cells_in(range)
            .filter(|(addr, cell)| *addr != range.start() && cell.has_content())
            .map(|(addr, cell)| {
                let cleared = Cell {
                    style: cell.style.clone(),
                    ..Cell::default()
                };
                (sheet, addr, cleared)
            })
            .collect();
        if center {
            let mut first = self.sheet().get(range.start()).cloned().unwrap_or_default();
            let mut style = first.style().clone();
            style.h_align = HAlign::Center;
            first.set_style(style);
            edits.push((sheet, range.start(), first));
        }
        let mut layout = self.sheet().layout().clone();
        layout.merges.retain(|m| !overlaps(*m, range));
        layout.merges.push(range);
        self.record(edits, vec![(sheet, layout)])
    }

    pub fn unmerge(&mut self, range: CellRange) -> bool {
        self.set_layout(|layout| layout.merges.retain(|m| !overlaps(*m, range)))
    }

    fn record(
        &mut self,
        edits: impl IntoIterator<Item = (usize, CellAddr, Cell)>,
        layouts: Vec<(usize, Layout)>,
    ) -> bool {
        let mut edits: Vec<(usize, CellAddr, Cell)> = edits.into_iter().collect();
        let mut layouts = layouts;
        self.table_effects(&mut edits, &mut layouts);
        self.commit(edits, layouts, None)
    }

    fn permitted(&self, edits: &[(usize, CellAddr, Cell)], layouts: &[(usize, Layout)]) -> bool {
        for (sheet, addr, cell) in edits {
            let source = &self.book.sheets()[*sheet];
            let Some(protection) = &source.layout().protection else {
                continue;
            };
            let before = source.get(*addr);
            let before_style = before.map_or(&*crate::style::DEFAULT_STYLE, Cell::style);
            let content = before.map_or(cell.has_content(), |old| {
                old.formula != cell.formula || (cell.formula.is_none() && old.value != cell.value)
            });
            let mut restyled = cell.style().clone();
            if content {
                restyled.number_format = before_style.number_format.clone();
            }
            let styled = restyled != *before_style;
            if before_style.locked && (content || styled) {
                return false;
            }
            if styled && !protection.allow.format_cells {
                return false;
            }
        }
        for (sheet, after) in layouts {
            let before = self.book.sheets()[*sheet].layout();
            let Some(protection) = &before.protection else {
                continue;
            };
            let allow = protection.allow;
            let mut normal = after.clone();
            normal.freeze_rows = before.freeze_rows;
            normal.freeze_cols = before.freeze_cols;
            normal.hide_gridlines = before.hide_gridlines;
            normal.hide_headings = before.hide_headings;
            normal.ignored_errors.clone_from(&before.ignored_errors);
            if allow.format_columns {
                normal.col_widths.clone_from(&before.col_widths);
                normal.hidden_cols.clone_from(&before.hidden_cols);
            }
            if allow.format_rows {
                normal.row_heights.clone_from(&before.row_heights);
                normal.hidden_rows.clone_from(&before.hidden_rows);
            }
            let same_filter =
                before.filter.as_ref().map(|f| f.range) == after.filter.as_ref().map(|f| f.range);
            if allow.autofilter && before.filter.is_some() && same_filter {
                normal.filter.clone_from(&before.filter);
                normal.hidden_rows.clone_from(&before.hidden_rows);
            }
            if allow.edit_objects {
                normal.charts.clone_from(&before.charts);
                normal.notes.clone_from(&before.notes);
                normal.comments.clone_from(&before.comments);
            }
            if allow.insert_links {
                normal.links.clone_from(&before.links);
            }
            if normal != *before {
                return false;
            }
        }
        true
    }

    pub fn take_blocked(&mut self) -> bool {
        std::mem::take(&mut self.blocked)
    }

    pub fn is_protected(&self) -> bool {
        self.sheet().layout().protection.is_some()
    }

    pub fn is_locked(&self, addr: CellAddr) -> bool {
        self.is_protected()
            && self
                .sheet()
                .get(addr)
                .is_none_or(|cell| cell.style().locked)
    }

    pub fn hides_formula(&self, addr: CellAddr) -> bool {
        self.is_protected()
            && self
                .sheet()
                .get(addr)
                .is_some_and(|cell| cell.style().hidden)
    }

    pub fn protect_sheet(&mut self, protection: Option<crate::protect::Protection>) -> bool {
        let mut layout = self.sheet().layout().clone();
        layout.protection = protection;
        self.guard_off = true;
        let changed = self.record(Vec::new(), vec![(self.active, layout)]);
        self.guard_off = false;
        changed
    }

    pub fn unprotect_sheet(&mut self, password: &str) -> bool {
        let hash = self
            .sheet()
            .layout()
            .protection
            .as_ref()
            .and_then(|protection| protection.password.clone());
        crate::protect::verify(hash.as_ref(), password) && self.protect_sheet(None)
    }

    pub fn structure_locked(&self) -> bool {
        self.book.protection.is_some()
    }

    pub fn protect_book(&mut self, password: Option<crate::protect::PasswordHash>) {
        self.book.protection = Some(password);
        self.mark_modified();
    }

    pub fn unprotect_book(&mut self, password: &str) -> bool {
        let Some(hash) = self.book.protection.clone() else {
            return true;
        };
        if !crate::protect::verify(hash.as_ref(), password) {
            return false;
        }
        self.book.protection = None;
        self.mark_modified();
        true
    }

    fn commit(
        &mut self,
        edits: impl IntoIterator<Item = (usize, CellAddr, Cell)>,
        layouts: Vec<(usize, Layout)>,
        names: Option<Vec<DefinedName>>,
    ) -> bool {
        let edits: Vec<(usize, CellAddr, Cell)> = edits.into_iter().collect();
        if !self.guard_off && !self.permitted(&edits, &layouts) {
            self.blocked = true;
            return false;
        }
        let mut step = Step::default();
        for (sheet, addr, cell) in edits {
            let after = (!cell.is_blank()).then_some(cell);
            let before = self.book.sheets()[sheet].get(addr).cloned();
            if !same_content(before.as_ref(), after.as_ref()) {
                self.apply((sheet, addr), after.clone());
                step.cells.push(Change {
                    sheet,
                    addr,
                    before,
                    after,
                });
            }
        }
        for (sheet, after) in layouts {
            let before = self.book.sheets()[sheet].layout().clone();
            if before != after {
                *self.book.sheet_at_mut(sheet).layout_mut() = after.clone();
                step.layouts.push(LayoutChange {
                    sheet,
                    before,
                    after,
                });
            }
        }
        if let Some(after) = names.filter(|after| after.as_slice() != self.book.names()) {
            let before = std::mem::replace(&mut self.book.names, after.clone());
            step.names = Some((before, after));
        }
        if step.cells.is_empty() && step.layouts.is_empty() && step.names.is_none() {
            return false;
        }
        self.refresh(&step);
        if self.undo.len() == UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(step);
        self.redo.clear();
        self.mark_modified();
        true
    }

    pub fn undo(&mut self) -> Option<(usize, Option<CellRange>)> {
        let step = self.undo.pop()?;
        if let Some((before, _)) = &step.names {
            self.book.names = before.clone();
        }
        for change in step.layouts.iter().rev() {
            *self.book.sheet_at_mut(change.sheet).layout_mut() = change.before.clone();
        }
        for change in step.cells.iter().rev() {
            self.apply((change.sheet, change.addr), change.before.clone());
        }
        Some(self.finish_history(step, false))
    }

    pub fn redo(&mut self) -> Option<(usize, Option<CellRange>)> {
        let step = self.redo.pop()?;
        if let Some((_, after)) = &step.names {
            self.book.names = after.clone();
        }
        for change in &step.layouts {
            *self.book.sheet_at_mut(change.sheet).layout_mut() = change.after.clone();
        }
        for change in &step.cells {
            self.apply((change.sheet, change.addr), change.after.clone());
        }
        Some(self.finish_history(step, true))
    }

    fn refresh(&mut self, step: &Step) {
        let structural = step.names.is_some()
            || step
                .layouts
                .iter()
                .any(|change| change.before.tables != change.after.tables);
        if !self.is_manual() {
            if structural {
                self.rebuild();
            } else {
                self.recalc(keys(&step.cells));
            }
            return;
        }
        if structural {
            self.register_all();
        }
        for key in keys(&step.cells) {
            if self.formulas.contains_key(&key) {
                self.evaluate(key);
            }
        }
        self.stale = true;
    }

    fn finish_history(&mut self, step: Step, redone: bool) -> (usize, Option<CellRange>) {
        self.refresh(&step);
        let sheet = step
            .cells
            .first()
            .map(|c| c.sheet)
            .or_else(|| step.layouts.first().map(|l| l.sheet))
            .unwrap_or(self.active);
        let range = step
            .cells
            .iter()
            .filter(|change| change.sheet == sheet)
            .map(|change| change.addr)
            .fold(None, |range: Option<CellRange>, addr| {
                Some(range.map_or(CellRange::single(addr), |r| r.including(addr)))
            });
        if redone {
            self.undo.push(step);
        } else {
            self.redo.push(step);
        }
        self.active = sheet;
        self.mark_modified();
        (sheet, range)
    }

    pub fn add_sheet(&mut self, prefix: &str) -> usize {
        let index = self.active + 1;
        let sheet = Sheet::new(self.book.unused_name(prefix));
        self.book
            .insert_sheet(index, sheet)
            .expect("unused names are valid");
        self.active = index;
        self.restructure();
        index
    }

    pub fn remove_sheet(&mut self, index: usize) -> Result<(), WorkbookError> {
        let name = self.book.sheets()[index].name().to_owned();
        self.book.remove_at(index)?;
        self.rewrite_formulas(|expr| expr.rename_sheet(&name, None));
        self.book.names = self.mapped_names(|expr| expr.rename_sheet(&name, None));
        self.active = self.active.min(self.book.sheets().len() - 1);
        self.restructure();
        Ok(())
    }

    pub fn rename_sheet(&mut self, index: usize, name: &str) -> Result<(), WorkbookError> {
        let old = self.book.sheets()[index].name().to_owned();
        self.book.rename_sheet(index, name)?;
        self.rewrite_formulas(|expr| expr.rename_sheet(&old, Some(name)));
        self.book.names = self.mapped_names(|expr| expr.rename_sheet(&old, Some(name)));
        self.restructure();
        Ok(())
    }

    pub fn copy_sheet(&mut self, index: usize, to: usize) -> usize {
        let mut sheet = self.book.sheets()[index].clone();
        sheet.rename(self.book.copy_name(sheet.name()));
        sheet.layout_mut().sheet_hidden = false;
        let at = to.min(self.book.sheets().len());
        self.book
            .insert_sheet(at, sheet)
            .expect("copy names are valid");
        self.active = at;
        self.restructure();
        at
    }

    pub fn visible_sheets(&self) -> Vec<usize> {
        (0..self.book.sheets().len())
            .filter(|&i| !self.book.sheets()[i].layout().sheet_hidden)
            .collect()
    }

    pub fn set_sheet_hidden(&mut self, index: usize, hidden: bool) -> bool {
        let visible = self.visible_sheets();
        if hidden && (visible.len() <= 1 || !visible.contains(&index)) {
            return false;
        }
        let mut layout = self.book.sheets()[index].layout().clone();
        layout.sheet_hidden = hidden;
        let changed = self.record(Vec::new(), vec![(index, layout)]);
        if hidden && self.active == index {
            self.active = visible
                .iter()
                .copied()
                .find(|&i| i > index)
                .or_else(|| visible.iter().copied().rev().find(|&i| i < index))
                .unwrap_or(0);
        } else if !hidden {
            self.active = index;
        }
        changed
    }

    pub fn set_tab_color(&mut self, index: usize, color: Option<crate::Color>) -> bool {
        let mut layout = self.book.sheets()[index].layout().clone();
        layout.tab_color = color;
        self.record(Vec::new(), vec![(index, layout)])
    }

    pub fn move_sheet(&mut self, from: usize, to: usize) {
        let active = self.book.sheets()[self.active].name().to_owned();
        self.book.move_sheet(from, to);
        self.active = self.book.index_of(&active).unwrap_or(0);
        self.restructure();
    }

    fn rewrite_formulas(&mut self, f: impl Fn(&Expr) -> Expr) {
        for index in 0..self.book.sheets().len() {
            let sheet = self.book.sheet_at_mut(index);
            let addrs: Vec<CellAddr> = sheet
                .cells()
                .filter(|(_, cell)| cell.formula.is_some())
                .map(|(addr, _)| addr)
                .collect();
            for addr in addrs {
                let cell = sheet.cell_mut(addr).expect("formula cell exists");
                let text = cell.formula.as_deref().expect("formula cell");
                if let Ok(expr) = formula::parse(&text[1..], &EN) {
                    cell.formula = Some(format!("={}", f(&expr).write(&EN)));
                }
            }
        }
    }

    fn restructure(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.mark_modified();
        self.rebuild();
    }

    fn rebuild(&mut self) {
        let formulas = self.register_all();
        self.recalc(formulas);
        self.stale = false;
    }

    fn register_all(&mut self) -> Vec<Key> {
        self.formulas.clear();
        self.volatile.clear();
        self.deps = Dependencies::default();
        self.circular.clear();
        let formulas: Vec<Key> = self
            .book
            .sheets()
            .iter()
            .enumerate()
            .flat_map(|(i, sheet)| {
                sheet
                    .cells()
                    .filter(|(_, cell)| cell.formula.is_some())
                    .map(move |(addr, _)| (i, addr))
            })
            .collect();
        for &key in &formulas {
            self.register(key);
        }
        formulas
    }

    fn apply(&mut self, (sheet, addr): Key, cell: Option<Cell>) {
        self.book
            .sheet_at_mut(sheet)
            .set(addr, cell.unwrap_or_default());
        self.register((sheet, addr));
    }

    fn register(&mut self, key: Key) {
        self.formulas.remove(&key);
        self.volatile.remove(&key);
        self.deps.remove(key);
        let (sheet, addr) = key;
        let Some(text) = self.book.sheets()[sheet]
            .get(addr)
            .and_then(|cell| cell.formula.clone())
        else {
            return;
        };
        match formula::parse(&text[1..], &EN) {
            Ok(expr) => {
                let expr = self.book.expand_names(&expr, sheet);
                let expr = self.book.expand_tables(&expr, sheet, addr);
                let ranges = expr
                    .references()
                    .into_iter()
                    .filter_map(|(name, range)| {
                        let index = match name {
                            Some(name) => self.book.index_of(name)?,
                            None => sheet,
                        };
                        Some((index, range))
                    })
                    .collect();
                self.deps.insert(key, ranges);
                if expr.is_volatile() {
                    self.volatile.insert(key);
                }
                self.formulas.insert(key, expr);
            }
            Err(_) => self.set_result(key, CellValue::Error(CellError::Name), None),
        }
    }

    fn recalc(&mut self, changed: Vec<Key>) {
        let mut stack = changed;
        stack.extend(self.volatile.iter().copied());
        let mut seen = HashSet::new();
        let mut dirty = HashSet::new();
        let mut found = Vec::new();
        while let Some(key) = stack.pop() {
            if !seen.insert(key) {
                continue;
            }
            if self.formulas.contains_key(&key) {
                dirty.insert(key);
            }
            found.clear();
            self.deps.dependents(key, &mut found);
            stack.extend(&found);
        }

        let mut pending: HashMap<Key, usize> = dirty.iter().map(|&key| (key, 0)).collect();
        let mut edges: HashMap<Key, Vec<Key>> = HashMap::new();
        for &from in &dirty {
            found.clear();
            self.deps.dependents(from, &mut found);
            for &to in found.iter().filter(|key| dirty.contains(key)) {
                edges.entry(from).or_default().push(to);
                *pending.get_mut(&to).expect("dirty formulas are pending") += 1;
            }
        }

        let mut ready: Vec<Key> = pending
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(key, _)| *key)
            .collect();
        ready.sort_unstable_by(|a, b| b.cmp(a));
        while let Some(key) = ready.pop() {
            pending.remove(&key);
            self.evaluate(key);
            for to in edges.get(&key).into_iter().flatten() {
                if let Some(count) = pending.get_mut(to) {
                    *count -= 1;
                    if *count == 0 {
                        ready.push(*to);
                    }
                }
            }
        }

        let formulas = &self.formulas;
        self.circular
            .retain(|key| !dirty.contains(key) && formulas.contains_key(key));
        for key in pending.into_keys() {
            self.set_result(key, CellValue::Number(0.0), None);
            self.circular.insert(key);
        }
    }

    fn evaluate(&mut self, key: Key) {
        let (sheet, addr) = key;
        let expr = &self.formulas[&key];
        let cached = self.book.sheets()[sheet]
            .get(addr)
            .is_some_and(|cell| !cell.value.is_empty());
        if cached && !expr.is_supported() {
            return;
        }
        let evaluator = Evaluator {
            book: &self.book,
            sheet,
            locale: self.locale,
            at: addr,
            today: self.today,
            now: self.now,
        };
        let value = evaluator.evaluate(expr);
        let format = evaluator.infer_format(expr);
        self.set_result(key, value, format);
    }

    fn set_result(&mut self, (sheet, addr): Key, value: CellValue, format: Option<String>) {
        if let Some(cell) = self.book.sheet_at_mut(sheet).cell_mut(addr) {
            cell.value = value;
            if let Some(code) = format.filter(|_| cell.style().has_general_format()) {
                let mut style = cell.style().clone();
                style.number_format = code;
                cell.set_style(style);
            }
        }
    }
}

fn keys(changes: &[Change]) -> Vec<Key> {
    changes.iter().map(|c| (c.sheet, c.addr)).collect()
}

fn same_content(a: Option<&Cell>, b: Option<&Cell>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            a.formula == b.formula
                && a.style() == b.style()
                && (a.formula.is_some() || a.value == b.value)
        }
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TR, numfmt};

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn range(s: &str) -> CellRange {
        s.parse().unwrap()
    }

    fn input(s: &str, text: &str) -> (CellAddr, Cell) {
        (addr(s), Cell::parse(text))
    }

    fn doc() -> Document {
        Document::new(Workbook::from_sheets(vec![Sheet::new("S")]).unwrap())
    }

    fn text(doc: &Document, s: &str) -> String {
        doc.sheet()
            .get(addr(s))
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    fn value(doc: &Document, s: &str) -> CellValue {
        doc.sheet().value(addr(s)).clone()
    }

    fn format(doc: &Document, s: &str) -> String {
        doc.sheet()
            .get(addr(s))
            .map(|c| c.number_format().to_owned())
            .unwrap_or_default()
    }

    #[test]
    fn edit_then_undo_and_redo() {
        let mut doc = doc();
        assert!(doc.edit([input("A1", "1")]));
        assert!(doc.edit([input("A1", "2"), input("B2", "x")]));
        assert!(doc.is_modified());

        assert_eq!(doc.undo(), Some((0, Some(range("A1:B2")))));
        assert_eq!(text(&doc, "A1"), "1");
        assert_eq!(text(&doc, "B2"), "");

        assert_eq!(doc.undo(), Some((0, Some(range("A1")))));
        assert!(doc.sheet().is_empty());
        assert_eq!(doc.undo(), None);

        doc.redo();
        doc.redo();
        assert_eq!(text(&doc, "A1"), "2");
        assert_eq!(text(&doc, "B2"), "x");
        assert!(!doc.can_redo());
    }

    #[test]
    fn unchanged_edits_are_not_recorded() {
        let mut doc = doc();
        doc.edit([input("A1", "5"), input("A2", "=A1")]);
        assert!(!doc.edit([input("A1", "5")]));
        assert!(!doc.edit([input("A2", "=A1")]));
        assert!(!doc.edit([input("C3", "")]));
        doc.undo();
        assert!(!doc.can_undo());
    }

    #[test]
    fn new_edit_discards_redo() {
        let mut doc = doc();
        doc.edit([input("A1", "1")]);
        doc.undo();
        assert!(doc.can_redo());
        doc.edit([input("A2", "2")]);
        assert!(!doc.can_redo());
    }

    #[test]
    fn clear_range_is_one_undo_step() {
        let mut doc = doc();
        doc.edit([input("A1", "1"), input("B1", "2"), input("C5", "3")]);
        assert!(doc.clear(range("A1:B5")));
        assert_eq!(doc.sheet().len(), 1);
        doc.undo();
        assert_eq!(value(&doc, "B1"), CellValue::Number(2.0));
        assert!(!doc.clear(range("D1:E9")));
    }

    #[test]
    fn history_is_capped() {
        let mut doc = doc();
        for i in 0..UNDO_LIMIT + 5 {
            doc.edit([input("A1", &i.to_string())]);
        }
        let mut steps = 0;
        while doc.undo().is_some() {
            steps += 1;
        }
        assert_eq!(steps, UNDO_LIMIT);
        assert_eq!(text(&doc, "A1"), "4");
    }

    #[test]
    fn saving_resets_modified_flag() {
        let mut doc = doc();
        assert!(!doc.is_modified());
        doc.edit([input("A1", "1")]);
        doc.mark_saved();
        assert!(!doc.is_modified());
        doc.undo();
        assert!(doc.is_modified());
    }

    #[test]
    fn formulas_recalculate_when_inputs_change() {
        let mut doc = doc();
        doc.edit([
            input("A1", "2"),
            input("A2", "=A1*10"),
            input("A3", "=A2+A1"),
        ]);
        assert_eq!(value(&doc, "A2"), CellValue::Number(20.0));
        assert_eq!(value(&doc, "A3"), CellValue::Number(22.0));
        doc.edit([input("A1", "5")]);
        assert_eq!(value(&doc, "A3"), CellValue::Number(55.0));
        doc.undo();
        assert_eq!(value(&doc, "A3"), CellValue::Number(22.0));
        doc.redo();
        assert_eq!(value(&doc, "A3"), CellValue::Number(55.0));
    }

    #[test]
    fn ranges_and_whole_columns_are_tracked() {
        let mut doc = doc();
        doc.edit([input("C1", "=SUM(A:A)"), input("C2", "=SUM(A1:B3)")]);
        doc.edit([input("A500", "7"), input("B2", "1")]);
        assert_eq!(value(&doc, "C1"), CellValue::Number(7.0));
        assert_eq!(value(&doc, "C2"), CellValue::Number(1.0));
        doc.clear(range("A1:B1000"));
        assert_eq!(value(&doc, "C1"), CellValue::Number(0.0));
        assert_eq!(value(&doc, "C2"), CellValue::Number(0.0));
    }

    #[test]
    fn long_chains_do_not_overflow_the_stack() {
        let mut doc = doc();
        let mut edits = vec![input("A1", "1")];
        for row in 2..=20_000 {
            edits.push(input(&format!("A{row}"), &format!("=A{}+1", row - 1)));
        }
        doc.edit(edits);
        assert_eq!(value(&doc, "A20000"), CellValue::Number(20_000.0));
        doc.edit([input("A1", "0")]);
        assert_eq!(value(&doc, "A20000"), CellValue::Number(19_999.0));
    }

    #[test]
    fn circular_references_are_reported() {
        let mut doc = doc();
        doc.edit([input("A1", "=B1+1"), input("B1", "=A1+1")]);
        assert_eq!(doc.circular(), Some((0, addr("A1"))));
        assert_eq!(value(&doc, "A1"), CellValue::Number(0.0));
        doc.edit([input("B1", "5")]);
        assert_eq!(doc.circular(), None);
        assert_eq!(value(&doc, "A1"), CellValue::Number(6.0));
        doc.edit([input("C1", "=C1")]);
        assert_eq!(doc.circular(), Some((0, addr("C1"))));
    }

    #[test]
    fn loaded_formulas_are_calculated() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("A1"), "4");
        sheet.set_input(addr("B1"), "=A1^2");
        let doc = Document::new(Workbook::from_sheets(vec![sheet]).unwrap());
        assert_eq!(value(&doc, "B1"), CellValue::Number(16.0));
    }

    #[test]
    fn unsupported_functions_keep_cached_values() {
        let mut sheet = Sheet::new("S");
        sheet.set(
            addr("A1"),
            Cell {
                value: CellValue::Number(42.0),
                formula: Some("=NPV(0.1,B1:B2)".into()),
                style: None,
            },
        );
        let mut doc = Document::new(Workbook::from_sheets(vec![sheet]).unwrap());
        assert_eq!(value(&doc, "A1"), CellValue::Number(42.0));
        doc.edit([input("A2", "=NPV(B1)")]);
        assert_eq!(value(&doc, "A2"), CellValue::Error(CellError::Name));
    }

    #[test]
    fn formulas_inherit_number_formats() {
        let mut doc = doc();
        doc.edit([input("A1", "=TODAY()"), input("A2", "=A1+7")]);
        doc.set_today(46000.0);
        assert_eq!(value(&doc, "A1"), CellValue::Number(46000.0));
        assert_eq!(value(&doc, "A2"), CellValue::Number(46007.0));
        assert_eq!(format(&doc, "A2"), numfmt::DATE);
        doc.edit([input("A3", "=A2-A1")]);
        assert_eq!(format(&doc, "A3"), "General");
        doc.style_cells(range("B1"), |_, s| {
            s.number_format = numfmt::CURRENCY.into()
        });
        doc.enter(addr("B1"), "10").unwrap();
        doc.edit([input("B2", "=B1*2"), input("B3", "=SUM(B1:B2)")]);
        assert_eq!(format(&doc, "B2"), numfmt::CURRENCY);
        assert_eq!(format(&doc, "B3"), numfmt::CURRENCY);
    }

    #[test]
    fn turkish_input_is_stored_canonically() {
        let book = Workbook::from_sheets(vec![Sheet::new("Sayfa1")]).unwrap();
        let mut doc = Document::with_locale(book, &TR);
        for (a, typed) in [
            ("A1", "1.234,5"),
            ("A2", "doğru"),
            ("A3", "=EĞER(A2;A1*2;0)"),
            ("A4", "17.10.2026"),
            ("A5", "12,5%"),
        ] {
            doc.enter(addr(a), typed).unwrap();
        }
        assert_eq!(text(&doc, "A3"), "=IF(A2,A1*2,0)");
        assert_eq!(text(&doc, "A4"), "2026-10-17");
        assert_eq!(value(&doc, "A3"), CellValue::Number(2469.0));
        assert_eq!(doc.input_text(addr("A3")), "=EĞER(A2;A1*2;0)");
        assert_eq!(doc.input_text(addr("A1")), "1234,5");
        assert_eq!(doc.input_text(addr("A2")), "DOĞRU");
        assert_eq!(doc.input_text(addr("A4")), "17.10.2026");
        assert_eq!(doc.input_text(addr("A5")), "12,5%");
        assert_eq!(format(&doc, "A5"), numfmt::PERCENT_DECIMAL);
        assert!(doc.enter(addr("A6"), "=TOPLA(1+").is_err());
    }

    #[test]
    fn typing_keeps_existing_formatting() {
        let mut doc = doc();
        doc.style_cells(range("A1:A2"), |_, s| s.bold = true);
        doc.enter(addr("A1"), "5").unwrap();
        doc.enter(addr("A2"), "2026-10-17").unwrap();
        let a1 = doc.sheet().get(addr("A1")).unwrap();
        assert!(a1.style().bold);
        let a2 = doc.sheet().get(addr("A2")).unwrap();
        assert!(a2.style().bold);
        assert_eq!(a2.number_format(), numfmt::DATE);
        doc.style_cells(range("A3"), |_, s| {
            s.number_format = numfmt::THOUSANDS.into()
        });
        doc.enter(addr("A3"), "2026-10-17").unwrap();
        assert_eq!(format(&doc, "A3"), numfmt::THOUSANDS);
    }

    #[test]
    fn styles_apply_and_undo_in_one_step() {
        let mut doc = doc();
        doc.edit([input("A1", "x")]);
        assert!(doc.style_cells(range("A1:B2"), |_, s| {
            s.bold = true;
            s.fill = Some(crate::Color(0xFFFF00));
        }));
        assert_eq!(doc.sheet().len(), 4);
        assert!(!doc.style_cells(range("A1:B2"), |_, s| s.bold = true));
        doc.undo();
        assert_eq!(doc.sheet().len(), 1);
        assert!(!doc.sheet().get(addr("A1")).unwrap().style().bold);
    }

    #[test]
    fn whole_columns_are_styled_within_used_area() {
        let mut doc = doc();
        doc.edit([input("A3", "1")]);
        doc.style_cells(range("A1:A1048576"), |_, s| s.italic = true);
        assert_eq!(doc.sheet().len(), 3);
    }

    #[test]
    fn clearing_contents_keeps_formats() {
        let mut doc = doc();
        doc.edit([input("A1", "5")]);
        doc.style_cells(range("A1"), |_, s| s.bold = true);
        doc.clear(range("A1"));
        let cell = doc.sheet().get(addr("A1")).unwrap();
        assert!(cell.style().bold && !cell.has_content());
        doc.edit([input("A1", "6")]);
        doc.clear_formats(range("A1"));
        assert!(doc.sheet().get(addr("A1")).unwrap().style.is_none());
        doc.clear_all(range("A1"));
        assert!(doc.sheet().is_empty());
    }

    #[test]
    fn layout_changes_are_undoable() {
        let mut doc = doc();
        assert!(doc.set_layout(|l| {
            l.col_widths.insert(2, 20.0);
            l.hide_gridlines = true;
        }));
        assert!(!doc.set_layout(|l| l.hide_gridlines = true));
        assert_eq!(doc.undo(), Some((0, None)));
        assert_eq!(doc.sheet().layout(), &Layout::default());
        doc.redo();
        assert_eq!(doc.sheet().layout().col_width(2), 20.0);
    }

    #[test]
    fn merging_keeps_only_the_top_left_value() {
        let mut doc = doc();
        doc.edit([input("A1", "baş"), input("B1", "x"), input("A2", "=A1")]);
        assert!(doc.merge(range("A1:B2"), true));
        assert_eq!(doc.sheet().layout().merges, [range("A1:B2")]);
        assert_eq!(value(&doc, "B1"), CellValue::Empty);
        assert_eq!(
            doc.sheet().get(addr("A1")).unwrap().style().h_align,
            HAlign::Center
        );
        assert!(doc.merge(range("B2:C3"), false));
        assert_eq!(doc.sheet().layout().merges, [range("A1:C3")]);
        doc.undo();
        doc.undo();
        assert_eq!(value(&doc, "B1"), CellValue::Text("x".into()));
        assert!(doc.sheet().layout().merges.is_empty());
        doc.merge(range("A1:B1"), false);
        assert!(doc.unmerge(range("B1")));
        assert!(doc.sheet().layout().merges.is_empty());
    }

    #[test]
    fn sorting_moves_rows_and_shifts_formulas() {
        let mut doc = doc();
        doc.edit([
            input("A1", "Ad"),
            input("B1", "Puan"),
            input("A2", "Zeki"),
            input("B2", "3"),
            input("C2", "=B2*10"),
            input("A3", "Ayşe"),
            input("B3", "9"),
            input("C3", "=B3*10"),
        ]);
        let key = SortKey {
            col: 1,
            descending: true,
        };
        assert_eq!(doc.sort(range("A1:C3"), &[key], true), Ok(true));
        assert_eq!(text(&doc, "A2"), "Ayşe");
        assert_eq!(text(&doc, "A1"), "Ad");
        assert_eq!(doc.input_text(addr("C2")), "=B2*10");
        assert_eq!(value(&doc, "C2"), CellValue::Number(90.0));
        doc.undo();
        assert_eq!(text(&doc, "A2"), "Zeki");
        doc.merge(range("A1:B1"), false);
        assert_eq!(
            doc.sort(range("A1:C3"), &[key], true),
            Err(SortError::MergedCells)
        );
    }

    #[test]
    fn inserting_and_deleting_rows_moves_cells_and_formulas() {
        let mut doc = doc();
        doc.edit([
            input("A1", "1"),
            input("A2", "2"),
            input("A3", "3"),
            input("B1", "=SUM(A1:A3)"),
            input("B3", "=A3*2"),
        ]);
        doc.set_layout(|layout| {
            layout.row_heights.insert(2, 30.0);
        });
        assert_eq!(doc.shift(Shift::rows(1, 2, true)), Ok(true));
        assert_eq!(text(&doc, "A5"), "3");
        assert_eq!(doc.input_text(addr("B1")), "=SUM(A1:A5)");
        assert_eq!(doc.input_text(addr("B5")), "=A5*2");
        assert_eq!(value(&doc, "B5"), CellValue::Number(6.0));
        assert_eq!(doc.sheet().layout().row_height(4), 30.0);
        doc.edit([input("A2", "10")]);
        assert_eq!(value(&doc, "B1"), CellValue::Number(16.0));
        doc.undo();
        doc.undo();
        assert_eq!(doc.input_text(addr("B1")), "=SUM(A1:A3)");
        assert_eq!(text(&doc, "A3"), "3");
        assert_eq!(doc.sheet().layout().row_height(2), 30.0);
        assert_eq!(doc.shift(Shift::rows(2, 1, false)), Ok(true));
        assert_eq!(doc.input_text(addr("B1")), "=SUM(A1:A2)");
        assert_eq!(value(&doc, "B1"), CellValue::Number(3.0));
        assert_eq!(doc.sheet().get(addr("B3")), None);
    }

    #[test]
    fn deleting_referenced_cells_gives_ref_errors() {
        let mut doc = doc();
        doc.edit([input("A1", "5"), input("C1", "=B1+A1")]);
        assert_eq!(doc.shift(Shift::cols(1, 1, false)), Ok(true));
        assert_eq!(doc.input_text(addr("B1")), "=#REF!+A1");
        assert_eq!(value(&doc, "B1"), CellValue::Error(CellError::Ref));
    }

    #[test]
    fn shifting_cells_updates_other_sheets() {
        let mut doc = two_sheets();
        doc.set_active(1);
        let down = Shift::cells(range("A1"), crate::Axis::Rows, true);
        assert_eq!(doc.shift(down), Ok(true));
        assert_eq!(text(&doc, "A2"), "5");
        assert_eq!(text(&doc, "A3"), "10");
        doc.set_active(0);
        assert_eq!(doc.input_text(addr("A1")), "=SUM(Veri!A:A)*2");
        assert_eq!(value(&doc, "A1"), CellValue::Number(30.0));
    }

    #[test]
    fn shifts_refuse_to_split_merges_or_drop_data() {
        let mut doc = doc();
        doc.edit([input("A1048576", "x")]);
        assert_eq!(
            doc.shift(Shift::rows(0, 1, true)),
            Err(ShiftError::OffSheet)
        );
        doc.merge(range("A3:B3"), false);
        let down = Shift::cells(range("B1"), crate::Axis::Rows, true);
        assert_eq!(doc.shift(down), Err(ShiftError::MergedCells));
        assert_eq!(doc.shift(Shift::rows(0, 1, false)), Ok(true));
        assert_eq!(doc.sheet().layout().merges, vec![range("A2:B2")]);
    }

    #[test]
    fn moving_cells_updates_formulas_and_undoes() {
        let mut doc = doc();
        doc.edit([
            input("A1", "4"),
            input("A2", "=A1*2"),
            input("B1", "=SUM(A1:A2)"),
            input("D1", "old"),
        ]);
        assert!(doc.move_cells(range("A1:A2"), addr("D1"), false));
        assert_eq!(doc.sheet().get(addr("A1")), None);
        assert_eq!(doc.input_text(addr("D2")), "=D1*2");
        assert_eq!(doc.input_text(addr("B1")), "=SUM(D1:D2)");
        assert_eq!(value(&doc, "B1"), CellValue::Number(12.0));
        doc.undo();
        assert_eq!(text(&doc, "D1"), "old");
        assert_eq!(doc.input_text(addr("B1")), "=SUM(A1:A2)");
        assert!(doc.move_cells(range("A1:A2"), addr("E1"), true));
        assert_eq!(doc.input_text(addr("E2")), "=E1*2");
        assert_eq!(doc.input_text(addr("A2")), "=A1*2");
    }

    #[test]
    fn styling_and_clearing_many_ranges_is_one_step() {
        let mut doc = doc();
        doc.edit([input("A1", "1"), input("C3", "2")]);
        doc.style_ranges(&[range("A1"), range("C3")], |_, s| s.bold = true);
        assert!(doc.sheet().get(addr("C3")).unwrap().style().bold);
        doc.clear_ranges(&[range("A1"), range("C3")]);
        assert_eq!(value(&doc, "C3"), CellValue::Empty);
        doc.undo();
        assert_eq!(text(&doc, "A1"), "1");
        assert_eq!(text(&doc, "C3"), "2");
        doc.undo();
        assert!(!doc.sheet().get(addr("C3")).unwrap().style().bold);
    }

    #[test]
    fn filters_hide_rows_and_clear() {
        let mut doc = doc();
        doc.edit([
            input("A1", "Şehir"),
            input("A2", "Ankara"),
            input("A3", "İzmir"),
            input("A4", "Ankara"),
            input("B4", "5"),
        ]);
        doc.toggle_filter(range("A1:B5"));
        assert_eq!(doc.filter_values(0), ["Ankara", "İzmir", ""]);
        let shown = ["İzmir".to_owned()].into_iter().collect();
        assert!(doc.set_filter_column(0, Some(ColumnFilter::Values(shown))));
        let hidden: Vec<u32> = doc.sheet().layout().hidden_rows.iter().copied().collect();
        assert_eq!(hidden, [1, 3, 4]);
        doc.clear_filters();
        assert!(doc.sheet().layout().hidden_rows.is_empty());
        doc.undo();
        assert_eq!(doc.sheet().layout().hidden_rows.len(), 3);
        doc.toggle_filter(range("A1:B5"));
        assert!(doc.sheet().layout().filter.is_none());
        assert!(doc.sheet().layout().hidden_rows.is_empty());
    }

    #[test]
    fn subtotals_insert_rows_and_outline_in_one_step() {
        let mut doc = doc();
        doc.edit([
            input("A1", "Bölge"),
            input("B1", "Tutar"),
            input("A2", "Ankara"),
            input("B2", "10"),
            input("A3", "ankara"),
            input("B3", "20"),
            input("A4", "İzmir"),
            input("B4", "5"),
            input("C9", "aşağıda"),
        ]);
        let steps = doc.undo.len();
        assert!(doc.subtotal(
            range("A1:B4"),
            0,
            9,
            &[1],
            ("Toplam", "Genel Toplam"),
            true,
            false
        ));
        assert_eq!(doc.undo.len(), steps + 1);
        assert_eq!(text(&doc, "A4"), "Ankara Toplam");
        assert_eq!(doc.input_text(addr("B4")), "=SUBTOTAL(9,B2:B3)");
        assert_eq!(value(&doc, "B4"), CellValue::Number(30.0));
        assert_eq!(text(&doc, "A6"), "İzmir Toplam");
        assert_eq!(text(&doc, "A7"), "Genel Toplam");
        assert_eq!(value(&doc, "B7"), CellValue::Number(35.0));
        assert_eq!(text(&doc, "C12"), "aşağıda");
        let levels: Vec<(u32, u8)> = doc
            .sheet()
            .layout()
            .row_levels
            .iter()
            .map(|(row, level)| (*row, *level))
            .collect();
        assert_eq!(levels, [(1, 2), (2, 2), (3, 1), (4, 2), (5, 1)]);
        let groups = outline::groups(&doc.sheet().layout().row_levels);
        doc.toggle_group(true, groups[1]);
        assert_eq!(value(&doc, "B4"), CellValue::Number(30.0));
        doc.undo();
        let range = doc.remove_subtotals(range("A1:B7"));
        assert_eq!(range, self::range("A1:B4"));
        assert_eq!(text(&doc, "A4"), "İzmir");
        assert!(doc.sheet().layout().row_levels.is_empty());
        assert_eq!(text(&doc, "C9"), "aşağıda");
        doc.undo();
        assert_eq!(text(&doc, "A7"), "Genel Toplam");
    }

    #[test]
    fn groups_rows_and_follow_insertions() {
        let mut doc = doc();
        assert!(doc.group(true, 1, 4));
        assert!(doc.group(true, 2, 3));
        let groups = outline::groups(&doc.sheet().layout().row_levels);
        assert_eq!(groups.len(), 2);
        assert!(doc.toggle_group(true, groups[0]));
        assert_eq!(doc.sheet().layout().hidden_rows.len(), 4);
        doc.toggle_group(true, groups[0]);
        assert!(doc.sheet().layout().hidden_rows.is_empty());
        doc.show_level(true, 2);
        let hidden: Vec<u32> = doc.sheet().layout().hidden_rows.iter().copied().collect();
        assert_eq!(hidden, [2, 3]);
        doc.shift(Shift::rows(0, 1, true)).unwrap();
        let levels: Vec<u32> = doc.sheet().layout().row_levels.keys().copied().collect();
        assert_eq!(levels, [2, 3, 4, 5]);
        assert!(doc.ungroup(true, 2, 5));
        assert!(doc.ungroup(true, 3, 4));
        assert!(doc.sheet().layout().row_levels.is_empty());
        assert!(!doc.ungroup(true, 3, 4));
    }

    #[test]
    fn advanced_filter_hides_or_copies_rows() {
        let mut doc = doc();
        doc.edit([
            input("A1", "Ad"),
            input("A2", "Ali"),
            input("A3", "Veli"),
            input("A4", "Ayşe"),
            input("C1", "Ad"),
            input("C2", "A"),
        ]);
        let list = range("A1:A4");
        assert_eq!(
            doc.advanced_filter(list, Some(range("C1:C2")), false, None),
            2
        );
        let hidden: Vec<u32> = doc.sheet().layout().hidden_rows.iter().copied().collect();
        assert_eq!(hidden, [2]);
        doc.show_rows(list);
        assert!(doc.sheet().layout().hidden_rows.is_empty());
        doc.advanced_filter(list, Some(range("C1:C2")), false, Some(addr("E1")));
        assert_eq!(text(&doc, "E1"), "Ad");
        assert_eq!(text(&doc, "E2"), "Ali");
        assert_eq!(text(&doc, "E3"), "Ayşe");
        assert_eq!(doc.sheet().get(addr("E4")), None);
    }

    #[test]
    fn condition_filters_hide_rows() {
        let mut doc = doc();
        doc.edit([
            input("A1", "Tutar"),
            input("A2", "5"),
            input("A3", "50"),
            input("A4", "500"),
            input("A5", "abc"),
        ]);
        doc.toggle_filter(range("A1:A5"));
        let over = ColumnFilter::Custom {
            first: crate::filter::Condition {
                op: crate::filter::Operator::Greater,
                value: "10".into(),
            },
            second: None,
            and: true,
        };
        doc.set_filter_column(0, Some(over));
        let hidden: Vec<u32> = doc.sheet().layout().hidden_rows.iter().copied().collect();
        assert_eq!(hidden, [1, 4]);
        let top = ColumnFilter::Top {
            count: 1.0,
            percent: false,
            bottom: false,
        };
        doc.set_filter_column(0, Some(top));
        let hidden: Vec<u32> = doc.sheet().layout().hidden_rows.iter().copied().collect();
        assert_eq!(hidden, [1, 2, 4]);
    }

    #[test]
    fn replace_all_rewrites_inputs_in_one_step() {
        let book = Workbook::from_sheets(vec![Sheet::new("S")]).unwrap();
        let mut doc = Document::with_locale(book, &crate::TR);
        for (at, text) in [("A1", "elma"), ("A2", "=TOPLA(1;2)"), ("B1", "Elma suyu")] {
            doc.enter(addr(at), text).unwrap();
        }
        let options = crate::find::FindOptions::default();
        assert_eq!(doc.replace_all("elma", "armut", options), 2);
        assert_eq!(text(&doc, "B1"), "armut suyu");
        assert_eq!(doc.replace_all("TOPLA", "ORTALAMA", options), 1);
        assert_eq!(value(&doc, "A2"), CellValue::Number(1.5));
        doc.undo();
        doc.undo();
        assert_eq!(text(&doc, "A1"), "elma");
    }

    #[test]
    fn sheets_copy_hide_and_color() {
        let mut doc = two_sheets();
        let copy = doc.copy_sheet(0, 2);
        assert_eq!(doc.book().sheets()[copy].name(), "Özet (2)");
        assert_eq!(doc.active(), 2);
        assert!(doc.set_sheet_hidden(2, true));
        assert_eq!(doc.visible_sheets(), [0, 1]);
        assert_ne!(doc.active(), 2);
        assert!(doc.set_sheet_hidden(1, true));
        assert!(!doc.set_sheet_hidden(0, true));
        doc.set_tab_color(0, Some(crate::Color(0xFF0000)));
        assert_eq!(
            doc.book().sheets()[0].layout().tab_color,
            Some(crate::Color(0xFF0000))
        );
        doc.undo();
        assert_eq!(doc.book().sheets()[0].layout().tab_color, None);
    }

    fn two_sheets() -> Document {
        let mut first = Sheet::new("Özet");
        first.set_input(addr("A1"), "=SUM(Veri!A:A)*2");
        first.set_input(addr("A2"), "=Yok!A1");
        let mut data = Sheet::new("Veri");
        data.set_input(addr("A1"), "5");
        data.set_input(addr("A2"), "10");
        Document::new(Workbook::from_sheets(vec![first, data]).unwrap())
    }

    #[test]
    fn references_reach_other_sheets() {
        let mut doc = two_sheets();
        assert_eq!(value(&doc, "A1"), CellValue::Number(30.0));
        assert_eq!(value(&doc, "A2"), CellValue::Error(CellError::Ref));
        doc.edit_cells([(1, addr("A3"), Cell::parse("15"))]);
        assert_eq!(value(&doc, "A1"), CellValue::Number(60.0));
    }

    #[test]
    fn undo_switches_to_the_changed_sheet() {
        let mut doc = two_sheets();
        doc.set_active(1);
        doc.edit([input("B1", "x")]);
        doc.set_active(0);
        assert_eq!(doc.undo(), Some((1, Some(range("B1")))));
        assert_eq!(doc.active(), 1);
        assert_eq!(doc.sheet().name(), "Veri");
    }

    #[test]
    fn renaming_a_sheet_updates_formulas() {
        let mut doc = two_sheets();
        doc.rename_sheet(1, "Girdi Tablosu").unwrap();
        assert_eq!(text(&doc, "A1"), "=SUM('Girdi Tablosu'!A:A)*2");
        assert_eq!(value(&doc, "A1"), CellValue::Number(30.0));
        assert!(doc.rename_sheet(1, "özet").is_err());
        assert!(!doc.can_undo());
    }

    #[test]
    fn removing_a_sheet_turns_references_into_errors() {
        let mut doc = two_sheets();
        doc.remove_sheet(1).unwrap();
        assert_eq!(text(&doc, "A1"), "=SUM(#REF!)*2");
        assert_eq!(value(&doc, "A1"), CellValue::Error(CellError::Ref));
        assert!(doc.remove_sheet(0).is_err());
    }

    #[test]
    fn adding_and_moving_sheets_keeps_calculation_working() {
        let mut doc = two_sheets();
        assert_eq!(doc.add_sheet("Sayfa"), 1);
        assert_eq!(doc.sheet().name(), "Sayfa1");
        doc.move_sheet(2, 0);
        let names: Vec<&str> = doc.book().sheets().iter().map(Sheet::name).collect();
        assert_eq!(names, ["Veri", "Özet", "Sayfa1"]);
        assert_eq!(doc.sheet().name(), "Sayfa1");
        doc.edit_cells([(0, addr("A3"), Cell::parse("1"))]);
        assert_eq!(
            doc.book().sheets()[1].value(addr("A1")),
            &CellValue::Number(32.0)
        );
    }

    fn named() -> Document {
        let mut doc = doc();
        doc.edit([
            input("A1", "Fiyat"),
            input("A2", "10"),
            input("A3", "20"),
            input("B1", "Adet"),
            input("B2", "3"),
            input("B3", "4"),
        ]);
        let formula = doc.parse_name_formula("=$A$2:$A$3").unwrap();
        assert_eq!(formula, "S!$A$2:$A$3");
        doc.define_name(None, DefinedName::new("Fiyat", &formula))
            .unwrap();
        doc
    }

    #[test]
    fn formulas_use_defined_names() {
        let mut doc = named();
        doc.enter(addr("C1"), "=SUM(fiyat)*2").unwrap();
        assert_eq!(doc.input_text(addr("C1")), "=SUM(Fiyat)*2");
        assert_eq!(value(&doc, "C1"), CellValue::Number(60.0));
        doc.edit([input("A3", "30")]);
        assert_eq!(value(&doc, "C1"), CellValue::Number(80.0));
        doc.enter(addr("C2"), "=Oran*2").unwrap();
        assert_eq!(value(&doc, "C2"), CellValue::Error(CellError::Name));
        doc.define_name(None, DefinedName::new("Oran", "0.5"))
            .unwrap();
        assert_eq!(value(&doc, "C2"), CellValue::Number(1.0));
        assert_eq!(doc.name_value(&doc.book().names()[1]), "0.5");
        assert_eq!(doc.name_value(&doc.book().names()[0]), "{10,30}");
        doc.undo();
        assert_eq!(value(&doc, "C2"), CellValue::Error(CellError::Name));
        doc.redo();
        assert_eq!(value(&doc, "C2"), CellValue::Number(1.0));
    }

    #[test]
    fn renaming_and_deleting_names_updates_formulas() {
        let mut doc = named();
        doc.enter(addr("C1"), "=SUM(Fiyat)").unwrap();
        let mut renamed = doc.book().names()[0].clone();
        renamed.name = "Birim_Fiyat".into();
        doc.define_name(Some(0), renamed).unwrap();
        assert_eq!(doc.input_text(addr("C1")), "=SUM(Birim_Fiyat)");
        assert_eq!(value(&doc, "C1"), CellValue::Number(30.0));
        assert_eq!(
            doc.define_name(None, DefinedName::new("birim_fiyat", "1")),
            Err(NameError::Duplicate("birim_fiyat".into()))
        );
        assert!(doc.delete_names(&[0]));
        assert!(doc.book().names().is_empty());
        assert_eq!(value(&doc, "C1"), CellValue::Error(CellError::Name));
        doc.undo();
        assert_eq!(value(&doc, "C1"), CellValue::Number(30.0));
    }

    #[test]
    fn names_follow_inserted_rows_and_renamed_sheets() {
        let mut doc = named();
        doc.enter(addr("C1"), "=SUM(Fiyat)").unwrap();
        doc.shift(Shift::rows(1, 1, true)).unwrap();
        assert_eq!(doc.book().names()[0].formula, "S!$A$3:$A$4");
        assert_eq!(value(&doc, "C1"), CellValue::Number(30.0));
        doc.undo();
        assert_eq!(doc.book().names()[0].formula, "S!$A$2:$A$3");
        doc.rename_sheet(0, "Satış Verisi").unwrap();
        assert_eq!(doc.book().names()[0].formula, "'Satış Verisi'!$A$2:$A$3");
        assert_eq!(value(&doc, "C1"), CellValue::Number(30.0));
        assert_eq!(
            doc.name_formula_text(&doc.book().names()[0].formula),
            "='Satış Verisi'!$A$2:$A$3"
        );
    }

    #[test]
    fn protection_blocks_locked_cells_until_unprotected() {
        use crate::protect::{PasswordHash, Protection};
        let mut doc = doc();
        doc.edit([input("A1", "sabit"), input("B1", "1")]);
        doc.style_cells(range("B1:B3"), |_, style| style.locked = false);
        let protection = Protection {
            password: PasswordHash::of("gizli"),
            ..Protection::default()
        };
        assert!(doc.protect_sheet(Some(protection)));
        assert!(doc.is_locked(addr("A1")));
        assert!(!doc.is_locked(addr("B2")));
        assert!(!doc.edit([input("A1", "değişti")]));
        assert!(doc.take_blocked());
        assert!(!doc.take_blocked());
        assert_eq!(text(&doc, "A1"), "sabit");
        assert_eq!(doc.enter(addr("B2"), "5"), Ok(true));
        assert!(!doc.style_cells(range("B2"), |_, style| style.bold = true));
        assert!(!doc.set_layout(|layout| {
            layout.col_widths.insert(0, 20.0);
        }));
        assert_eq!(doc.shift(Shift::rows(0, 1, true)), Ok(false));
        assert!(doc.take_blocked());
        assert!(!doc.unprotect_sheet("yanlış"));
        assert!(doc.unprotect_sheet("gizli"));
        assert!(doc.edit([input("A1", "değişti")]));
        doc.protect_book(PasswordHash::of("kitap"));
        assert!(doc.structure_locked());
        assert!(!doc.unprotect_book("x"));
        assert!(doc.unprotect_book("kitap"));
    }

    #[test]
    fn links_set_text_and_style_in_one_step() {
        let mut doc = doc();
        let link = crate::review::Link {
            target: "https://ornek.com".into(),
            tooltip: String::new(),
        };
        let steps = doc.undo.len();
        assert!(doc.set_link(addr("B2"), Some(link), Some("Örnek")));
        assert_eq!(doc.undo.len(), steps + 1);
        assert_eq!(text(&doc, "B2"), "Örnek");
        let style = doc.sheet().get(addr("B2")).unwrap().style().clone();
        assert_eq!(style.underline, crate::Underline::Single);
        assert_eq!(style.color, Some(crate::Color(LINK_COLOR)));
        assert!(doc.sheet().layout().links.contains_key(&addr("B2")));
        doc.set_link(addr("B2"), None, None);
        assert!(doc.sheet().layout().links.is_empty());
        assert_eq!(
            doc.sheet().get(addr("B2")).unwrap().style().underline,
            crate::Underline::None
        );
        assert_eq!(text(&doc, "B2"), "Örnek");
        doc.undo();
        doc.undo();
        assert_eq!(doc.sheet().get(addr("B2")), None);
    }

    #[test]
    fn validations_check_input_and_follow_rows() {
        let mut doc = doc();
        doc.edit([input("E1", "Evet"), input("E2", "Hayır")]);
        let list = Validation {
            allow: rules::Allow::List,
            formula1: "$E$1:$E$2".into(),
            ..Validation::new(Vec::new())
        };
        assert!(doc.set_validation(&[range("A1:A5")], Some(list)));
        assert_eq!(doc.list_items(addr("A3")), ["Evet", "Hayır"]);
        assert!(doc.violated(addr("A1"), &Cell::parse("evet")).is_none());
        assert!(doc.violated(addr("A1"), &Cell::parse("belki")).is_some());
        assert!(doc.violated(addr("B1"), &Cell::parse("belki")).is_none());
        assert_eq!(doc.sheet().get(addr("A1")), None);
        doc.edit([input("A2", "olmaz"), input("A3", "Evet")]);
        assert_eq!(doc.invalid_cells(), [addr("A2")]);
        doc.set_validation(&[range("A2")], None);
        let ranges = doc.sheet().layout().validations[0].ranges.clone();
        assert_eq!(ranges, [range("A1"), range("A3:A5")]);
        doc.shift(Shift::rows(0, 2, true)).unwrap();
        let ranges = doc.sheet().layout().validations[0].ranges.clone();
        assert_eq!(ranges, [range("A3"), range("A5:A7")]);
        assert_eq!(doc.sheet().layout().validations[0].formula1, "$E$3:$E$4");
        doc.undo();
        doc.undo();
        assert_eq!(doc.sheet().layout().validations[0].ranges, [range("A1:A5")]);
    }

    #[test]
    fn splits_text_into_columns_and_removes_duplicates() {
        let mut doc =
            Document::with_locale(Workbook::from_sheets(vec![Sheet::new("S")]).unwrap(), &TR);
        doc.edit([
            input("A1", "Ali;10"),
            input("A2", "Veli;20"),
            input("A3", "ali;10"),
            input("C1", "dolu"),
        ]);
        let split = Split::Delimited {
            delimiters: crate::tools::Delimiters {
                semicolon: true,
                ..Default::default()
            },
            merge: false,
            qualifier: Some('"'),
        };
        let source = range("A1:A3");
        let edits = doc.split_columns(source, &split, &[], addr("A1"));
        assert!(!doc.overwrites(&edits, source));
        let elsewhere = doc.split_columns(source, &split, &[], addr("B1"));
        assert!(doc.overwrites(&elsewhere, source));
        assert!(doc.text_to_columns(source, &split, &[], addr("A1")));
        assert_eq!(text(&doc, "A2"), "Veli");
        assert_eq!(value(&doc, "B2"), CellValue::Number(20.0));
        assert_eq!(
            doc.remove_duplicates(range("A1:B3"), &[0, 1], false),
            (1, 2)
        );
        assert_eq!(text(&doc, "A2"), "Veli");
        assert_eq!(doc.sheet().get(addr("A3")), None);
        assert_eq!(text(&doc, "C1"), "dolu");
        doc.undo();
        assert_eq!(text(&doc, "A3"), "ali");
    }

    #[test]
    fn lists_precedents_and_dependents() {
        let mut doc = two_sheets();
        doc.set_active(0);
        doc.edit([input("B1", "=A1+A1+SUM(Veri!A1:A2)")]);
        assert_eq!(
            doc.precedents(0, addr("B1")),
            [(0, range("A1")), (1, range("A1:A2"))]
        );
        assert_eq!(doc.precedents(0, addr("C9")), []);
        assert_eq!(
            doc.dependents(1, addr("A2")),
            [(0, addr("A1")), (0, addr("B1"))]
        );
        assert_eq!(doc.dependents(0, addr("A1")), [(0, addr("B1"))]);
    }

    #[test]
    fn manual_calculation_waits_for_calculate() {
        let mut doc = doc();
        doc.edit([input("A1", "1"), input("B1", "=A1*2")]);
        doc.set_manual(true);
        assert!(doc.is_manual());
        doc.edit([input("A1", "5")]);
        assert_eq!(value(&doc, "B1"), CellValue::Number(2.0));
        assert!(doc.needs_calculation());
        doc.edit([input("C1", "=A1+1")]);
        assert_eq!(value(&doc, "C1"), CellValue::Number(6.0));
        doc.calculate(0.0);
        assert_eq!(value(&doc, "B1"), CellValue::Number(10.0));
        assert!(!doc.needs_calculation());
        doc.edit([input("A1", "7")]);
        doc.set_manual(false);
        assert_eq!(value(&doc, "B1"), CellValue::Number(14.0));
        assert!(!doc.needs_calculation());
    }

    #[test]
    fn creates_names_from_selection_labels() {
        let mut doc = named();
        assert_eq!(
            doc.create_names(range("A1:B3"), [true, false, false, false]),
            2
        );
        let names: Vec<_> = doc
            .book()
            .names()
            .iter()
            .map(|defined| (defined.name.as_str(), defined.formula.as_str()))
            .collect();
        assert_eq!(names, [("Fiyat", "S!$A$2:$A$3"), ("Adet", "S!$B$2:$B$3")]);
        doc.enter(addr("C1"), "=SUMPRODUCT(Fiyat,Adet)").unwrap();
        assert_eq!(value(&doc, "C1"), CellValue::Number(110.0));
        assert_eq!(
            doc.create_names(range("A1:A1"), [true, false, false, false]),
            0
        );
    }
}
