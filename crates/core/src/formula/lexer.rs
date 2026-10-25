use std::ops::Range;

use super::{Coord, FormulaError, Reference, StructuredRef, TableArea, parse_area};
use crate::locale::{EN, fold};
use crate::{CellError, CellRange, Locale, MAX_COLS, MAX_ROWS, column_index};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Number(f64),
    Text(String),
    Error(CellError),
    Word(String),
    Reference(Option<String>, Reference),
    Structured(StructuredRef),
    Op(&'static str),
    Sep,
    Open,
    Close,
}

const OPERATORS: [&str; 14] = [
    "<>", "<=", ">=", "<", ">", "=", "+", "-", "*", "/", "^", "&", "%", ":",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Highlight {
    Reference(Option<String>, CellRange),
    Name(String),
}

pub(crate) fn tokenize(src: &str, locale: &Locale) -> Result<Vec<Token>, FormulaError> {
    let mut tokens = Vec::new();
    scan(src, locale, &mut tokens, &mut Vec::new())?;
    Ok(tokens)
}

pub fn highlights(src: &str, locale: &Locale) -> Vec<(Range<usize>, Highlight)> {
    let mut tokens = Vec::new();
    let mut spans = Vec::new();
    scan(src, locale, &mut tokens, &mut spans).ok();
    let offsets: Vec<usize> = src
        .char_indices()
        .map(|(index, _)| index)
        .chain([src.len()])
        .collect();
    let mut out = Vec::new();
    for (index, (token, &(start, end))) in tokens.iter().zip(&spans).enumerate() {
        let highlight = match token {
            Token::Reference(sheet, reference) => {
                Highlight::Reference(sheet.clone(), reference.range())
            }
            Token::Word(word)
                if tokens.get(index + 1) != Some(&Token::Open)
                    && locale.parse_bool(word).is_none()
                    && EN.parse_bool(word).is_none() =>
            {
                Highlight::Name(word.clone())
            }
            _ => continue,
        };
        out.push((offsets[start]..offsets[end], highlight));
    }
    out
}

fn scan(
    src: &str,
    locale: &Locale,
    tokens: &mut Vec<Token>,
    spans: &mut Vec<(usize, usize)>,
) -> Result<(), FormulaError> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        scan_one(&chars, &mut i, locale, tokens)?;
        spans.resize(tokens.len(), (start, i));
    }
    Ok(())
}

fn scan_one(
    chars: &[char],
    index: &mut usize,
    locale: &Locale,
    tokens: &mut Vec<Token>,
) -> Result<(), FormulaError> {
    let mut i = *index;
    let c = chars[i];
    let next = chars.get(i + 1).copied();
    if c.is_whitespace() {
        i += 1;
    } else if c == '"' {
        let (text, end) = quoted(chars, i, '"').ok_or(FormulaError::UnterminatedText)?;
        tokens.push(Token::Text(text));
        i = end;
    } else if c == '#' {
        let (error, end) = error_at(chars, i, locale).ok_or(FormulaError::Syntax)?;
        tokens.push(Token::Error(error));
        i = end;
    } else if c == '\'' {
        let (sheet, end) = quoted(chars, i, '\'').ok_or(FormulaError::Syntax)?;
        if chars.get(end) != Some(&'!') {
            return Err(FormulaError::Syntax);
        }
        let (reference, end) = reference_at(chars, end + 1).ok_or(FormulaError::Syntax)?;
        tokens.push(Token::Reference(Some(sheet), reference));
        i = end;
    } else if c == locale.list {
        tokens.push(Token::Sep);
        i += 1;
    } else if c == '(' {
        tokens.push(Token::Open);
        i += 1;
    } else if c == ')' {
        tokens.push(Token::Close);
        i += 1;
    } else if c == '[' {
        let (reference, end) = structured_at(chars, i, None, locale).ok_or(FormulaError::Syntax)?;
        tokens.push(Token::Structured(reference));
        i = end;
    } else if let Some((reference, end)) = reference_at(chars, i) {
        tokens.push(Token::Reference(None, reference));
        i = end;
    } else if c.is_ascii_digit()
        || (c == locale.decimal && next.is_some_and(|n| n.is_ascii_digit()))
    {
        let (number, end) = number_at(chars, i, locale).ok_or(FormulaError::Syntax)?;
        tokens.push(Token::Number(number));
        i = end;
    } else if is_word_start(c) {
        let end = (i..chars.len())
            .find(|&j| !is_word_char(chars[j]))
            .unwrap_or(chars.len());
        let word: String = chars[i..end].iter().collect();
        if chars.get(end) == Some(&'!') {
            let (reference, end) = reference_at(chars, end + 1).ok_or(FormulaError::Syntax)?;
            tokens.push(Token::Reference(Some(word), reference));
            i = end;
        } else if chars.get(end) == Some(&'[') {
            let (reference, end) =
                structured_at(chars, end, Some(word), locale).ok_or(FormulaError::Syntax)?;
            tokens.push(Token::Structured(reference));
            i = end;
        } else {
            tokens.push(Token::Word(word));
            i = end;
        }
    } else if let Some(op) = OPERATORS
        .iter()
        .find(|op| op.chars().zip(&chars[i..]).filter(|(a, b)| a == *b).count() == op.len())
    {
        tokens.push(Token::Op(op));
        i += op.len();
    } else {
        return Err(FormulaError::Syntax);
    }
    *index = i;
    Ok(())
}

