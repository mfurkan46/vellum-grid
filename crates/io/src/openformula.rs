pub fn to_canonical(src: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push('"');
                loop {
                    let c = chars.next()?;
                    out.push(c);
                    if c == '"' {
                        if chars.peek() == Some(&'"') {
                            out.push(chars.next()?);
                        } else {
                            break;
                        }
                    }
                }
            }
            '[' => {
                let mut inner = String::new();
                loop {
                    match chars.next()? {
                        ']' => break,
                        c => inner.push(c),
                    }
                }
                out.push_str(&reference(&inner)?);
            }
            ';' => out.push(','),
            c => out.push(c),
        }
    }
    Some(out.replace("COM.MICROSOFT.", ""))
}

fn reference(inner: &str) -> Option<String> {
    let mut parts = inner.splitn(2, ':');
    let (sheet, first) = part(parts.next()?)?;
    let mut out = String::new();
    if let Some(sheet) = sheet {
        out.push('\'');
        out.push_str(&sheet.replace('\'', "''"));
        out.push_str("'!");
    }
    out.push_str(&first);
    if let Some(second) = parts.next() {
        out.push(':');
        out.push_str(&part(second)?.1);
    }
    Some(out)
}

fn part(text: &str) -> Option<(Option<String>, String)> {
    let dot = text.rfind('.')?;
    let sheet = text[..dot].trim_start_matches('$');
    let sheet = (!sheet.is_empty()).then(|| {
        sheet
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .map_or_else(|| sheet.to_owned(), |s| s.replace("''", "'"))
    });
    Some((sheet, text[dot + 1..].to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_references_and_separators() {
        assert_eq!(
            to_canonical("SUM([.A1:.B3])+[.C1]*2").as_deref(),
            Some("SUM(A1:B3)+C1*2")
        );
        assert_eq!(
            to_canonical("IF([.$A$1]>1;\"a;b\";\"x\"\"y\")").as_deref(),
            Some("IF($A$1>1,\"a;b\",\"x\"\"y\")")
        );
        assert_eq!(to_canonical("[$Sayfa2.A1]").as_deref(), Some("'Sayfa2'!A1"));
        assert_eq!(
            to_canonical("SUM([$'Gelir Tablosu'.B2:.B9])").as_deref(),
            Some("SUM('Gelir Tablosu'!B2:B9)")
        );
        assert_eq!(
            to_canonical("COUNTA([.A:.A])").as_deref(),
            Some("COUNTA(A:A)")
        );
        assert_eq!(
            to_canonical("COM.MICROSOFT.XLOOKUP([.A1];[.B1:.B3];[.C1:.C3])").as_deref(),
            Some("XLOOKUP(A1,B1:B3,C1:C3)")
        );
    }

    #[test]
    fn rejects_unterminated_input() {
        assert_eq!(to_canonical("[.A1"), None);
        assert_eq!(to_canonical("\"open"), None);
    }
}
