use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use vellum_grid_core::formula::{self, Coord, Expr, Reference};
use vellum_grid_core::numfmt::{self, Kind};
use vellum_grid_core::{
    Border, CellAddr, CellValue, EN, HAlign, LineStyle, Style, Underline, VAlign, Workbook,
    column_name, from_serial,
};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::Error;

const MIMETYPE: &str = "application/vnd.oasis.opendocument.spreadsheet";
const NAMESPACES: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2" office:version="1.3""#;
const POINTS_PER_PIXEL: f64 = 0.75;

pub fn write_ods(book: &Workbook) -> Result<Vec<u8>, Error> {
    let mut styles = Styles::default();
    let mut tables = String::new();
    for index in 0..book.sheets().len() {
        write_table(book, index, &mut styles, &mut tables);
    }
    write_names(book, &mut tables);
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NAMESPACES}><office:automatic-styles>{}</office:automatic-styles><office:body><office:spreadsheet>{tables}</office:spreadsheet></office:body></office:document-content>"#,
        styles.xml()
    );
    let manifest = r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/></manifest:manifest>"#;
    let shared = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NAMESPACES}><office:styles/></office:document-styles>"#
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("mimetype", stored)?;
    zip.write_all(MIMETYPE.as_bytes())?;
    zip.start_file("META-INF/manifest.xml", deflated)?;
    zip.write_all(manifest.as_bytes())?;
    zip.start_file("styles.xml", deflated)?;
    zip.write_all(shared.as_bytes())?;
    zip.start_file("content.xml", deflated)?;
    zip.write_all(content.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}

fn write_names(book: &Workbook, out: &mut String) {
    let base = book.sheets()[0].name().replace('\'', "''");
    let base = escape(&format!("$'{base}'.$A$1"));
    let mut names = String::new();
    for defined in book
        .names()
        .iter()
        .filter(|defined| defined.scope.is_none())
    {
        let Ok(expr) = formula::parse(&defined.formula, &EN) else {
            continue;
        };
        let name = escape(&defined.name);
        match expr {
            Expr::Reference {
                sheet: Some(sheet),
                reference,
            } => {
                let mut range = String::new();
                write_reference(
                    &format!("$'{}'", sheet.replace('\'', "''")),
                    reference,
                    &mut range,
                );
                let _ = write!(
                    names,
                    r#"<table:named-range table:name="{name}" table:base-cell-address="{base}" table:cell-range-address="{}"/>"#,
                    escape(&range)
                );
            }
            expr => {
                let _ = write!(
                    names,
                    r#"<table:named-expression table:name="{name}" table:base-cell-address="{base}" table:expression="of:={}"/>"#,
                    escape(&openformula(&expr))
                );
            }
        }
    }
    if !names.is_empty() {
        let _ = write!(
            out,
            "<table:named-expressions>{names}</table:named-expressions>"
        );
    }
}

