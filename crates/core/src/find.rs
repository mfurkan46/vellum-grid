use crate::{Cell, CellAddr, Locale, Sheet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FindOptions {
    pub match_case: bool,
    pub whole_cell: bool,
    pub values: bool,
}

pub fn searched_text(cell: &Cell, options: FindOptions, locale: &Locale) -> String {
    if options.values {
        locale.format_value(&cell.value, cell.number_format())
    } else {
        cell.input_text_in(locale)
    }
}

pub fn cell_matches(text: &str, query: &str, options: FindOptions) -> bool {
    if query.is_empty() {
        return false;
    }
    let text = chars(text, options.match_case);
    let query = chars(query, options.match_case);
    if options.whole_cell {
        wildcard(&query, &text)
    } else {
        first_match(&query, &text, 0).is_some()
    }
}

pub fn find_next(
    sheet: &Sheet,
    from: CellAddr,
    query: &str,
    options: FindOptions,
    locale: &Locale,
) -> Option<CellAddr> {
    let hit = |(_, cell): &(CellAddr, &Cell)| {
        cell.has_content() && cell_matches(&searched_text(cell, options, locale), query, options)
    };
    let cells: Vec<(CellAddr, &Cell)> = sheet.cells().collect();
    cells
        .iter()
        .filter(|(addr, _)| *addr > from)
        .chain(cells.iter().filter(|(addr, _)| *addr <= from))
        .find(|entry| hit(entry))
        .map(|(addr, _)| *addr)
}

pub fn replace_text(text: &str, query: &str, replacement: &str, options: FindOptions) -> String {
    let original: Vec<char> = text.chars().collect();
    let haystack = chars(text, options.match_case);
    let pattern = chars(query, options.match_case);
    if options.whole_cell {
        return if wildcard(&pattern, &haystack) {
            replacement.to_owned()
        } else {
            text.to_owned()
        };
    }
    let mut out = String::new();
    let mut position = 0;
    while let Some((start, end)) = first_match(&pattern, &haystack, position) {
        out.extend(&original[position..start]);
        out.push_str(replacement);
        position = end;
    }
    out.extend(&original[position..]);
    out
}

fn chars(text: &str, match_case: bool) -> Vec<char> {
    text.chars()
        .map(|c| match c {
            _ if match_case => c,
            'i' | 'ı' | 'İ' => 'I',
            c => c.to_uppercase().next().unwrap_or(c),
        })
        .collect()
}

fn first_match(pattern: &[char], text: &[char], from: usize) -> Option<(usize, usize)> {
    (from..=text.len()).find_map(|start| {
        (start..=text.len())
            .rev()
            .find(|&end| wildcard(pattern, &text[start..end]))
            .filter(|&end| end > start)
            .map(|end| (start, end))
    })
}

fn wildcard(pattern: &[char], text: &[char]) -> bool {
    match pattern.first() {
        None => text.is_empty(),
        Some('*') => (0..=text.len()).any(|skip| wildcard(&pattern[1..], &text[skip..])),
        Some('?') => !text.is_empty() && wildcard(&pattern[1..], &text[1..]),
        Some('~') if pattern.len() > 1 => {
            text.first() == Some(&pattern[1]) && wildcard(&pattern[2..], &text[1..])
        }
        Some(c) => text.first() == Some(c) && wildcard(&pattern[1..], &text[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TR;

    fn options(match_case: bool, whole_cell: bool) -> FindOptions {
        FindOptions {
            match_case,
            whole_cell,
            values: false,
        }
    }

    #[test]
    fn matches_with_wildcards_and_case() {
        let any = options(false, false);
        assert!(cell_matches("İstanbul", "istan", any));
        assert!(cell_matches("Ankara", "a?k", any));
        assert!(cell_matches("Ankara", "an*a", options(false, true)));
        assert!(!cell_matches("Ankara", "an", options(false, true)));
        assert!(!cell_matches("Ankara", "ank", options(true, false)));
        assert!(cell_matches("5*3", "~*", any));
        assert!(!cell_matches("53", "~*", any));
        assert!(!cell_matches("x", "", any));
    }

    #[test]
    fn replaces_every_occurrence() {
        let any = options(false, false);
        assert_eq!(
            replace_text("elma Elma", "elma", "armut", any),
            "armut armut"
        );
        assert_eq!(
            replace_text("elma Elma", "elma", "armut", options(true, false)),
            "armut Elma"
        );
        assert_eq!(replace_text("a-b-c", "-", "", any), "abc");
        assert_eq!(replace_text("ab12cd", "?2", "X", any), "abXcd");
        assert_eq!(replace_text("a*b", "~*", "x", any), "axb");
        assert_eq!(
            replace_text("Ankara", "an*", "B", options(false, true)),
            "B"
        );
        assert_eq!(
            replace_text("Ankara", "an", "B", options(false, true)),
            "Ankara"
        );
    }

    #[test]
    fn finds_in_reading_order_and_wraps() {
        let mut sheet = Sheet::new("s");
        for (at, input) in [
            ("B1", "elma"),
            ("A2", "armut"),
            ("C3", "Elma suyu"),
            ("D4", "=1+1"),
        ] {
            sheet.set(at.parse().unwrap(), Cell::parse_in(input, &TR).unwrap());
        }
        let mut amount = Cell::parse_in("1234,5", &TR).unwrap();
        amount.set_style(crate::Style::with_number_format(crate::numfmt::THOUSANDS));
        sheet.set("E5".parse().unwrap(), amount);
        let any = options(false, false);
        let next = |from: &str, query: &str, options| {
            find_next(&sheet, from.parse().unwrap(), query, options, &TR).map(|a| a.to_string())
        };
        assert_eq!(next("A1", "elma", any).as_deref(), Some("B1"));
        assert_eq!(next("B1", "elma", any).as_deref(), Some("C3"));
        assert_eq!(next("C3", "elma", any).as_deref(), Some("B1"));
        assert_eq!(next("A1", "kiraz", any), None);
        assert_eq!(next("A1", "+", any).as_deref(), Some("D4"));
        let values = FindOptions {
            values: true,
            ..any
        };
        assert_eq!(next("A1", "+", values), None);
        assert_eq!(next("A1", "1.234", values).as_deref(), Some("E5"));
        assert_eq!(next("A1", "1.234", any), None);
    }
}