fn bracket_end(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = start;
    while i < chars.len() {
        match chars[i] {
            '\'' => i += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn unescape(chars: &[char]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\'' && i + 1 < chars.len() {
            i += 1;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn split_items(chars: &[char], separators: &[char]) -> Vec<Vec<char>> {
    let mut items = vec![Vec::new()];
    let mut depth = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' if i + 1 < chars.len() => {
                items.last_mut().expect("item").extend([c, chars[i + 1]]);
                i += 2;
                continue;
            }
            '[' => depth += 1,
            ']' => depth -= 1,
            c if depth == 0 && separators.contains(&c) => {
                items.push(Vec::new());
                i += 1;
                continue;
            }
            _ => {}
        }
        items.last_mut().expect("item").push(c);
        i += 1;
    }
    items
        .into_iter()
        .map(|item| {
            let start = item
                .iter()
                .position(|c| !c.is_whitespace())
                .unwrap_or(item.len());
            let end = item
                .iter()
                .rposition(|c| !c.is_whitespace())
                .map_or(start, |e| e + 1);
            item[start..end].to_vec()
        })
        .collect()
}

fn bracketed(item: &[char]) -> Option<&[char]> {
    (item.first() == Some(&'[') && item.last() == Some(&']')).then(|| &item[1..item.len() - 1])
}

fn column_span(item: &[char]) -> Option<(String, Option<String>)> {
    let parts = split_items(item, &[':']);
    match parts.as_slice() {
        [single] => Some((unescape(bracketed(single).unwrap_or(single)), None)),
        [first, last] => Some((
            unescape(bracketed(first)?),
            Some(unescape(bracketed(last)?)),
        )),
        _ => None,
    }
}

fn structured_at(
    chars: &[char],
    start: usize,
    table: Option<String>,
    locale: &Locale,
) -> Option<(StructuredRef, usize)> {
    let end = bracket_end(chars, start)?;
    let inner = &chars[start + 1..end];
    let mut area = TableArea::Data;
    let mut columns = None;
    if let Some(rest) = inner.strip_prefix(&['@']) {
        area = TableArea::ThisRow;
        if !rest.is_empty() {
            columns = Some(column_span(rest)?);
        }
    } else if inner.first() == Some(&'[') {
        let mut seen: Option<TableArea> = None;
        for item in split_items(inner, &[locale.list, EN.list]) {
            let body = bracketed(&item).unwrap_or(&item);
            if body.first() == Some(&'#') && item.first() == Some(&'[') {
                let found = parse_area(&body.iter().collect::<String>(), locale)?;
                seen = Some(match seen {
                    Some(previous) => previous.combine(found)?,
                    None => found,
                });
            } else if let Some(rest) = item.strip_prefix(&['@']) {
                seen = Some(TableArea::ThisRow);
                columns = Some(column_span(rest)?);
            } else {
                columns = Some(column_span(&item)?);
            }
        }
        area = seen.unwrap_or(TableArea::Data);
    } else if inner.first() == Some(&'#') {
        area = parse_area(&inner.iter().collect::<String>(), locale)?;
    } else if !inner.is_empty() {
        columns = Some((unescape(inner), None));
    }
    Some((
        StructuredRef {
            table,
            area,
            columns,
        },
        end + 1,
    ))
}

fn is_word_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '\\'
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}

fn quoted(chars: &[char], start: usize, quote: char) -> Option<(String, usize)> {
    let mut text = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == quote {
            if chars.get(i + 1) == Some(&quote) {
                text.push(quote);
                i += 2;
                continue;
            }
            return Some((text, i + 1));
        }
        text.push(chars[i]);
        i += 1;
    }
    None
}

