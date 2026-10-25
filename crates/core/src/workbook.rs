use std::fmt;

use crate::{DefinedName, Sheet, fold};

const MAX_SHEET_NAME_LEN: usize = 31;
const FORBIDDEN_NAME_CHARS: [char; 7] = [':', '\\', '/', '?', '*', '[', ']'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkbookError {
    InvalidSheetName(String),
    DuplicateSheetName(String),
    SheetNotFound(String),
    LastSheet,
}

impl fmt::Display for WorkbookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSheetName(name) => write!(f, "invalid sheet name '{name}'"),
            Self::DuplicateSheetName(name) => write!(f, "a sheet named '{name}' already exists"),
            Self::SheetNotFound(name) => write!(f, "no sheet named '{name}'"),
            Self::LastSheet => write!(f, "a workbook must contain at least one sheet"),
        }
    }
}

impl std::error::Error for WorkbookError {}

#[derive(Debug, Clone, PartialEq)]
pub struct Workbook {
    sheets: Vec<Sheet>,
    pub(crate) names: Vec<DefinedName>,
    manual: bool,
    pub protection: Option<Option<crate::protect::PasswordHash>>,
}

impl Default for Workbook {
    fn default() -> Self {
        Self {
            sheets: vec![Sheet::new("Sheet1")],
            names: Vec::new(),
            manual: false,
            protection: None,
        }
    }
}

impl Workbook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_sheets(sheets: Vec<Sheet>) -> Result<Self, WorkbookError> {
        let mut book = Self {
            sheets: Vec::new(),
            names: Vec::new(),
            manual: false,
            protection: None,
        };
        for sheet in sheets {
            book.add_sheet(sheet)?;
        }
        if book.sheets.is_empty() {
            return Err(WorkbookError::LastSheet);
        }
        Ok(book)
    }

    pub fn manual_calculation(&self) -> bool {
        self.manual
    }

    pub fn set_manual_calculation(&mut self, manual: bool) {
        self.manual = manual;
    }

    pub fn sheets(&self) -> &[Sheet] {
        &self.sheets
    }

    pub(crate) fn sheet_at_mut(&mut self, index: usize) -> &mut Sheet {
        &mut self.sheets[index]
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.position(name)
    }

    pub fn sheet(&self, name: &str) -> Option<&Sheet> {
        self.position(name).map(|i| &self.sheets[i])
    }

    pub fn sheet_mut(&mut self, name: &str) -> Option<&mut Sheet> {
        self.position(name).map(|i| &mut self.sheets[i])
    }

    pub fn add_sheet(&mut self, sheet: Sheet) -> Result<&mut Sheet, WorkbookError> {
        if !is_valid_sheet_name(sheet.name()) {
            return Err(WorkbookError::InvalidSheetName(sheet.name().to_owned()));
        }
        if self.position(sheet.name()).is_some() {
            return Err(WorkbookError::DuplicateSheetName(sheet.name().to_owned()));
        }
        self.sheets.push(sheet);
        Ok(self.sheets.last_mut().expect("sheet was just pushed"))
    }

    pub fn remove_sheet(&mut self, name: &str) -> Result<Sheet, WorkbookError> {
        let index = self
            .position(name)
            .ok_or_else(|| WorkbookError::SheetNotFound(name.to_owned()))?;
        self.remove_at(index)
    }

    pub fn insert_sheet(&mut self, index: usize, sheet: Sheet) -> Result<(), WorkbookError> {
        self.check_name(sheet.name(), None)?;
        let index = index.min(self.sheets.len());
        self.sheets.insert(index, sheet);
        for name in &mut self.names {
            if let Some(scope) = name.scope.as_mut().filter(|scope| **scope >= index) {
                *scope += 1;
            }
        }
        Ok(())
    }

    pub fn rename_sheet(&mut self, index: usize, name: &str) -> Result<(), WorkbookError> {
        self.check_name(name, Some(index))?;
        self.sheets[index].rename(name.to_owned());
        Ok(())
    }

    pub fn remove_at(&mut self, index: usize) -> Result<Sheet, WorkbookError> {
        if self.sheets.len() == 1 {
            return Err(WorkbookError::LastSheet);
        }
        self.names.retain(|name| name.scope != Some(index));
        for name in &mut self.names {
            if let Some(scope) = name.scope.as_mut().filter(|scope| **scope > index) {
                *scope -= 1;
            }
        }
        Ok(self.sheets.remove(index))
    }

    pub fn move_sheet(&mut self, from: usize, to: usize) {
        let sheet = self.sheets.remove(from);
        let to = to.min(self.sheets.len());
        self.sheets.insert(to, sheet);
        let mut order: Vec<usize> = (0..self.sheets.len()).collect();
        let moved = order.remove(from);
        order.insert(to, moved);
        for name in &mut self.names {
            if let Some(scope) = name.scope.as_mut() {
                *scope = order.iter().position(|old| old == scope).unwrap_or(*scope);
            }
        }
    }

    pub fn copy_name(&self, base: &str) -> String {
        (2..)
            .map(|n| {
                let suffix = format!(" ({n})");
                let room = MAX_SHEET_NAME_LEN - suffix.chars().count();
                let stem: String = base.chars().take(room).collect();
                format!("{stem}{suffix}")
            })
            .find(|name| self.position(name).is_none())
            .expect("an unused name exists")
    }

    pub fn unused_name(&self, prefix: &str) -> String {
        (1..)
            .map(|n| format!("{prefix}{n}"))
            .find(|name| self.position(name).is_none())
            .expect("an unused name exists")
    }

    fn check_name(&self, name: &str, except: Option<usize>) -> Result<(), WorkbookError> {
        if !is_valid_sheet_name(name) {
            return Err(WorkbookError::InvalidSheetName(name.to_owned()));
        }
        if self.position(name).is_some_and(|i| Some(i) != except) {
            return Err(WorkbookError::DuplicateSheetName(name.to_owned()));
        }
        Ok(())
    }

    fn position(&self, name: &str) -> Option<usize> {
        let name = fold(name);
        self.sheets.iter().position(|s| fold(s.name()) == name)
    }
}

