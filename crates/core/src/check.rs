use crate::formula::{self, Expr};
use crate::locale::EN;
use crate::{CellAddr, CellError, CellRange, CellValue, Locale, Sheet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    ErrorValue(CellError),
    NumberAsText,
    Inconsistent { from: CellAddr },
}

impl Issue {
    pub fn marked(self) -> bool {
        !matches!(self, Self::ErrorValue(_))
    }
}

fn parsed(sheet: &Sheet, addr: CellAddr) -> Option<Expr> {
    let text = sheet.get(addr)?.formula.as_deref()?;
    formula::parse(&text[1..], &EN).ok()
}

fn written(expr: &Expr) -> String {
    expr.write(&EN)
}

fn inconsistent(sheet: &Sheet, addr: CellAddr, own: &Expr) -> Option<CellAddr> {
    let own = written(own);
    for (rows, cols) in [(1i64, 0i64), (0, 1)] {
        let before = addr.offset(-rows, -cols);
        let after = addr.offset(rows, cols);
        if before == addr || after == addr {
            continue;
        }
        let (Some(first), Some(last)) = (parsed(sheet, before), parsed(sheet, after)) else {
            continue;
        };
        let pattern = written(&first.shifted(2 * rows, 2 * cols));
        if pattern == written(&last) && written(&first.shifted(rows, cols)) != own {
            return Some(before);
        }
    }
    None
}

pub fn cell_issue(sheet: &Sheet, addr: CellAddr, locale: &Locale) -> Option<Issue> {
    if sheet.layout().ignored_errors.contains(&addr) {
        return None;
    }
    let cell = sheet.get(addr)?;
    match (&cell.formula, &cell.value) {
        (Some(_), CellValue::Error(error)) => Some(Issue::ErrorValue(*error)),
        (Some(_), _) => {
            let own = parsed(sheet, addr)?;
            inconsistent(sheet, addr, &own).map(|from| Issue::Inconsistent { from })
        }
        (None, CellValue::Text(text)) => {
            let text = text.trim();
            let number = !text.is_empty()
                && (locale.parse_number(text).is_some() || EN.parse_number(text).is_some());
            number.then_some(Issue::NumberAsText)
        }
        _ => None,
    }
}

pub fn issues(sheet: &Sheet, range: CellRange, locale: &Locale) -> Vec<(CellAddr, Issue)> {
    sheet
        .cells_in(range)
        .filter_map(|(addr, _)| cell_issue(sheet, addr, locale).map(|issue| (addr, issue)))
        .collect()
}

pub fn all_issues(sheet: &Sheet, locale: &Locale) -> Vec<(CellAddr, Issue)> {
    let mut found: Vec<_> = sheet
        .cells()
        .filter_map(|(addr, _)| cell_issue(sheet, addr, locale).map(|issue| (addr, issue)))
        .collect();
    found.sort_by_key(|(addr, _)| (addr.row(), addr.col()));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, TR, Workbook};

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn doc(cells: &[(&str, &str)]) -> Document {
        let mut sheet = Sheet::new("S");
        for (at, input) in cells {
            sheet.set_input(addr(at), input);
        }
        Document::new(Workbook::from_sheets(vec![sheet]).unwrap())
    }

    #[test]
    fn finds_errors_text_numbers_and_odd_formulas() {
        let doc = doc(&[
            ("A1", "1"),
            ("A2", "'25"),
            ("A3", "metin"),
            ("B1", "=A1*2"),
            ("B2", "=A2*3"),
            ("B3", "=A3*2"),
            ("C1", "=1/0"),
        ]);
        let sheet = doc.sheet();
        assert_eq!(cell_issue(sheet, addr("A1"), &TR), None);
        assert_eq!(
            cell_issue(sheet, addr("A2"), &TR),
            Some(Issue::NumberAsText)
        );
        assert_eq!(cell_issue(sheet, addr("A3"), &TR), None);
        assert_eq!(
            cell_issue(sheet, addr("B2"), &TR),
            Some(Issue::Inconsistent { from: addr("B1") })
        );
        assert_eq!(
            cell_issue(sheet, addr("C1"), &TR),
            Some(Issue::ErrorValue(CellError::Div0))
        );
        let all: Vec<_> = all_issues(sheet, &TR)
            .into_iter()
            .map(|(at, _)| at.to_string())
            .collect();
        assert_eq!(all, ["C1", "A2", "B2", "B3"]);
    }

    #[test]
    fn consistent_columns_and_ignored_cells_are_clean() {
        let mut doc = doc(&[
            ("A1", "1"),
            ("A2", "2"),
            ("A3", "3"),
            ("B1", "=A1*2"),
            ("B2", "=A2*2"),
            ("B3", "=A3*2"),
            ("C1", "'7"),
        ]);
        assert_eq!(issues(doc.sheet(), "A1:C3".parse().unwrap(), &TR).len(), 1);
        doc.set_layout(|layout| {
            layout.ignored_errors.insert(addr("C1"));
        });
        assert!(issues(doc.sheet(), "A1:C3".parse().unwrap(), &TR).is_empty());
    }
}
