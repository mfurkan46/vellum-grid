use std::fmt;

use crate::formula::{self, Expr};
use crate::locale::{EN, TR, fold};
use crate::{CellError, CellRange, Workbook};

const MAX_NAME_LEN: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinedName {
    pub name: String,
    pub scope: Option<usize>,
    pub formula: String,
    pub comment: String,
}

impl DefinedName {
    pub fn new(name: &str, formula: &str) -> Self {
        Self {
            name: name.to_owned(),
            scope: None,
            formula: formula.to_owned(),
            comment: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    Invalid(String),
    Duplicate(String),
    Formula,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(name) => write!(f, "'{name}' is not a valid name"),
            Self::Duplicate(name) => write!(f, "the name '{name}' already exists"),
            Self::Formula => write!(f, "the reference is not a valid formula"),
        }
    }
}

impl std::error::Error for NameError {}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '\\'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}

fn looks_like_reference(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let r1c1 = |s: &str| {
        let rest = s.trim_start_matches(|c: char| c.is_ascii_digit());
        rest.is_empty()
            || rest
                .strip_prefix('C')
                .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()))
    };
    upper.parse::<CellRange>().is_ok()
        || upper.strip_prefix('R').is_some_and(r1c1)
        || upper
            .strip_prefix('C')
            .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()))
}

pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(is_name_start)
        && chars.all(is_name_char)
        && name.chars().count() <= MAX_NAME_LEN
        && !looks_like_reference(name)
        && EN.parse_bool(name).is_none()
        && TR.parse_bool(name).is_none()
}

pub fn name_from_label(label: &str) -> Option<String> {
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    let mut name: String = label
        .chars()
        .map(|c| if is_name_char(c) { c } else { '_' })
        .collect();
    if !name.starts_with(is_name_start) || looks_like_reference(&name) {
        name.insert(0, '_');
    }
    is_valid_name(&name).then_some(name)
}

pub(crate) fn check_names(names: &[DefinedName]) -> Result<(), NameError> {
    for (index, defined) in names.iter().enumerate() {
        if !is_valid_name(&defined.name) {
            return Err(NameError::Invalid(defined.name.clone()));
        }
        let key = fold(&defined.name);
        let clash = names[..index]
            .iter()
            .any(|other| other.scope == defined.scope && fold(&other.name) == key);
        if clash {
            return Err(NameError::Duplicate(defined.name.clone()));
        }
        if formula::parse(&defined.formula, &EN).is_err() {
            return Err(NameError::Formula);
        }
    }
    Ok(())
}

impl Workbook {
    pub fn names(&self) -> &[DefinedName] {
        &self.names
    }

    pub fn add_name(&mut self, defined: DefinedName) -> Result<(), NameError> {
        let mut names = self.names.clone();
        names.push(defined);
        check_names(&names)?;
        self.names = names;
        Ok(())
    }

    pub fn find_name(&self, name: &str, sheet: usize) -> Option<&DefinedName> {
        let key = fold(name);
        let named = |defined: &&DefinedName| fold(&defined.name) == key;
        let mut found = self.names.iter().filter(named);
        let local = found.clone().find(|defined| defined.scope == Some(sheet));
        local.or_else(|| found.find(|defined| defined.scope.is_none()))
    }

    pub fn name_range(&self, name: &str, sheet: usize) -> Option<(usize, CellRange)> {
        let expr = self.expand_names(&Expr::Name(name.to_owned()), sheet);
        match expr {
            Expr::Reference {
                sheet: named,
                reference,
            } => {
                let index = match named {
                    Some(named) => self.index_of(&named)?,
                    None => sheet,
                };
                Some((index, reference.range()))
            }
            _ => None,
        }
    }

    pub fn expand_names(&self, expr: &Expr, sheet: usize) -> Expr {
        self.expand(expr, sheet, &[])
    }

