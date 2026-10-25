use std::collections::{HashMap, HashSet};

use crate::{CellAddr, CellRange};

const WIDE: u32 = 64;

pub(crate) type Key = (usize, CellAddr);
type ColumnSpan = (u32, u32, Key);

#[derive(Debug, Clone, Default)]
pub(crate) struct Dependencies {
    cells: HashMap<Key, HashSet<Key>>,
    columns: HashMap<(usize, u32), Vec<ColumnSpan>>,
    wide: Vec<(usize, CellRange, Key)>,
    precedents: HashMap<Key, Vec<(usize, CellRange)>>,
}

impl Dependencies {
    pub(crate) fn insert(&mut self, formula: Key, ranges: Vec<(usize, CellRange)>) {
        self.remove(formula);
        for &(sheet, range) in &ranges {
            let (start, end) = (range.start(), range.end());
            if range.is_single() {
                self.cells
                    .entry((sheet, start))
                    .or_default()
                    .insert(formula);
            } else if range.cols() <= WIDE {
                for col in start.col()..=end.col() {
                    self.columns.entry((sheet, col)).or_default().push((
                        start.row(),
                        end.row(),
                        formula,
                    ));
                }
            } else {
                self.wide.push((sheet, range, formula));
            }
        }
        self.precedents.insert(formula, ranges);
    }

    pub(crate) fn remove(&mut self, formula: Key) {
        let Some(ranges) = self.precedents.remove(&formula) else {
            return;
        };
        for (sheet, range) in ranges {
            if range.is_single() {
                let key = (sheet, range.start());
                if let Some(set) = self.cells.get_mut(&key) {
                    set.remove(&formula);
                    if set.is_empty() {
                        self.cells.remove(&key);
                    }
                }
            } else if range.cols() <= WIDE {
                for col in range.start().col()..=range.end().col() {
                    if let Some(list) = self.columns.get_mut(&(sheet, col)) {
                        list.retain(|(_, _, f)| *f != formula);
                    }
                }
            } else {
                self.wide.retain(|(_, _, f)| *f != formula);
            }
        }
    }

    pub(crate) fn dependents(&self, (sheet, addr): Key, out: &mut Vec<Key>) {
        if let Some(set) = self.cells.get(&(sheet, addr)) {
            out.extend(set);
        }
        if let Some(list) = self.columns.get(&(sheet, addr.col())) {
            out.extend(
                list.iter()
                    .filter(|(top, bottom, _)| (*top..=*bottom).contains(&addr.row()))
                    .map(|(_, _, f)| *f),
            );
        }
        out.extend(
            self.wide
                .iter()
                .filter(|(s, range, _)| *s == sheet && range.contains(addr))
                .map(|(_, _, f)| *f),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(sheet: usize, s: &str) -> Key {
        (sheet, s.parse().unwrap())
    }

    fn range(sheet: usize, s: &str) -> (usize, CellRange) {
        (sheet, s.parse().unwrap())
    }

    fn dependents(deps: &Dependencies, sheet: usize, s: &str) -> Vec<String> {
        let mut out = Vec::new();
        deps.dependents(key(sheet, s), &mut out);
        let mut names: Vec<String> = out.iter().map(|(i, a)| format!("{i}!{a}")).collect();
        names.sort();
        names
    }

    #[test]
    fn finds_cell_column_and_wide_dependents() {
        let mut deps = Dependencies::default();
        deps.insert(key(0, "Z1"), vec![range(0, "A1"), range(0, "B1:B10")]);
        deps.insert(key(0, "Z2"), vec![range(0, "A1:XFD1")]);
        deps.insert(key(0, "Z3"), vec![range(0, "B:B")]);
        assert_eq!(dependents(&deps, 0, "A1"), ["0!Z1", "0!Z2"]);
        assert_eq!(dependents(&deps, 0, "B5"), ["0!Z1", "0!Z3"]);
        assert_eq!(dependents(&deps, 0, "B500"), ["0!Z3"]);
        assert_eq!(dependents(&deps, 0, "C1"), ["0!Z2"]);
        assert!(dependents(&deps, 0, "C2").is_empty());
    }

    #[test]
    fn sheets_are_kept_apart() {
        let mut deps = Dependencies::default();
        deps.insert(
            key(0, "A1"),
            vec![range(1, "B2"), range(1, "C:C"), range(1, "1:1")],
        );
        assert_eq!(dependents(&deps, 1, "B2"), ["0!A1"]);
        assert_eq!(dependents(&deps, 1, "C9"), ["0!A1"]);
        assert_eq!(dependents(&deps, 1, "Z1"), ["0!A1"]);
        assert!(dependents(&deps, 0, "B2").is_empty());
        assert!(dependents(&deps, 0, "Z1").is_empty());
    }

    #[test]
    fn reinserting_replaces_old_precedents() {
        let mut deps = Dependencies::default();
        let z1 = key(0, "Z1");
        deps.insert(z1, vec![range(0, "A1"), range(0, "B1:B2"), range(0, "1:1")]);
        deps.insert(z1, vec![range(0, "C3")]);
        assert!(dependents(&deps, 0, "A1").is_empty());
        assert!(dependents(&deps, 0, "B2").is_empty());
        assert_eq!(dependents(&deps, 0, "C3"), ["0!Z1"]);
        deps.remove(z1);
        assert!(dependents(&deps, 0, "C3").is_empty());
    }
}
