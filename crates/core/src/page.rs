use std::collections::BTreeSet;

use crate::{CellAddr, CellRange, Sheet};

pub const POINTS_PER_INCH: f64 = 72.0;
const POINTS_PER_PIXEL: f64 = 0.75;
const PIXELS_PER_CHAR: f64 = 7.0;
const MIN_SCALE: f64 = 0.1;
const FIT_STEP: f64 = 0.97;
const FIT_ATTEMPTS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Paper {
    #[default]
    A4,
    A3,
    A5,
    Letter,
    Legal,
}

impl Paper {
    pub const ALL: [Self; 5] = [Self::A4, Self::A3, Self::A5, Self::Letter, Self::Legal];

    pub fn size(self) -> (f64, f64) {
        match self {
            Self::A4 => (595.28, 841.89),
            Self::A3 => (841.89, 1190.55),
            Self::A5 => (419.53, 595.28),
            Self::Letter => (612.0, 792.0),
            Self::Legal => (612.0, 1008.0),
        }
    }

    pub fn code(self) -> u8 {
        match self {
            Self::Letter => 1,
            Self::Legal => 5,
            Self::A3 => 8,
            Self::A4 => 9,
            Self::A5 => 11,
        }
    }

    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|p| u32::from(p.code()) == code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    pub header: f64,
    pub footer: f64,
}

impl Margins {
    pub const NORMAL: Self = Self {
        left: 0.7,
        right: 0.7,
        top: 0.75,
        bottom: 0.75,
        header: 0.3,
        footer: 0.3,
    };
    pub const WIDE: Self = Self {
        left: 1.0,
        right: 1.0,
        top: 1.0,
        bottom: 1.0,
        header: 0.5,
        footer: 0.5,
    };
    pub const NARROW: Self = Self {
        left: 0.25,
        right: 0.25,
        top: 0.75,
        bottom: 0.75,
        header: 0.3,
        footer: 0.3,
    };
}

impl Default for Margins {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Percent(u16),
    Fit { width: u16, height: u16 },
}