    fn expand(&self, expr: &Expr, sheet: usize, chain: &[String]) -> Expr {
        expr.map_names(&|name| {
            let defined = self.find_name(name, sheet)?;
            let key = fold(&defined.name);
            if chain.contains(&key) {
                return Some(Expr::Error(CellError::Name));
            }
            let Ok(parsed) = formula::parse(&defined.formula, &EN) else {
                return Some(Expr::Error(CellError::Name));
            };
            let mut chain = chain.to_vec();
            chain.push(key);
            Some(match self.expand(&parsed, sheet, &chain) {
                inner @ (Expr::Reference { .. }
                | Expr::Number(_)
                | Expr::Text(_)
                | Expr::Bool(_)
                | Expr::Error(_)
                | Expr::Paren(_)
                | Expr::Call(..)) => inner,
                other => Expr::Paren(Box::new(other)),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sheet;

    fn book() -> Workbook {
        let mut book = Workbook::new();
        book.add_sheet(Sheet::new("Veri")).unwrap();
        book
    }

    #[test]
    fn validates_names() {
        for name in [
            "Vergi_Oranı",
            "_toplam",
            "\\kök",
            "Satış.2024",
            "A",
            "Gelir2",
        ] {
            assert!(is_valid_name(name), "{name}");
        }
        for name in [
            "",
            "1yıl",
            "A1",
            "xfd1048576",
            "TAX2024",
            "R",
            "c",
            "R1C1",
            "R2",
            "C10",
            "DOĞRU",
            "true",
            "ad soyad",
            "a-b",
        ] {
            assert!(!is_valid_name(name), "{name}");
        }
        assert_eq!(name_from_label("Toplam Satış"), Some("Toplam_Satış".into()));
        assert_eq!(name_from_label("2024"), Some("_2024".into()));
        assert_eq!(name_from_label("B2"), Some("_B2".into()));
        assert_eq!(name_from_label(""), None);
    }

    #[test]
    fn rejects_duplicates_in_the_same_scope() {
        let mut book = book();
        book.add_name(DefinedName::new("Oran", "Veri!$A$1"))
            .unwrap();
        assert_eq!(
            book.add_name(DefinedName::new("ORAN", "Veri!$B$1")),
            Err(NameError::Duplicate("ORAN".into()))
        );
        let local = DefinedName {
            scope: Some(1),
            ..DefinedName::new("oran", "Veri!$B$1")
        };
        book.add_name(local).unwrap();
        assert_eq!(
            book.add_name(DefinedName::new("Bozuk", "SUM(")),
            Err(NameError::Formula)
        );
        assert_eq!(
            book.add_name(DefinedName::new("A1", "1")),
            Err(NameError::Invalid("A1".into()))
        );
    }

    #[test]
    fn sheet_names_shadow_workbook_names() {
        let mut book = book();
        book.add_name(DefinedName::new("Oran", "Veri!$A$1"))
            .unwrap();
        book.add_name(DefinedName {
            scope: Some(1),
            ..DefinedName::new("Oran", "Veri!$B$1")
        })
        .unwrap();
        assert_eq!(book.find_name("oran", 0).unwrap().formula, "Veri!$A$1");
        assert_eq!(book.find_name("ORAN", 1).unwrap().formula, "Veri!$B$1");
        assert_eq!(book.name_range("Oran", 1), Some((1, "B1".parse().unwrap())));
        assert_eq!(book.name_range("Yok", 0), None);
    }

    #[test]
    fn expands_nested_and_circular_names() {
        let mut book = book();
        book.add_name(DefinedName::new("Fiyat", "Veri!$A$1:$A$3"))
            .unwrap();
        book.add_name(DefinedName::new("Toplam", "SUM(Fiyat)"))
            .unwrap();
        book.add_name(DefinedName::new("Kdv", "Toplam*0.2"))
            .unwrap();
        book.add_name(DefinedName::new("Döngü", "Döngü+1")).unwrap();
        let expand = |src: &str| {
            let expr = formula::parse(src, &EN).unwrap();
            book.expand_names(&expr, 0).write(&EN)
        };
        assert_eq!(expand("kdv+1"), "(SUM(Veri!$A$1:$A$3)*0.2)+1");
        assert_eq!(expand("Döngü"), "(#NAME?+1)");
        assert_eq!(expand("Bilinmeyen*2"), "Bilinmeyen*2");
    }

    #[test]
    fn sheet_moves_and_removals_follow_scopes() {
        let mut book = book();
        book.add_sheet(Sheet::new("Son")).unwrap();
        let local = |name: &str, scope| DefinedName {
            scope: Some(scope),
            ..DefinedName::new(name, "1")
        };
        book.add_name(local("İlk", 0)).unwrap();
        book.add_name(local("Orta", 1)).unwrap();
        book.add_name(local("Son", 2)).unwrap();
        book.move_sheet(0, 2);
        let scopes: Vec<_> = book.names().iter().map(|n| n.scope).collect();
        assert_eq!(scopes, [Some(2), Some(0), Some(1)]);
        book.insert_sheet(0, Sheet::new("Yeni")).unwrap();
        let scopes: Vec<_> = book.names().iter().map(|n| n.scope).collect();
        assert_eq!(scopes, [Some(3), Some(1), Some(2)]);
        book.remove_at(1).unwrap();
        let names: Vec<_> = book
            .names()
            .iter()
            .map(|n| (n.name.as_str(), n.scope))
            .collect();
        assert_eq!(names, [("İlk", Some(2)), ("Son", Some(1))]);
    }
}
