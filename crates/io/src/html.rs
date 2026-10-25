use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use vellum_grid_core::page::{col_points, row_points};
use vellum_grid_core::{CellAddr, CellValue, HAlign, Locale, Sheet, Style, VAlign};

pub fn write_html(sheet: &Sheet, title: &str, locale: &Locale) -> String {
    let layout = sheet.layout();
    let (rows, cols) = sheet.extent();
    let mut spans: HashMap<CellAddr, (u32, u32)> = HashMap::new();
    let mut covered: HashSet<CellAddr> = HashSet::new();
    for merge in &layout.merges {
        spans.insert(merge.start(), (merge.rows(), merge.cols()));
        for row in merge.start().row()..=merge.end().row() {
            for col in merge.start().col()..=merge.end().col() {
                let addr = CellAddr::new(row, col).expect("inside the merge");
                if addr != merge.start() {
                    covered.insert(addr);
                }
            }
        }
    }
    let mut out = String::new();
    let _ = write!(
        out,
        "<!DOCTYPE html>\n<html lang=\"tr\">\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n<style>table{{border-collapse:collapse;table-layout:fixed;font-family:Calibri,Carlito,sans-serif;font-size:11pt}}td{{border:1px solid #d9d9d9;padding:0 3px;overflow:hidden;white-space:nowrap;vertical-align:bottom}}</style>\n</head>\n<body>\n<table>\n",
        escape(title)
    );
    for col in 0..cols {
        let _ = writeln!(out, "<col style=\"width:{}pt\">", col_points(sheet, col));
    }
    for row in 0..rows {
        if layout.hidden_rows.contains(&row) {
            continue;
        }
        let _ = write!(out, "<tr style=\"height:{}pt\">", row_points(sheet, row));
        for col in 0..cols {
            let addr = CellAddr::new(row, col).expect("inside the sheet");
            if covered.contains(&addr) || layout.hidden_cols.contains(&col) {
                continue;
            }
            out.push_str("<td");
            if let Some((rows, cols)) = spans.get(&addr) {
                let _ = write!(out, " rowspan=\"{rows}\" colspan=\"{cols}\"");
            }
            let Some(cell) = sheet.get(addr) else {
                out.push_str("></td>");
                continue;
            };
            let style = cell.style();
            let rendered = locale.render(&cell.value, style.number_format.as_str());
            let css = css(style, &cell.value, rendered.color.or(style.color));
            if !css.is_empty() {
                let _ = write!(out, " style=\"{css}\"");
            }
            out.push('>');
            out.push_str(&escape(&rendered.text).replace('\n', "<br>"));
            out.push_str("</td>");
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</table>\n</body>\n</html>\n");
    out
}

fn css(style: &Style, value: &CellValue, color: Option<vellum_grid_core::Color>) -> String {
    let mut css = String::new();
    if style.bold {
        css.push_str("font-weight:bold;");
    }
    if style.italic {
        css.push_str("font-style:italic;");
    }
    match (
        style.underline != vellum_grid_core::Underline::None,
        style.strike,
    ) {
        (true, true) => css.push_str("text-decoration:underline line-through;"),
        (true, false) => css.push_str("text-decoration:underline;"),
        (false, true) => css.push_str("text-decoration:line-through;"),
        _ => {}
    }
    if let Some(color) = color {
        let _ = write!(css, "color:#{};", color.hex());
    }
    if let Some(fill) = style.fill {
        let _ = write!(css, "background:#{};", fill.hex());
    }
    if style.size != vellum_grid_core::DEFAULT_SIZE {
        let _ = write!(css, "font-size:{}pt;", style.points());
    }
    if style.font != vellum_grid_core::DEFAULT_FONT {
        let _ = write!(css, "font-family:'{}';", escape(&style.font));
    }
    let align = match (style.h_align, value) {
        (HAlign::General, CellValue::Number(_)) | (HAlign::Right, _) => Some("right"),
        (HAlign::General, CellValue::Bool(_) | CellValue::Error(_))
        | (HAlign::Center | HAlign::CenterAcross, _) => Some("center"),
        (HAlign::Justify | HAlign::Distributed, _) => Some("justify"),
        _ => None,
    };
    if let Some(align) = align {
        let _ = write!(css, "text-align:{align};");
    }
    match style.v_align {
        VAlign::Top | VAlign::Justify | VAlign::Distributed => css.push_str("vertical-align:top;"),
        VAlign::Center => css.push_str("vertical-align:middle;"),
        VAlign::Bottom => {}
    }
    if style.wrap {
        css.push_str("white-space:normal;");
    }
    css
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_grid_core::TR;

    #[test]
    fn writes_values_styles_and_merges() {
        let mut sheet = Sheet::new("S");
        sheet.set_input("A1".parse().unwrap(), "Ad <b>");
        sheet.set_input("B1".parse().unwrap(), "1234.5");
        sheet.layout_mut().merges.push("A2:B2".parse().unwrap());
        sheet.set_input("A2".parse().unwrap(), "birleşik");
        sheet.set_input("B2".parse().unwrap(), "gizli");
        let html = write_html(&sheet, "Rapor", &TR);
        assert!(html.contains("<td>Ad &lt;b&gt;</td>"));
        assert!(html.contains("text-align:right;\">1234,5</td>"));
        assert!(html.contains("rowspan=\"1\" colspan=\"2\""));
        assert!(!html.contains("gizli"));
    }
}