fn error_at(chars: &[char], start: usize, locale: &Locale) -> Option<(CellError, usize)> {
    locale
        .errors()
        .chain(EN.errors())
        .filter_map(|(error, name)| {
            let len = name.chars().count();
            let candidate: String = chars.get(start..start + len)?.iter().collect();
            (fold(&candidate) == fold(name)).then_some((error, start + len))
        })
        .max_by_key(|(_, end)| *end)
}

fn number_at(chars: &[char], start: usize, locale: &Locale) -> Option<(f64, usize)> {
    let mut i = start;
    let mut text = String::new();
    let digits = |i: &mut usize, text: &mut String| {
        while let Some(c) = chars.get(*i).filter(|c| c.is_ascii_digit()) {
            text.push(*c);
            *i += 1;
        }
    };
    digits(&mut i, &mut text);
    if chars.get(i) == Some(&locale.decimal) {
        text.push('.');
        i += 1;
        digits(&mut i, &mut text);
    }
    if matches!(chars.get(i), Some('e' | 'E')) {
        let mut exponent = String::from("e");
        let mut j = i + 1;
        if let Some(sign @ ('+' | '-')) = chars.get(j) {
            exponent.push(*sign);
            j += 1;
        }
        let before = exponent.len();
        digits(&mut j, &mut exponent);
        if exponent.len() > before {
            text.push_str(&exponent);
            i = j;
        }
    }
    let n: f64 = text.parse().ok().filter(|n: &f64| n.is_finite())?;
    Some((n, i))
}

fn reference_at(chars: &[char], start: usize) -> Option<(Reference, usize)> {
    let (first, end) = part_at(chars, start)?;
    let (reference, end) = if chars.get(end) == Some(&':') {
        match part_at(chars, end + 1) {
            Some((second, end)) => (combine(first, second)?, end),
            None => (single(first)?, end),
        }
    } else {
        (single(first)?, end)
    };
    let follows_word = chars
        .get(end)
        .is_some_and(|&c| is_word_char(c) || c == '(' || c == '!');
    (!follows_word).then_some((reference, end))
}

#[derive(Clone, Copy)]
enum Part {
    Cell(Coord, Coord),
    Col(Coord),
    Row(Coord),
}

fn part_at(chars: &[char], start: usize) -> Option<(Part, usize)> {
    let mut i = start;
    let col_abs = chars.get(i) == Some(&'$');
    if col_abs {
        i += 1;
    }
    let letters_end = (i..chars.len())
        .find(|&j| !chars[j].is_ascii_alphabetic())
        .unwrap_or(chars.len());
    let letters: String = chars[i..letters_end].iter().collect();
    let mut j = letters_end;
    let row_abs = !letters.is_empty() && chars.get(j) == Some(&'$');
    if row_abs {
        j += 1;
    }
    let digits_end = (j..chars.len())
        .find(|&k| !chars[k].is_ascii_digit())
        .unwrap_or(chars.len());
    let digits: String = chars[j..digits_end].iter().collect();

    let col = || {
        let index = column_index(&letters).ok()?;
        Some(Coord {
            index,
            absolute: col_abs,
        })
    };
    let row = |absolute: bool| {
        let n: u32 = digits.parse().ok()?;
        (1..=MAX_ROWS).contains(&n).then(|| Coord {
            index: n - 1,
            absolute,
        })
    };
    match (letters.is_empty(), digits.is_empty()) {
        (false, false) => Some((Part::Cell(row(row_abs)?, col()?), digits_end)),
        (false, true) if !row_abs => Some((Part::Col(col()?), letters_end)),
        (true, false) => Some((Part::Row(row(col_abs)?), digits_end)),
        _ => None,
    }
}

fn single(part: Part) -> Option<Reference> {
    match part {
        Part::Cell(row, col) => Some(Reference::Cell { row, col }),
        _ => None,
    }
}