impl Default for Scale {
    fn default() -> Self {
        Self::Percent(100)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageSetup {
    pub paper: Paper,
    pub landscape: bool,
    pub margins: Margins,
    pub scale: Scale,
    pub print_area: Option<CellRange>,
    pub title_rows: Option<(u32, u32)>,
    pub title_cols: Option<(u32, u32)>,
    pub header: String,
    pub footer: String,
    pub gridlines: bool,
    pub headings: bool,
    pub center_horizontally: bool,
    pub center_vertically: bool,
    pub row_breaks: BTreeSet<u32>,
    pub col_breaks: BTreeSet<u32>,
}

impl PageSetup {
    pub fn paper_size(&self) -> (f64, f64) {
        let (w, h) = self.paper.size();
        if self.landscape { (h, w) } else { (w, h) }
    }

    pub fn printable(&self) -> (f64, f64) {
        let (w, h) = self.paper_size();
        let m = self.margins;
        (
            (w - (m.left + m.right) * POINTS_PER_INCH).max(POINTS_PER_INCH),
            (h - (m.top + m.bottom) * POINTS_PER_INCH).max(POINTS_PER_INCH),
        )
    }
}

pub fn column_points(width: f64) -> f64 {
    (width * PIXELS_PER_CHAR).round() * POINTS_PER_PIXEL
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    pub rows: (u32, u32),
    pub cols: (u32, u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pagination {
    pub scale: f64,
    pub area: CellRange,
    pub pages: Vec<Page>,
}

pub fn print_range(sheet: &Sheet) -> Option<CellRange> {
    if let Some(area) = sheet.layout().page.print_area {
        return Some(area);
    }
    let (rows, cols) = sheet.extent();
    if rows == 0 {
        return None;
    }
    Some(CellRange::new(
        CellAddr::new(0, 0).expect("A1 is valid"),
        CellAddr::new(rows - 1, cols - 1).expect("extent is inside the sheet"),
    ))
}

pub fn row_points(sheet: &Sheet, row: u32) -> f64 {
    let layout = sheet.layout();
    if layout.hidden_rows.contains(&row) {
        0.0
    } else {
        layout.row_height(row)
    }
}

pub fn col_points(sheet: &Sheet, col: u32) -> f64 {
    let layout = sheet.layout();
    if layout.hidden_cols.contains(&col) {
        0.0
    } else {
        column_points(layout.col_width(col))
    }
}

pub fn paginate(sheet: &Sheet) -> Option<Pagination> {
    let area = print_range(sheet)?;
    let setup = &sheet.layout().page;
    let (width, height) = setup.printable();
    let rows = (area.start().row(), area.end().row());
    let cols = (area.start().col(), area.end().col());
    let row_size = |row| row_points(sheet, row);
    let col_size = |col| col_points(sheet, col);
    let titles = |span: Option<(u32, u32)>, size: &dyn Fn(u32) -> f64| {
        span.map_or(0.0, |(a, b)| (a..=b).map(size).sum::<f64>())
    };
    let title_height = titles(setup.title_rows, &row_size);
    let title_width = titles(setup.title_cols, &col_size);
    let layout = |scale: f64| {
        let row_chunks = split(
            rows,
            &row_size,
            height / scale,
            title_height,
            setup.title_rows,
            &setup.row_breaks,
        );
        let col_chunks = split(
            cols,
            &col_size,
            width / scale,
            title_width,
            setup.title_cols,
            &setup.col_breaks,
        );
        (row_chunks, col_chunks)
    };
    let mut scale = match setup.scale {
        Scale::Percent(p) => (f64::from(p) / 100.0).max(MIN_SCALE),
        Scale::Fit { .. } => 1.0,
    };
    let (mut row_chunks, mut col_chunks) = layout(scale);
    if let Scale::Fit {
        width: wide,
        height: tall,
    } = setup.scale
    {
        let fits = |r: &Vec<(u32, u32)>, c: &Vec<(u32, u32)>| {
            (wide == 0 || c.len() <= usize::from(wide))
                && (tall == 0 || r.len() <= usize::from(tall))
        };
        for _ in 0..FIT_ATTEMPTS {
            if fits(&row_chunks, &col_chunks) || scale <= MIN_SCALE {
                break;
            }
            scale = (scale * FIT_STEP).max(MIN_SCALE);
            (row_chunks, col_chunks) = layout(scale);
        }
    }
    let pages = col_chunks
        .iter()
        .flat_map(|&cols| row_chunks.iter().map(move |&rows| Page { rows, cols }))
        .collect();
    Some(Pagination { scale, area, pages })
}

fn split(
    (first, last): (u32, u32),
    size: &dyn Fn(u32) -> f64,
    available: f64,
    titles: f64,
    title_span: Option<(u32, u32)>,
    breaks: &BTreeSet<u32>,
) -> Vec<(u32, u32)> {
    let mut chunks = Vec::new();
    let mut start = first;
    let mut used = 0.0;
    let room = |start: u32| match title_span {
        Some((_, end)) if start > end => available - titles,
        _ => available,
    };
    for index in first..=last {
        let next = size(index);
        if index > start && (breaks.contains(&index) || used + next > room(start)) {
            chunks.push((start, index - 1));
            start = index;
            used = 0.0;
        }
        used += next;
    }
    chunks.push((start, last));
    chunks
}

pub struct Context<'a> {
    pub page: usize,
    pub pages: usize,
    pub sheet: &'a str,
    pub book: &'a str,
    pub date: &'a str,
    pub time: &'a str,
}

pub fn header_sections(code: &str, context: &Context) -> [String; 3] {
    let mut sections = [String::new(), String::new(), String::new()];
    let mut current = 1;
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            sections[current].push(c);
            continue;
        }
        let Some(code) = chars.next() else {
            break;
        };
        let out = &mut sections[current];
        match code {
            'L' => current = 0,
            'C' => current = 1,
            'R' => current = 2,
            'P' => out.push_str(&context.page.to_string()),
            'N' => out.push_str(&context.pages.to_string()),
            'A' => out.push_str(context.sheet),
            'F' => out.push_str(context.book),
            'D' => out.push_str(context.date),
            'T' => out.push_str(context.time),
            '&' => out.push('&'),
            '"' => {
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                }
            }
            c if c.is_ascii_digit() => {
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    chars.next();
                }
            }
            _ => {}
        }
    }
    sections
}

pub fn split_sections(code: &str) -> [String; 3] {
    let mut sections = [String::new(), String::new(), String::new()];
    let mut current = 1;
    let mut chars = code.chars();
    while let Some(c) = chars.next() {
        if c != '&' {
            sections[current].push(c);
            continue;
        }
        match chars.next() {
            Some('L') => current = 0,
            Some('C') => current = 1,
            Some('R') => current = 2,
            Some(other) => {
                sections[current].push('&');
                sections[current].push(other);
            }
            None => sections[current].push('&'),
        }
    }
    sections
}

pub fn join_sections(sections: &[String; 3]) -> String {
    ["&L", "&C", "&R"]
        .iter()
        .zip(sections)
        .filter(|(_, text)| !text.is_empty())
        .map(|(code, text)| format!("{code}{text}"))
        .collect()
}

