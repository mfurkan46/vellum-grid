use std::cmp::Ordering;

use super::eval::compare;
use crate::locale::fold;
use crate::{CellValue, Locale};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
enum Target {
    Value(CellValue),
    Pattern(String),
    Blank,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Criteria {
    op: Op,
    target: Target,
}

impl Criteria {
    pub(super) fn new(value: &CellValue, locale: &Locale) -> Self {
        let CellValue::Text(text) = value else {
            let target = match value {
                CellValue::Empty => Target::Blank,
                other => Target::Value(other.clone()),
            };
            return Self { op: Op::Eq, target };
        };
        let (op, rest) = [
            ("<>", Op::Ne),
            ("<=", Op::Le),
            (">=", Op::Ge),
            ("<", Op::Lt),
            (">", Op::Gt),
            ("=", Op::Eq),
        ]
        .into_iter()
        .find_map(|(prefix, op)| text.strip_prefix(prefix).map(|rest| (op, rest)))
        .unwrap_or((Op::Eq, text.as_str()));
        let target = if rest.is_empty() {
            Target::Blank
        } else if let Some(n) = locale.parse_value(rest) {
            Target::Value(CellValue::Number(n))
        } else if let Some(b) = locale.parse_bool(rest) {
            Target::Value(CellValue::Bool(b))
        } else if let Some(e) = locale.parse_error(rest) {
            Target::Value(CellValue::Error(e))
        } else if matches!(op, Op::Eq | Op::Ne) {
            Target::Pattern(fold(rest))
        } else {
            Target::Value(CellValue::Text(rest.to_owned()))
        };
        Self { op, target }
    }

    pub(super) fn matches(&self, value: &CellValue) -> bool {
        let blank = value.is_empty() || matches!(value, CellValue::Text(s) if s.is_empty());
        let hit = match &self.target {
            Target::Blank => blank,
            Target::Pattern(pattern) => match value {
                CellValue::Text(s) => wildcard(pattern, &fold(s)),
                _ => false,
            },
            Target::Value(target) => {
                if std::mem::discriminant(target) != std::mem::discriminant(value) {
                    return self.op == Op::Ne;
                }
                return compare(value, target).is_ok_and(|ordering| self.accepts(ordering));
            }
        };
        if self.op == Op::Ne { !hit } else { hit }
    }

    fn accepts(&self, ordering: Ordering) -> bool {
        match self.op {
            Op::Eq => ordering == Ordering::Equal,
            Op::Ne => ordering != Ordering::Equal,
            Op::Lt => ordering == Ordering::Less,
            Op::Le => ordering != Ordering::Greater,
            Op::Gt => ordering == Ordering::Greater,
            Op::Ge => ordering != Ordering::Less,
        }
    }
}

enum Piece {
    Any,
    One,
    Char(char),
}

pub(super) fn wildcard(pattern: &str, text: &str) -> bool {
    let mut pieces = Vec::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        pieces.push(match c {
            '*' => Piece::Any,
            '?' => Piece::One,
            '~' => Piece::Char(chars.next().unwrap_or('~')),
            c => Piece::Char(c),
        });
    }
    let text: Vec<char> = text.chars().collect();
    let mut matched = vec![false; text.len() + 1];
    matched[0] = true;
    for piece in &pieces {
        let mut next = vec![false; text.len() + 1];
        for j in 0..=text.len() {
            next[j] = match piece {
                Piece::Any => matched[j] || (j > 0 && next[j - 1]),
                Piece::One => j > 0 && matched[j - 1],
                Piece::Char(c) => j > 0 && matched[j - 1] && *c == text[j - 1],
            };
        }
        matched = next;
    }
    matched[text.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EN, TR};

    fn criteria(text: &str) -> Criteria {
        Criteria::new(&CellValue::Text(text.into()), &TR)
    }

    fn text(s: &str) -> CellValue {
        CellValue::Text(s.into())
    }

    #[test]
    fn numeric_comparisons() {
        let gt = criteria(">1,5");
        assert!(gt.matches(&CellValue::Number(2.0)));
        assert!(!gt.matches(&CellValue::Number(1.5)));
        assert!(!gt.matches(&text("9")));
        assert!(criteria("<=10").matches(&CellValue::Number(10.0)));
        assert!(criteria("<>5").matches(&text("x")));
        assert!(Criteria::new(&CellValue::Number(3.0), &TR).matches(&CellValue::Number(3.0)));
    }

    #[test]
    fn text_matching_is_case_insensitive_with_wildcards() {
        assert!(criteria("elma").matches(&text("ELMA")));
        assert!(criteria("İstanbul").matches(&text("istanbul")));
        assert!(criteria("el*").matches(&text("Elmalı")));
        assert!(criteria("?lma").matches(&text("alma")));
        assert!(!criteria("?lma").matches(&text("lma")));
        assert!(criteria("*~*").matches(&text("5*")));
        assert!(!criteria("*~*").matches(&text("5?")));
        assert!(criteria("<>el*").matches(&text("armut")));
        assert!(!criteria("<>el*").matches(&text("elma")));
    }

    #[test]
    fn blanks_and_booleans() {
        assert!(criteria("").matches(&CellValue::Empty));
        assert!(criteria("=").matches(&CellValue::Empty));
        assert!(!criteria("=").matches(&text("x")));
        assert!(criteria("<>").matches(&text("x")));
        assert!(!criteria("<>").matches(&CellValue::Empty));
        assert!(criteria("DOĞRU").matches(&CellValue::Bool(true)));
        assert!(Criteria::new(&text("TRUE"), &EN).matches(&CellValue::Bool(true)));
    }
}