fn write_table(book: &Workbook, index: usize, styles: &mut Styles, out: &mut String) {
    let sheet = &book.sheets()[index];
    let layout = sheet.layout();
    let (rows, cols) = sheet.extent();
    let _ = write!(
        out,
        r#"<table:table table:name="{}">"#,
        escape(sheet.name())
    );
    for col in 0..cols.max(1) {
        let width = styles.column(layout.col_width(col));
        let hidden = if layout.hidden_cols.contains(&col) {
            r#" table:visibility="collapse""#
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"<table:table-column table:style-name="{width}"{hidden}/>"#
        );
    }
    let mut spans: HashMap<CellAddr, (u32, u32)> = HashMap::new();
    let mut covered: HashSet<CellAddr> = HashSet::new();
    for merge in &layout.merges {
        spans.insert(merge.start(), (merge.rows(), merge.cols()));
        for addr in cells_of(merge.start(), merge.end()) {
            if addr != merge.start() {
                covered.insert(addr);
            }
        }
    }
    let by_row: BTreeMap<u32, Vec<CellAddr>> =
        sheet.cells().fold(BTreeMap::new(), |mut map, (addr, _)| {
            map.entry(addr.row()).or_insert_with(Vec::new).push(addr);
            map
        });
    for row in 0..rows {
        let height = layout.row_heights.get(&row).copied();
        let row_style = height.map(|points| styles.row(points));
        let mut attributes = String::new();
        if let Some(name) = row_style {
            let _ = write!(attributes, r#" table:style-name="{name}""#);
        }
        if layout.hidden_rows.contains(&row) {
            attributes.push_str(r#" table:visibility="collapse""#);
        }
        let _ = write!(out, "<table:table-row{attributes}>");
        let mut col = 0;
        let addrs = by_row.get(&row).cloned().unwrap_or_default();
        let covered_here: Vec<u32> = covered
            .iter()
            .filter(|a| a.row() == row)
            .map(|a| a.col())
            .collect();
        let mut columns: Vec<u32> = addrs.iter().map(|a| a.col()).collect();
        columns.extend(covered_here);
        columns.sort_unstable();
        columns.dedup();
        for target in columns {
            if target > col {
                let _ = write!(
                    out,
                    r#"<table:table-cell table:number-columns-repeated="{}"/>"#,
                    target - col
                );
            }
            let addr = CellAddr::new(row, target).expect("cells are inside the sheet");
            if covered.contains(&addr) {
                out.push_str("<table:covered-table-cell/>");
            } else {
                write_cell(book, index, addr, spans.get(&addr).copied(), styles, out);
            }
            col = target + 1;
        }
        if col == 0 {
            out.push_str("<table:table-cell/>");
        }
        out.push_str("</table:table-row>");
    }
    out.push_str("</table:table>");
}

fn cells_of(start: CellAddr, end: CellAddr) -> impl Iterator<Item = CellAddr> {
    (start.row()..=end.row()).flat_map(move |row| {
        (start.col()..=end.col()).map(move |col| CellAddr::new(row, col).expect("inside the merge"))
    })
}

fn write_cell(
    book: &Workbook,
    index: usize,
    addr: CellAddr,
    span: Option<(u32, u32)>,
    styles: &mut Styles,
    out: &mut String,
) {
    let cell = book.sheets()[index].get(addr).expect("listed cells exist");
    let style = cell.style();
    let format = numbers(style);
    let mut attributes = String::new();
    if *style != Style::default() || format.is_some() {
        let name = styles.cell(style, format);
        let _ = write!(attributes, r#" table:style-name="{name}""#);
    }
    if let Some((rows, cols)) = span {
        let _ = write!(
            attributes,
            r#" table:number-rows-spanned="{rows}" table:number-columns-spanned="{cols}""#
        );
    }
    if let Some(text) = &cell.formula
        && let Ok(expr) = formula::parse(&text[1..], &EN)
    {
        let expr = book.expand_tables(&expr, index, addr);
        let _ = write!(
            attributes,
            r#" table:formula="of:={}""#,
            escape(&openformula(&expr))
        );
    }
    let mut display = String::new();
    match &cell.value {
        CellValue::Number(n) => match format {
            Some(DataStyle::Date { .. }) => match from_serial(*n) {
                Some(date) => {
                    let _ = write!(
                        attributes,
                        r#" office:value-type="date" office:date-value="{:04}-{:02}-{:02}T{:02}:{:02}:{:02}""#,
                        date.year, date.month, date.day, date.hour, date.minute, date.second
                    );
                    display = format!("{:02}.{:02}.{:04}", date.day, date.month, date.year);
                }
                None => {
                    let _ = write!(
                        attributes,
                        r#" office:value-type="float" office:value="{n}""#
                    );
                    display = n.to_string();
                }
            },
            Some(DataStyle::Number { percent: true, .. }) => {
                let _ = write!(
                    attributes,
                    r#" office:value-type="percentage" office:value="{n}""#
                );
                display = format!("{}%", n * 100.0);
            }
            _ => {
                let _ = write!(
                    attributes,
                    r#" office:value-type="float" office:value="{n}""#
                );
                display = n.to_string();
            }
        },
        CellValue::Text(text) => {
            let _ = write!(attributes, r#" office:value-type="string""#);
            display = text.clone();
        }
        CellValue::Bool(b) => {
            let _ = write!(
                attributes,
                r#" office:value-type="boolean" office:boolean-value="{b}""#
            );
            display = if *b { "TRUE" } else { "FALSE" }.to_owned();
        }
        CellValue::Error(e) => display = EN.error_name(*e).to_owned(),
        CellValue::Empty => {}
    }
    let _ = write!(out, "<table:table-cell{attributes}>");
    if !display.is_empty() {
        for line in display.split('\n') {
            let _ = write!(out, "<text:p>{}</text:p>", escape(line));
        }
    }
    out.push_str("</table:table-cell>");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DataStyle {
    Date {
        time: bool,
    },
    Number {
        decimals: u8,
        grouping: bool,
        percent: bool,
    },
}

fn numbers(style: &Style) -> Option<DataStyle> {
    let code = style.number_format.as_str();
    match numfmt::kind(code) {
        Kind::Date => Some(DataStyle::Date {
            time: code.to_lowercase().contains('h'),
        }),
        Kind::Number => {
            let first = code.split(';').next().unwrap_or(code);
            let decimals = first
                .split_once('.')
                .map_or(0, |(_, rest)| rest.chars().filter(|c| *c == '0').count())
                as u8;
            Some(DataStyle::Number {
                decimals,
                grouping: first.contains(','),
                percent: first.contains('%'),
            })
        }
        _ => None,
    }
}

#[derive(Default)]
struct Styles {
    cells: Vec<(Style, Option<String>)>,
    data: Vec<DataStyle>,
    columns: Vec<String>,
    rows: Vec<String>,
}

impl Styles {
    fn cell(&mut self, style: &Style, format: Option<DataStyle>) -> String {
        let data = format.map(|format| self.data_style(format));
        let key = (style.clone(), data);
        let index = match self.cells.iter().position(|c| *c == key) {
            Some(index) => index,
            None => {
                self.cells.push(key);
                self.cells.len() - 1
            }
        };
        format!("ce{}", index + 1)
    }

    fn data_style(&mut self, format: DataStyle) -> String {
        let index = match self.data.iter().position(|d| *d == format) {
            Some(index) => index,
            None => {
                self.data.push(format);
                self.data.len() - 1
            }
        };
        format!("N{}", index + 1)
    }

    fn column(&mut self, width: f64) -> String {
        let pixels = (width * 7.0 + 5.0).round();
        let value = format!("{}pt", pixels * POINTS_PER_PIXEL);
        named(&mut self.columns, value, "co")
    }

    fn row(&mut self, points: f64) -> String {
        named(&mut self.rows, format!("{points}pt"), "ro")
    }

    fn xml(&self) -> String {
        let mut out = String::new();
        for (index, format) in self.data.iter().enumerate() {
            let name = format!("N{}", index + 1);
            match *format {
                DataStyle::Date { time } => {
                    let _ = write!(
                        out,
                        r#"<number:date-style style:name="{name}"><number:day number:style="long"/><number:text>.</number:text><number:month number:style="long"/><number:text>.</number:text><number:year number:style="long"/>"#
                    );
                    if time {
                        out.push_str(r#"<number:text> </number:text><number:hours number:style="long"/><number:text>:</number:text><number:minutes number:style="long"/>"#);
                    }
                    out.push_str("</number:date-style>");
                }
                DataStyle::Number {
                    decimals,
                    grouping,
                    percent,
                } => {
                    let kind = if percent {
                        "percentage-style"
                    } else {
                        "number-style"
                    };
                    let _ = write!(
                        out,
                        r#"<number:{kind} style:name="{name}"><number:number number:decimal-places="{decimals}" number:min-decimal-places="{decimals}" number:min-integer-digits="1" number:grouping="{grouping}"/>"#
                    );
                    if percent {
                        out.push_str("<number:text>%</number:text>");
                    }
                    let _ = write!(out, "</number:{kind}>");
                }
            }
        }
        for (index, width) in self.columns.iter().enumerate() {
            let _ = write!(
                out,
                r#"<style:style style:name="co{}" style:family="table-column"><style:table-column-properties style:column-width="{width}"/></style:style>"#,
                index + 1
            );
        }
        for (index, height) in self.rows.iter().enumerate() {
            let _ = write!(
                out,
                r#"<style:style style:name="ro{}" style:family="table-row"><style:table-row-properties style:row-height="{height}" style:use-optimal-row-height="false"/></style:style>"#,
                index + 1
            );
        }
        for (index, (style, data)) in self.cells.iter().enumerate() {
            let data = data
                .as_ref()
                .map(|name| format!(r#" style:data-style-name="{name}""#))
                .unwrap_or_default();
            let _ = write!(
                out,
                r#"<style:style style:name="ce{}" style:family="table-cell"{data}>{}</style:style>"#,
                index + 1,
                cell_properties(style)
            );
        }
        out
    }
}

fn named(list: &mut Vec<String>, value: String, prefix: &str) -> String {
    let index = match list.iter().position(|v| *v == value) {
        Some(index) => index,
        None => {
            list.push(value);
            list.len() - 1
        }
    };
    format!("{prefix}{}", index + 1)
}

fn cell_properties(style: &Style) -> String {
    let mut cell = String::new();
    if let Some(fill) = style.fill {
        let _ = write!(cell, r##" fo:background-color="#{}""##, fill.hex());
    }
    if style.wrap {
        cell.push_str(r#" fo:wrap-option="wrap""#);
    }
    let vertical = match style.v_align {
        VAlign::Top | VAlign::Justify | VAlign::Distributed => "top",
        VAlign::Center => "middle",
        VAlign::Bottom => "bottom",
    };
    let _ = write!(cell, r#" style:vertical-align="{vertical}""#);
    for (side, border) in [
        ("top", style.borders.top),
        ("bottom", style.borders.bottom),
        ("left", style.borders.left),
        ("right", style.borders.right),
    ] {
        if let Some(border) = border {
            let _ = write!(cell, r#" fo:border-{side}="{}""#, border_text(border));
        }
    }
    let mut out = format!("<style:table-cell-properties{cell}/>");
    let align = match style.h_align {
        HAlign::General | HAlign::Fill => None,
        HAlign::Left => Some("start"),
        HAlign::Center | HAlign::CenterAcross => Some("center"),
        HAlign::Right => Some("end"),
        HAlign::Justify | HAlign::Distributed => Some("justify"),
    };
    if let Some(align) = align {
        let _ = write!(
            out,
            r#"<style:paragraph-properties fo:text-align="{align}"/>"#
        );
    }
    let mut text = format!(
        r#" fo:font-family="{}" fo:font-size="{}pt""#,
        escape(&style.font),
        f64::from(style.size) / 10.0
    );
    if style.bold {
        text.push_str(r#" fo:font-weight="bold""#);
    }
    if style.italic {
        text.push_str(r#" fo:font-style="italic""#);
    }
    if style.underline != Underline::None {
        let kind = if style.underline.is_double() {
            "double"
        } else {
            "single"
        };
        let _ = write!(
            text,
            r#" style:text-underline-style="solid" style:text-underline-type="{kind}" style:text-underline-width="auto" style:text-underline-color="font-color""#
        );
    }
    if style.strike {
        text.push_str(r#" style:text-line-through-style="solid""#);
    }
    if let Some(color) = style.color {
        let _ = write!(text, r##" fo:color="#{}""##, color.hex());
    }
    let _ = write!(out, "<style:text-properties{text}/>");
    out
}

fn border_text(border: Border) -> String {
    let width = match border.style.weight() {
        1 => "0.75pt",
        2 => "1.5pt",
        _ => "2.25pt",
    };
    let line = match border.style {
        LineStyle::Double => "double",
        LineStyle::Hair | LineStyle::Dotted => "dotted",
        style if !style.dashes().is_empty() => "dashed",
        _ => "solid",
    };
    format!("{width} {line} #{}", border.color.hex())
}

pub fn openformula(expr: &Expr) -> String {
    let mut out = String::new();
    write_expr(expr, &mut out);
    out
}

fn write_expr(expr: &Expr, out: &mut String) {
    match expr {
        Expr::Number(n) => {
            let _ = write!(out, "{n}");
        }
        Expr::Text(s) => {
            let _ = write!(out, "\"{}\"", s.replace('"', "\"\""));
        }
        Expr::Bool(b) => out.push_str(if *b { "TRUE()" } else { "FALSE()" }),
        Expr::Error(e) => out.push_str(EN.error_name(*e)),
        Expr::Reference { sheet, reference } => {
            out.push('[');
            let prefix = sheet
                .as_ref()
                .map(|name| format!("'{}'", name.replace('\'', "''")))
                .unwrap_or_default();
            write_reference(&prefix, *reference, out);
            out.push(']');
        }
        Expr::Name(name) => out.push_str(name),
        Expr::Structured(_) => out.push_str(EN.error_name(vellum_grid_core::CellError::Ref)),
        Expr::Missing => {}
        Expr::Paren(inner) => {
            out.push('(');
            write_expr(inner, out);
            out.push(')');
        }
        Expr::Neg(inner) => {
            out.push('-');
            write_expr(inner, out);
        }
        Expr::Plus(inner) => {
            out.push('+');
            write_expr(inner, out);
        }
        Expr::Percent(inner) => {
            write_expr(inner, out);
            out.push('%');
        }
        Expr::Binary(op, left, right) => {
            write_expr(left, out);
            out.push_str(binary_symbol(*op));
            write_expr(right, out);
        }
        Expr::Call(name, args) => {
            let name = match name.as_str() {
                "CONCAT" | "XLOOKUP" | "IFS" | "SWITCH" | "TEXTJOIN" | "MAXIFS" | "MINIFS" => {
                    format!("COM.MICROSOFT.{name}")
                }
                _ => name.clone(),
            };
            out.push_str(&name);
            out.push('(');
            for (index, arg) in args.iter().enumerate() {
                if index > 0 {
                    out.push(';');
                }
                write_expr(arg, out);
            }
            out.push(')');
        }
    }
}

fn binary_symbol(op: formula::BinOp) -> &'static str {
    use formula::BinOp::*;
    match op {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Pow => "^",
        Concat => "&",
        Eq => "=",
        Ne => "<>",
        Lt => "<",
        Le => "<=",
        Gt => ">",
        Ge => ">=",
    }
}

fn write_reference(sheet: &str, reference: Reference, out: &mut String) {
    let col = |c: Coord| {
        format!(
            "{}{}",
            if c.absolute { "$" } else { "" },
            column_name(c.index)
        )
    };
    let row = |c: Coord| format!("{}{}", if c.absolute { "$" } else { "" }, c.index + 1);
    match reference {
        Reference::Cell { row: r, col: c } => {
            let _ = write!(out, "{sheet}.{}{}", col(c), row(r));
        }
        Reference::Area {
            top,
            left,
            bottom,
            right,
        } => {
            let _ = write!(
                out,
                "{sheet}.{}{}:.{}{}",
                col(left),
                row(top),
                col(right),
                row(bottom)
            );
        }
        Reference::Cols { left, right } => {
            let _ = write!(out, "{sheet}.{}:.{}", col(left), col(right));
        }
        Reference::Rows { top, bottom } => {
            let _ = write!(out, "{sheet}.{}:.{}", row(top), row(bottom));
        }
    }
}

pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::read_workbook;
    use vellum_grid_core::Sheet;

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    #[test]
    fn formulas_become_openformula() {
        let expr = formula::parse("SUM(A1:B2,$C$3)+Veri!D4&\"x\"", &EN).unwrap();
        assert_eq!(
            openformula(&expr),
            "SUM([.A1:.B2];[.$C$3])+['Veri'.D4]&\"x\""
        );
        let expr = formula::parse("IF(TRUE,A:A,3:4)", &EN).unwrap();
        assert_eq!(openformula(&expr), "IF(TRUE();[.A:.A];[.3:.4])");
    }

    #[test]
    fn names_become_named_ranges_and_expressions() {
        let mut book = Workbook::from_sheets(vec![Sheet::new("Veri")]).unwrap();
        book.add_name(vellum_grid_core::DefinedName::new(
            "Fiyat",
            "Veri!$A$1:$A$3",
        ))
        .unwrap();
        book.add_name(vellum_grid_core::DefinedName::new("Kdv", "SUM(Fiyat)*0.2"))
            .unwrap();
        let mut out = String::new();
        write_names(&book, &mut out);
        assert_eq!(
            out,
            "<table:named-expressions>\
             <table:named-range table:name=\"Fiyat\" table:base-cell-address=\"$&apos;Veri&apos;.$A$1\" table:cell-range-address=\"$&apos;Veri&apos;.$A$1:.$A$3\"/>\
             <table:named-expression table:name=\"Kdv\" table:base-cell-address=\"$&apos;Veri&apos;.$A$1\" table:expression=\"of:=SUM(Fiyat)*0.2\"/>\
             </table:named-expressions>"
        );
    }

    #[test]
    fn written_ods_reads_back() {
        let mut sheet = Sheet::new("Veri");
        sheet.set_input(addr("A1"), "Ad & Soyad");
        sheet.set_input(addr("A2"), "12.5");
        sheet.set_input(addr("A3"), "=A2*2");
        sheet.set_input(addr("C2"), "TRUE");
        sheet.layout_mut().merges.push("A5:B5".parse().unwrap());
        sheet.set_input(addr("A5"), "birleşik");
        let bold = Style {
            bold: true,
            number_format: "0.00".to_owned(),
            ..Style::default()
        };
        let mut cell = sheet.get(addr("A2")).cloned().unwrap();
        cell.set_style(bold);
        sheet.set(addr("A2"), cell);
        let book = Workbook::from_sheets(vec![sheet, Sheet::new("Boş")]).unwrap();
        let bytes = write_ods(&book).unwrap();
        let back = read_workbook(bytes).unwrap();
        assert_eq!(back.sheets().len(), 2);
        let sheet = &back.sheets()[0];
        assert_eq!(sheet.name(), "Veri");
        assert_eq!(
            *sheet.value(addr("A1")),
            CellValue::Text("Ad & Soyad".into())
        );
        assert_eq!(*sheet.value(addr("A2")), CellValue::Number(12.5));
        assert_eq!(
            sheet.get(addr("A3")).unwrap().formula.as_deref(),
            Some("=A2*2")
        );
        assert_eq!(*sheet.value(addr("C2")), CellValue::Bool(true));
        assert_eq!(*sheet.value(addr("A5")), CellValue::Text("birleşik".into()));
    }
}
