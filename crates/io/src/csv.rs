use std::io::{Read, Write};

use vellum_grid_core::{Cell, CellAddr, CellValue, EN, Locale, Sheet, TR};

use crate::{Error, encoding};

pub fn read_sheet<R: Read>(name: &str, reader: R) -> Result<Sheet, Error> {
    read_with(name, reader, b',', &EN)
}

pub fn read_bytes(name: &str, bytes: &[u8]) -> Result<Sheet, Error> {
    let text = encoding::decode(bytes);
    let delimiter = detect_delimiter(&text);
    let locale = if delimiter == b';' { &TR } else { &EN };
    read_with(name, text.as_bytes(), delimiter, locale)
}

fn read_with<R: Read>(
    name: &str,
    reader: R,
    delimiter: u8,
    locale: &Locale,
) -> Result<Sheet, Error> {
    let mut reader = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(reader);
    let mut sheet = Sheet::new(name);
    for (row, record) in reader.records().enumerate() {
        let record = record?;
        for (col, field) in record.iter().enumerate().filter(|(_, f)| !f.is_empty()) {
            let addr = CellAddr::new(to_u32(row), to_u32(col))?;
            let cell = Cell::parse_in(field, locale)
                .unwrap_or_else(|_| CellValue::Text(field.to_owned()).into());
            sheet.set(addr, cell);
        }
    }
    Ok(sheet)
}

fn detect_delimiter(text: &str) -> u8 {
    let mut counts = [(b',', 0), (b';', 0), (b'\t', 0)];
    let mut quoted = false;
    for c in text.chars() {
        if c == '\n' && !quoted {
            break;
        }
        if c == '"' {
            quoted = !quoted;
        } else if !quoted && let Some(entry) = counts.iter_mut().find(|(d, _)| char::from(*d) == c)
        {
            entry.1 += 1;
        }
    }
    counts
        .iter()
        .rev()
        .max_by_key(|(_, count)| *count)
        .filter(|(_, count)| *count > 0)
        .map_or(b',', |(delimiter, _)| *delimiter)
}

pub fn write_sheet<W: Write>(sheet: &Sheet, writer: W) -> Result<(), Error> {
    let mut writer = ::csv::Writer::from_writer(writer);
    let (rows, cols) = sheet.extent();
    let mut cells = sheet.cells().peekable();
    for row in 0..rows {
        let mut record = vec![String::new(); cols as usize];
        while let Some((addr, cell)) = cells.next_if(|(addr, _)| addr.row() == row) {
            record[addr.col() as usize] = cell.to_string();
        }
        writer.write_record(&record)?;
    }
    writer.flush()?;
    Ok(())
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_grid_core::{CellError, CellValue};

    fn load(csv: &str) -> Sheet {
        read_sheet("test", csv.as_bytes()).unwrap()
    }

    fn export(sheet: &Sheet) -> String {
        let mut out = Vec::new();
        write_sheet(sheet, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    #[test]
    fn infers_cell_types() {
        let sheet = load("ad,tutar,ok,hata\nelma,19.99,TRUE,#N/A\n");
        assert_eq!(sheet.value(addr("A2")), &CellValue::Text("elma".into()));
        assert_eq!(sheet.value(addr("B2")), &CellValue::Number(19.99));
        assert_eq!(sheet.value(addr("C2")), &CellValue::Bool(true));
        assert_eq!(sheet.value(addr("D2")), &CellValue::Error(CellError::NA));
    }

    #[test]
    fn empty_fields_are_not_stored() {
        let sheet = load("a,,c\n,,\n");
        assert_eq!(sheet.len(), 2);
        assert_eq!(sheet.extent(), (1, 3));
    }

    #[test]
    fn round_trip_preserves_content() {
        let input = "ad,tutar,not\n\
                     elma,19.99,\"virgül, tırnak \"\"x\"\"\"\n\
                     armut,0.1,=B2+B3\n\
                     ,,TRUE\n\
                     kiraz,-1500,#DIV/0!\n";
        let first = load(input);
        let output = export(&first);
        assert_eq!(output, input);
        assert_eq!(load(&output), first);
    }

    #[test]
    fn ragged_rows_are_padded() {
        let sheet = load("a,b,c\nd\n");
        assert_eq!(export(&sheet), "a,b,c\nd,,\n");
    }

    #[test]
    fn export_reflects_edits() {
        let mut sheet = load("x,1\ny,2\n");
        sheet.set_input(addr("B2"), "3.5");
        sheet.set_input(addr("C3"), "yeni");
        sheet.clear(addr("A1"));
        assert_eq!(export(&sheet), ",1,\ny,3.5,\n,,yeni\n");
    }

    #[test]
    fn exports_numbers_without_float_noise() {
        let mut sheet = Sheet::new("s");
        sheet.set(addr("A1"), CellValue::Number(0.1 + 0.2));
        assert_eq!(export(&sheet), "0.3\n");
    }

    #[test]
    fn empty_sheet_exports_nothing() {
        assert_eq!(export(&load("")), "");
    }

    #[test]
    fn rejects_invalid_utf8() {
        assert!(read_sheet("bad", &[0xff, 0xfe, b'\n'][..]).is_err());
    }

    #[test]
    fn detects_delimiters() {
        assert_eq!(detect_delimiter("a;b;c\n1,5;2"), b';');
        assert_eq!(detect_delimiter("a,b\n"), b',');
        assert_eq!(detect_delimiter("a\tb\tc"), b'\t');
        assert_eq!(detect_delimiter("\"x;y;z\",b\n"), b',');
        assert_eq!(detect_delimiter("tek"), b',');
    }

    #[test]
    fn reads_turkish_csv() {
        let mut bytes = b"\xDDsim;Tutar;Tarih;Durum\n".to_vec();
        bytes.extend_from_slice(b"\xC7i\xE7ek;1.234,50;17.10.2026;DO\xD0RU\n");
        bytes.extend_from_slice(b"Kitap;12,5;;YANLI\xDE\n");
        let sheet = read_bytes("Türkçe", &bytes).unwrap();
        assert_eq!(sheet.value(addr("A1")), &CellValue::Text("İsim".into()));
        assert_eq!(sheet.value(addr("A2")), &CellValue::Text("Çiçek".into()));
        assert_eq!(sheet.value(addr("B2")), &CellValue::Number(1234.5));
        assert_eq!(sheet.value(addr("B3")), &CellValue::Number(12.5));
        assert_eq!(sheet.value(addr("C2")), &CellValue::Number(46312.0));
        assert_eq!(
            sheet.get(addr("C2")).unwrap().number_format(),
            vellum_grid_core::numfmt::DATE
        );
        assert_eq!(sheet.value(addr("D2")), &CellValue::Bool(true));
        assert_eq!(sheet.value(addr("D3")), &CellValue::Bool(false));
    }

    #[test]
    fn reads_canonical_csv_from_bytes() {
        let sheet = read_bytes("s", "\u{feff}ad,tutar\nelma,1.5\n".as_bytes()).unwrap();
        assert_eq!(sheet.value(addr("A1")), &CellValue::Text("ad".into()));
        assert_eq!(sheet.value(addr("B2")), &CellValue::Number(1.5));
    }
}