pub fn parse_span(text: &str, rows: bool) -> Option<(u32, u32)> {
    let text = text.replace('$', "");
    let text = text
        .rsplit_once('!')
        .map_or(text.as_str(), |(_, r)| r)
        .trim();
    let (first, last) = text.split_once(':').unwrap_or((text, text));
    let index = |s: &str| -> Option<u32> {
        if rows {
            s.trim()
                .parse::<u32>()
                .ok()
                .filter(|r| *r >= 1)
                .map(|r| r - 1)
        } else {
            crate::column_index(&s.trim().to_uppercase()).ok()
        }
    };
    let (a, b) = (index(first)?, index(last)?);
    Some((a.min(b), a.max(b)))
}

pub fn span_text((first, last): (u32, u32), rows: bool) -> String {
    let name = |i: u32| {
        if rows {
            format!("${}", i + 1)
        } else {
            format!("${}", crate::column_name(i))
        }
    };
    format!("{}:{}", name(first), name(last))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn filled(rows: u32, cols: u32) -> Sheet {
        let mut sheet = Sheet::new("S");
        let last = CellAddr::new(rows - 1, cols - 1).unwrap();
        sheet.set_input(addr("A1"), "x");
        sheet.set_input(last, "y");
        sheet
    }

    #[test]
    fn default_columns_are_48_points() {
        assert_eq!(column_points(crate::DEFAULT_COLUMN_WIDTH), 48.0);
    }

    #[test]
    fn pages_go_down_then_over() {
        let sheet = filled(100, 12);
        let pagination = paginate(&sheet).unwrap();
        let (width, height) = sheet.layout().page.printable();
        let per_page_rows = (height / 15.0) as u32;
        let per_page_cols = (width / 48.0) as u32;
        assert_eq!(pagination.pages[0].rows, (0, per_page_rows - 1));
        assert_eq!(pagination.pages[0].cols, (0, per_page_cols - 1));
        assert_eq!(pagination.pages[1].cols, (0, per_page_cols - 1));
        let row_pages = 100_u32.div_ceil(per_page_rows) as usize;
        assert_eq!(pagination.pages[row_pages].cols.0, per_page_cols);
    }

    #[test]
    fn manual_breaks_and_fit_to_page() {
        let mut sheet = filled(30, 3);
        sheet.layout_mut().page.row_breaks.insert(10);
        let pagination = paginate(&sheet).unwrap();
        assert_eq!(pagination.pages.len(), 2);
        assert_eq!(pagination.pages[0].rows, (0, 9));
        let mut sheet = filled(200, 30);
        sheet.layout_mut().page.scale = Scale::Fit {
            width: 1,
            height: 1,
        };
        let pagination = paginate(&sheet).unwrap();
        assert_eq!(pagination.pages.len(), 1);
        assert!(pagination.scale < 0.5);
    }

    #[test]
    fn landscape_swaps_the_paper() {
        let mut setup = PageSetup::default();
        let (w, h) = setup.paper_size();
        setup.landscape = true;
        assert_eq!(setup.paper_size(), (h, w));
        assert_eq!(Paper::from_code(9), Some(Paper::A4));
    }

    #[test]
    fn sections_split_and_join() {
        let parts = split_sections("&LSol&CSayfa &P&RSağ && ok");
        assert_eq!(
            parts,
            [
                "Sol".to_owned(),
                "Sayfa &P".to_owned(),
                "Sağ && ok".to_owned()
            ]
        );
        assert_eq!(join_sections(&parts), "&LSol&CSayfa &P&RSağ && ok");
        assert_eq!(
            join_sections(&[String::new(), "X".into(), String::new()]),
            "&CX"
        );
        assert_eq!(parse_span("$1:$2", true), Some((0, 1)));
        assert_eq!(parse_span("'Veri'!$B:$A", false), Some((0, 1)));
        assert_eq!(parse_span("3", true), Some((2, 2)));
        assert_eq!(span_text((0, 1), true), "$1:$2");
        assert_eq!(span_text((2, 2), false), "$C:$C");
    }

    #[test]
    fn header_codes_expand() {
        let context = Context {
            page: 2,
            pages: 5,
            sheet: "Veri",
            book: "Bütçe",
            date: "19.10.2026",
            time: "14:30",
        };
        assert_eq!(
            header_sections("&L&\"Arial,Bold\"&12&A&CSayfa &P / &N&R&D &T", &context),
            [
                "Veri".to_owned(),
                "Sayfa 2 / 5".to_owned(),
                "19.10.2026 14:30".to_owned()
            ]
        );
        assert_eq!(
            header_sections("Rapor && Özet", &context),
            [String::new(), "Rapor & Özet".to_owned(), String::new()]
        );
    }
}
