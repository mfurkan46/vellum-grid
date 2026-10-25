use ::csv::{ReaderBuilder, Terminator, WriterBuilder};
use vellum_grid_core::{Cell, CellAddr, CellRange, Sheet};

pub fn to_tsv(sheet: &Sheet, range: CellRange, text: impl Fn(&Cell) -> String) -> String {
    let range = sheet.trim_to_extent(range);
    let (start, end) = (range.start(), range.end());

    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
        .terminator(Terminator::CRLF)
        .flexible(true)
        .from_writer(Vec::new());
    for row in start.row()..=end.row() {
        let record: Vec<String> = (start.col()..=end.col())
            .map(|col| {
                let addr = CellAddr::new(row, col).expect("inside the range");
                sheet.get(addr).map(&text).unwrap_or_default()
            })
            .collect();
        writer
            .write_record(&record)
            .expect("writing to memory cannot fail");
    }
    let bytes = writer.into_inner().expect("writing to memory cannot fail");
    String::from_utf8(bytes).expect("cell text is UTF-8")
}

pub fn parse_tsv(text: &str) -> Vec<Vec<String>> {
    ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes())
        .records()
        .flatten()
        .map(|record| record.iter().map(str::to_owned).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> Sheet {
        let mut sheet = Sheet::new("S");
        for (addr, input) in [
            ("A1", "ad"),
            ("B1", "1.5"),
            ("A2", "çok\tsatır"),
            ("B3", "=A1"),
        ] {
            sheet.set_input(addr.parse().unwrap(), input);
        }
        sheet
    }

    #[test]
    fn copies_range_as_text() {
        let text = to_tsv(&sheet(), "A1:B3".parse().unwrap(), ToString::to_string);
        assert_eq!(text, "ad\t1.5\r\n\"çok\tsatır\"\t\r\n\t=A1\r\n");
    }

    #[test]
    fn whole_columns_are_clipped_to_used_area() {
        let text = to_tsv(
            &sheet(),
            "B1:B1048576".parse().unwrap(),
            ToString::to_string,
        );
        assert_eq!(text, "1.5\r\n\"\"\r\n=A1\r\n");
    }

    #[test]
    fn empty_area_copies_one_empty_cell() {
        let text = to_tsv(&sheet(), "F9:G12".parse().unwrap(), ToString::to_string);
        assert_eq!(parse_tsv(&text), [[""]]);
    }

    #[test]
    fn parses_clipboard_text() {
        let rows = parse_tsv("a\tb\r\n\"x\ty\"\t\r\n3\r\n");
        assert_eq!(rows, vec![vec!["a", "b"], vec!["x\ty", ""], vec!["3"]]);
    }

    #[test]
    fn parses_single_value_without_newline() {
        assert_eq!(parse_tsv("42"), [["42"]]);
    }

    #[test]
    fn round_trips_through_clipboard() {
        let sheet = sheet();
        let range = "A1:B3".parse().unwrap();
        let rows = parse_tsv(&to_tsv(&sheet, range, ToString::to_string));
        assert_eq!(rows[1][0], "çok\tsatır");
        assert_eq!(rows[2][1], "=A1");
    }
}