fn combine(a: Part, b: Part) -> Option<Reference> {
    let order = |x: Coord, y: Coord| if x.index <= y.index { (x, y) } else { (y, x) };
    match (a, b) {
        (Part::Cell(r1, c1), Part::Cell(r2, c2)) => {
            let (top, bottom) = order(r1, r2);
            let (left, right) = order(c1, c2);
            Some(Reference::Area {
                top,
                left,
                bottom,
                right,
            })
        }
        (Part::Col(c1), Part::Col(c2)) if c1.index < MAX_COLS && c2.index < MAX_COLS => {
            let (left, right) = order(c1, c2);
            Some(Reference::Cols { left, right })
        }
        (Part::Row(r1), Part::Row(r2)) => {
            let (top, bottom) = order(r1, r2);
            Some(Reference::Rows { top, bottom })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TR;

    fn tokens(src: &str, locale: &Locale) -> Vec<Token> {
        tokenize(src, locale).unwrap()
    }

    #[test]
    fn reads_turkish_numbers_and_separators() {
        assert_eq!(
            tokens("1,5;2", &TR),
            [Token::Number(1.5), Token::Sep, Token::Number(2.0)]
        );
        assert_eq!(
            tokens("1.5,2", &EN),
            [Token::Number(1.5), Token::Sep, Token::Number(2.0)]
        );
        assert_eq!(tokens("1,5E+3", &TR), [Token::Number(1500.0)]);
    }

    #[test]
    fn reads_references() {
        let range = |s: &str| match &tokens(s, &EN)[0] {
            Token::Reference(_, r) => r.range().to_string(),
            other => panic!("{other:?}"),
        };
        assert_eq!(range("$a$1"), "A1");
        assert_eq!(range("B3:A1"), "A1:B3");
        assert_eq!(range("C:A"), "A1:C1048576");
        assert_eq!(range("2:3"), "A2:XFD3");
        assert_eq!(range("XFD1048576"), "XFD1048576");
    }

    #[test]
    fn words_that_are_not_references() {
        assert_eq!(tokens("LOG10(", &EN)[0], Token::Word("LOG10".into()));
        assert_eq!(tokens("ABCD1", &EN)[0], Token::Word("ABCD1".into()));
        assert_eq!(tokens("A1048577", &EN)[0], Token::Word("A1048577".into()));
        assert_eq!(
            tokens("BAĞ_DEĞ_SAY(", &TR)[0],
            Token::Word("BAĞ_DEĞ_SAY".into())
        );
    }

    #[test]
    fn reads_sheet_prefixes() {
        assert_eq!(
            tokens("'Gelir Tablosu'!B2", &TR)[0],
            Token::Reference(
                Some("Gelir Tablosu".into()),
                tokens("B2", &TR)[0].reference()
            )
        );
        assert!(
            matches!(&tokens("Sayfa1!A:A", &TR)[0], Token::Reference(Some(s), _) if s == "Sayfa1")
        );
    }

    #[test]
    fn reads_text_errors_and_operators() {
        assert_eq!(
            tokens("\"a\"\"b\"&#YOK<>#n/a", &TR),
            [
                Token::Text("a\"b".into()),
                Token::Op("&"),
                Token::Error(CellError::NA),
                Token::Op("<>"),
                Token::Error(CellError::NA),
            ]
        );
        assert_eq!(tokenize("\"open", &EN), Err(FormulaError::UnterminatedText));
        assert_eq!(tokenize("1 ? 2", &EN), Err(FormulaError::Syntax));
    }

    #[test]
    fn finds_reference_and_name_spans() {
        let src = "=TOPLA(A1:B2;'Gelir Tablosu'!C3)+Fiyat*DOĞRU+\"D4\"+EĞER(";
        let found = highlights(src, &TR);
        let texts: Vec<_> = found.iter().map(|(span, _)| &src[span.clone()]).collect();
        assert_eq!(texts, ["A1:B2", "'Gelir Tablosu'!C3", "Fiyat"]);
        assert_eq!(
            found[1].1,
            Highlight::Reference(Some("Gelir Tablosu".into()), "C3".parse().unwrap())
        );
        assert_eq!(found[2].1, Highlight::Name("Fiyat".into()));
        let partial = highlights("=A1+\"açık", &TR);
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].0, 1..3);
    }

    impl Token {
        fn reference(&self) -> Reference {
            match self {
                Token::Reference(_, r) => *r,
                other => panic!("{other:?}"),
            }
        }
    }
}