pub fn sheet_name_from(text: &str) -> Option<String> {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if FORBIDDEN_NAME_CHARS.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name: String = cleaned
        .trim_matches('\'')
        .chars()
        .take(MAX_SHEET_NAME_LEN)
        .collect();
    is_valid_sheet_name(&name).then_some(name)
}

fn is_valid_sheet_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= MAX_SHEET_NAME_LEN
        && !name.contains(FORBIDDEN_NAME_CHARS)
        && !name.starts_with('\'')
        && !name.ends_with('\'')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CellAddr, CellValue};

    #[test]
    fn new_workbook_has_one_sheet() {
        let wb = Workbook::new();
        assert_eq!(wb.sheets().len(), 1);
        assert_eq!(wb.sheets()[0].name(), "Sheet1");
    }

    #[test]
    fn sheets_hold_independent_cells() {
        let mut wb = Workbook::new();
        wb.add_sheet(Sheet::new("Gelir")).unwrap();
        let a1: CellAddr = "A1".parse().unwrap();

        wb.sheet_mut("Sheet1")
            .unwrap()
            .set(a1, CellValue::Number(1.0));
        wb.sheet_mut("Gelir")
            .unwrap()
            .set(a1, CellValue::Number(2.0));

        assert_eq!(
            wb.sheet("Sheet1").unwrap().value(a1),
            &CellValue::Number(1.0)
        );
        assert_eq!(
            wb.sheet("Gelir").unwrap().value(a1),
            &CellValue::Number(2.0)
        );
    }

    #[test]
    fn sheet_lookup_ignores_case() {
        let mut wb = Workbook::new();
        wb.add_sheet(Sheet::new("Özet")).unwrap();
        assert!(wb.sheet("özet").is_some());
        assert!(wb.sheet("SHEET1").is_some());
        assert!(wb.sheet("Yok").is_none());
    }

    #[test]
    fn builds_from_sheets() {
        let wb = Workbook::from_sheets(vec![Sheet::new("A"), Sheet::new("B")]).unwrap();
        assert_eq!(wb.index_of("b"), Some(1));
        assert_eq!(
            Workbook::from_sheets(vec![Sheet::new("A"), Sheet::new("a")]),
            Err(WorkbookError::DuplicateSheetName("a".into()))
        );
        assert_eq!(
            Workbook::from_sheets(Vec::new()),
            Err(WorkbookError::LastSheet)
        );
    }

    #[test]
    fn inserts_renames_moves_and_removes_by_index() {
        let mut wb = Workbook::from_sheets(vec![Sheet::new("Sayfa1")]).unwrap();
        let name = wb.unused_name("Sayfa");
        assert_eq!(name, "Sayfa2");
        wb.insert_sheet(1, Sheet::new(name)).unwrap();
        assert_eq!(
            wb.insert_sheet(0, Sheet::new("sayfa1")),
            Err(WorkbookError::DuplicateSheetName("sayfa1".into()))
        );
        wb.rename_sheet(0, "SAYFA1").unwrap();
        assert_eq!(
            wb.rename_sheet(0, "sayfa2"),
            Err(WorkbookError::DuplicateSheetName("sayfa2".into()))
        );
        wb.move_sheet(0, 1);
        let names: Vec<&str> = wb.sheets().iter().map(Sheet::name).collect();
        assert_eq!(names, ["Sayfa2", "SAYFA1"]);
        assert_eq!(wb.remove_at(0).unwrap().name(), "Sayfa2");
        assert_eq!(wb.remove_at(0), Err(WorkbookError::LastSheet));
    }

    #[test]
    fn sanitizes_file_names_into_sheet_names() {
        assert_eq!(
            sheet_name_from("rapor[2026]").as_deref(),
            Some("rapor_2026_")
        );
        assert_eq!(sheet_name_from("'tırnak'").as_deref(), Some("tırnak"));
        assert_eq!(sheet_name_from(&"x".repeat(40)).map(|n| n.len()), Some(31));
        assert_eq!(sheet_name_from("''"), None);
    }

    #[test]
    fn rejects_duplicate_names() {
        let mut wb = Workbook::new();
        assert_eq!(
            wb.add_sheet(Sheet::new("sheet1")),
            Err(WorkbookError::DuplicateSheetName("sheet1".into()))
        );
    }

    #[test]
    fn rejects_invalid_names() {
        let mut wb = Workbook::new();
        let too_long = "a".repeat(MAX_SHEET_NAME_LEN + 1);
        for name in [
            "", "a/b", "a:b", "a?", "*", "[x]", "a\\b", "'quoted", "end'", &too_long,
        ] {
            assert_eq!(
                wb.add_sheet(Sheet::new(name)),
                Err(WorkbookError::InvalidSheetName(name.into())),
                "{name}"
            );
        }
        assert!(
            wb.add_sheet(Sheet::new("ş".repeat(MAX_SHEET_NAME_LEN)))
                .is_ok()
        );
        assert!(wb.add_sheet(Sheet::new("it's")).is_ok());
    }

    #[test]
    fn removes_sheets_but_keeps_the_last() {
        let mut wb = Workbook::new();
        wb.add_sheet(Sheet::new("Two")).unwrap();
        assert_eq!(wb.remove_sheet("sheet1").unwrap().name(), "Sheet1");
        assert_eq!(wb.remove_sheet("Two"), Err(WorkbookError::LastSheet));
        assert_eq!(
            wb.remove_sheet("Missing"),
            Err(WorkbookError::SheetNotFound("Missing".into()))
        );
        assert_eq!(wb.sheets().len(), 1);
    }
}
