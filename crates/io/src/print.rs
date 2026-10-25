use std::collections::{HashMap, HashSet};

use vellum_grid_core::page::{self, Context, Page, Pagination};
use vellum_grid_core::table_style::table_look;
use vellum_grid_core::{
    Border, Cell, CellAddr, CellRange, CellValue, Color, DEFAULT_FONT, HAlign, Highlighter,
    LineStyle, Locale, Sheet, Style, VAlign, Workbook, column_name,
};

const PADDING: f32 = 1.5;
const LINE_FACTOR: f32 = 1.2;
const HEADING_WIDTH: f32 = 28.0;
const HEADING_HEIGHT: f32 = 15.0;
const HEADER_SIZE: f32 = 11.0;
const GRID_COLOR: Color = Color(0xBFBFBF);
const HEADING_FILL: Color = Color(0xF2F2F2);
const FALLBACK_ADVANCE: f32 = 0.5;
const FALLBACK_ASCENT: f32 = 0.75;
const FALLBACK_DESCENT: f32 = 0.25;

pub trait FontSource {
    fn load(&self, family: &str, bold: bool, italic: bool) -> Option<Vec<u8>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontKey {
    pub family: String,
    pub bold: bool,
    pub italic: bool,
}

pub struct Face {
    pub key: FontKey,
    pub data: Option<Vec<u8>>,
    units: f32,
    ascent: f32,
    descent: f32,
}

impl Face {
    fn parsed(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(self.data.as_ref()?, 0).ok()
    }

    pub fn glyph(&self, c: char) -> Option<u16> {
        Some(self.parsed()?.glyph_index(c)?.0)
    }

    pub fn advance(&self, glyph: u16) -> f32 {
        self.parsed()
            .and_then(|face| face.glyph_hor_advance(ttf_parser::GlyphId(glyph)))
            .map_or(FALLBACK_ADVANCE, |a| f32::from(a) / self.units)
    }

    pub fn units(&self) -> f32 {
        self.units
    }

    pub fn ascent(&self) -> f32 {
        self.ascent
    }

    pub fn descent(&self) -> f32 {
        self.descent
    }
}

pub struct FontBook<'a> {
    source: &'a dyn FontSource,
    faces: Vec<Face>,
    index: HashMap<FontKey, usize>,
}

impl<'a> FontBook<'a> {
    pub fn new(source: &'a dyn FontSource) -> Self {
        Self {
            source,
            faces: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    pub fn id(&mut self, family: &str, bold: bool, italic: bool) -> usize {
        let key = FontKey {
            family: family.to_owned(),
            bold,
            italic,
        };
        if let Some(&id) = self.index.get(&key) {
            return id;
        }
        let data = [
            (family, bold, italic),
            (family, bold, false),
            (family, false, false),
            (DEFAULT_FONT, bold, italic),
            (DEFAULT_FONT, false, false),
        ]
        .into_iter()
        .find_map(|(f, b, i)| self.source.load(f, b, i));
        let metrics = data
            .as_ref()
            .and_then(|d| ttf_parser::Face::parse(d, 0).ok())
            .map(|face| {
                let units = f32::from(face.units_per_em());
                (
                    units,
                    f32::from(face.ascender()) / units,
                    -f32::from(face.descender()) / units,
                )
            });
        let (units, ascent, descent) =
            metrics.unwrap_or((1000.0, FALLBACK_ASCENT, FALLBACK_DESCENT));
        let data = metrics.and(data);
        self.faces.push(Face {
            key: key.clone(),
            data,
            units,
            ascent,
            descent,
        });
        let id = self.faces.len() - 1;
        self.index.insert(key, id);
        id
    }

    pub fn face(&self, id: usize) -> &Face {
        &self.faces[id]
    }

    pub fn width(&self, id: usize, text: &str, size: f32) -> f32 {
        let face = &self.faces[id];
        let Some(parsed) = face.parsed() else {
            return text.chars().count() as f32 * FALLBACK_ADVANCE * size;
        };
        text.chars()
            .map(|c| {
                parsed
                    .glyph_index(c)
                    .and_then(|g| parsed.glyph_hor_advance(g))
                    .map_or(FALLBACK_ADVANCE, |a| f32::from(a) / face.units)
            })
            .sum::<f32>()
            * size
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    fn right(self) -> f32 {
        self.x + self.w
    }

    fn bottom(self) -> f32 {
        self.y + self.h
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Fill {
        area: Area,
        color: Color,
    },
    Line {
        from: (f32, f32),
        to: (f32, f32),
        width: f32,
        color: Color,
        dash: Option<(f32, f32)>,
    },
    Text {
        x: f32,
        baseline: f32,
        text: String,
        font: usize,
        size: f32,
        color: Color,
        underline: bool,
        strike: bool,
        clip: Option<Area>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrintPage {
    pub width: f32,
    pub height: f32,
    pub items: Vec<Item>,
}

pub struct Job<'a> {
    pub book: &'a str,
    pub date: &'a str,
    pub time: &'a str,
    pub locale: &'a Locale,
    pub today: f64,
}

pub fn layout(
    book: &Workbook,
    sheets: &[usize],
    selection: Option<CellRange>,
    job: &Job,
    fonts: &mut FontBook,
) -> Vec<PrintPage> {
    let mut plans: Vec<(usize, Pagination)> = Vec::new();
    for &index in sheets {
        let sheet = &book.sheets()[index];
        let pagination = match selection {
            Some(selection) => {
                let mut copy = sheet.clone();
                copy.layout_mut().page.print_area = Some(selection);
                page::paginate(&copy)
            }
            None => page::paginate(sheet),
        };
        if let Some(pagination) = pagination {
            plans.push((index, pagination));
        }
    }
    let total: usize = plans.iter().map(|(_, p)| p.pages.len()).sum();
    let mut out = Vec::new();
    let mut number = 0;
    for (index, pagination) in plans {
        let sheet = &book.sheets()[index];
        for page in &pagination.pages {
            number += 1;
            let context = Context {
                page: number,
                pages: total,
                sheet: sheet.name(),
                book: job.book,
                date: job.date,
                time: job.time,
            };
            out.push(render_page(
                book,
                index,
                &pagination,
                *page,
                &context,
                job,
                fonts,
            ));
        }
    }
    out
}

fn render_page(
    book: &Workbook,
    index: usize,
    pagination: &Pagination,
    page: Page,
    context: &Context,
    job: &Job,
    fonts: &mut FontBook,
) -> PrintPage {
    let sheet = &book.sheets()[index];
    let layout = sheet.layout();
    let setup = &layout.page;
    let (paper_w, paper_h) = setup.paper_size();
    let (width, height) = (paper_w as f32, paper_h as f32);
    let scale = pagination.scale as f32;
    let inch = page::POINTS_PER_INCH as f32;
    let m = setup.margins;
    let mut rows: Vec<u32> = Vec::new();
    if let Some((a, b)) = setup.title_rows
        && page.rows.0 > b
    {
        rows.extend(a..=b);
    }
    rows.extend(page.rows.0..=page.rows.1);
    let mut cols: Vec<u32> = Vec::new();
    if let Some((a, b)) = setup.title_cols
        && page.cols.0 > b
    {
        cols.extend(a..=b);
    }
    cols.extend(page.cols.0..=page.cols.1);
    let heading_w = if setup.headings {
        HEADING_WIDTH * scale
    } else {
        0.0
    };
    let heading_h = if setup.headings {
        HEADING_HEIGHT * scale
    } else {
        0.0
    };
    let col_w: Vec<f32> = cols
        .iter()
        .map(|&c| page::col_points(sheet, c) as f32 * scale)
        .collect();
    let row_h: Vec<f32> = rows
        .iter()
        .map(|&r| page::row_points(sheet, r) as f32 * scale)
        .collect();
    let content_w = heading_w + col_w.iter().sum::<f32>();
    let content_h = heading_h + row_h.iter().sum::<f32>();
    let printable_w = width - (m.left + m.right) as f32 * inch;
    let printable_h = height - (m.top + m.bottom) as f32 * inch;
    let mut x0 = m.left as f32 * inch;
    let mut y0 = m.top as f32 * inch;
    if setup.center_horizontally {
        x0 += ((printable_w - content_w) / 2.0).max(0.0);
    }
    if setup.center_vertically {
        y0 += ((printable_h - content_h) / 2.0).max(0.0);
    }
    let mut xs = Vec::with_capacity(cols.len() + 1);
    let mut x = x0 + heading_w;
    for w in &col_w {
        xs.push(x);
        x += w;
    }
    xs.push(x);
    let mut ys = Vec::with_capacity(rows.len() + 1);
    let mut y = y0 + heading_h;
    for h in &row_h {
        ys.push(y);
        y += h;
    }
    ys.push(y);
    let col_at: HashMap<u32, usize> = cols.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let row_at: HashMap<u32, usize> = rows.iter().enumerate().map(|(i, &r)| (r, i)).collect();
    let cell_area = |r: usize, c: usize| Area {
        x: xs[c],
        y: ys[r],
        w: xs[c + 1] - xs[c],
        h: ys[r + 1] - ys[r],
    };
    let mut items = Vec::new();
    if setup.gridlines {
        let line = |from, to| Item::Line {
            from,
            to,
            width: 0.5 * scale.max(0.5),
            color: GRID_COLOR,
            dash: None,
        };
        for &x in &xs {
            items.push(line((x, ys[0]), (x, ys[rows.len()])));
        }
        for &y in &ys {
            items.push(line((xs[0], y), (xs[cols.len()], y)));
        }
    }
    if setup.headings {
        let font = fonts.id(DEFAULT_FONT, false, false);
        let size = HEADER_SIZE * scale;
        let corner = Area {
            x: x0,
            y: y0,
            w: heading_w,
            h: heading_h,
        };
        items.push(Item::Fill {
            area: Area {
                w: content_w,
                ..corner
            },
            color: HEADING_FILL,
        });
        items.push(Item::Fill {
            area: Area {
                h: content_h,
                ..corner
            },
            color: HEADING_FILL,
        });
        for (i, &c) in cols.iter().enumerate() {
            let area = Area {
                x: xs[i],
                y: y0,
                w: xs[i + 1] - xs[i],
                h: heading_h,
            };
            centered(&mut items, fonts, font, size, &column_name(c), area);
        }
        for (i, &r) in rows.iter().enumerate() {
            let area = Area {
                x: x0,
                y: ys[i],
                w: heading_w,
                h: ys[i + 1] - ys[i],
            };
            centered(&mut items, fonts, font, size, &(r + 1).to_string(), area);
        }
    }

    let merges: Vec<CellRange> = layout
        .merges
        .iter()
        .copied()
        .filter(|m| {
            rows.iter()
                .any(|r| (m.start().row()..=m.end().row()).contains(r))
        })
        .filter(|m| {
            cols.iter()
                .any(|c| (m.start().col()..=m.end().col()).contains(c))
        })
        .collect();
    let merged_at = |addr: CellAddr| merges.iter().copied().find(|m| m.contains(addr));
    let merge_area = |m: CellRange| -> Option<Area> {
        let rs: Vec<usize> = (m.start().row()..=m.end().row())
            .filter_map(|r| row_at.get(&r).copied())
            .collect();
        let cs: Vec<usize> = (m.start().col()..=m.end().col())
            .filter_map(|c| col_at.get(&c).copied())
            .collect();
        let (r0, r1) = (*rs.first()?, *rs.last()?);
        let (c0, c1) = (*cs.first()?, *cs.last()?);
        Some(Area {
            x: xs[c0],
            y: ys[r0],
            w: xs[c1 + 1] - xs[c0],
            h: ys[r1 + 1] - ys[r0],
        })
    };
    let visible = CellRange::new(
        CellAddr::new(
            *rows.iter().min().unwrap_or(&0),
            *cols.iter().min().unwrap_or(&0),
        )
        .expect("rows and columns are inside the sheet"),
        CellAddr::new(
            *rows.iter().max().unwrap_or(&0),
            *cols.iter().max().unwrap_or(&0),
        )
        .expect("rows and columns are inside the sheet"),
    );
    let highlighter = Highlighter::new(book, index, visible, job.today);
    for m in &merges {
        if let Some(area) = merge_area(*m) {
            let fill = sheet
                .get(m.start())
                .and_then(|c| c.style().fill)
                .unwrap_or(Color::WHITE);
            items.push(Item::Fill { area, color: fill });
        }
    }
    let mut texts = Vec::new();
    let mut borders = Vec::new();
    for (ri, &row) in rows.iter().enumerate() {
        for (ci, &col) in cols.iter().enumerate() {
            let addr = CellAddr::new(row, col).expect("inside the sheet");
            let look = sheet
                .layout()
                .tables
                .iter()
                .find(|t| t.range.contains(addr))
                .and_then(|t| table_look(t, addr));
            let blank = Cell::default();
            let cell = match (sheet.get(addr), look) {
                (Some(cell), _) => cell,
                (None, Some(_)) => &blank,
                (None, None) => continue,
            };
            let merge = merged_at(addr);
            let area = match merge {
                Some(m) if m.start() == addr => merge_area(m).unwrap_or(cell_area(ri, ci)),
                Some(_) => continue,
                None => cell_area(ri, ci),
            };
            let mut style = cell.style().clone();
            if let Some(look) = look {
                look.apply(&mut style);
            }
            let style = &style;
            let highlight = highlighter.highlight(addr, &cell.value);
            let fill = highlight.fill.or(style.fill);
            if let (Some(fill), None) = (fill, merge) {
                items.push(Item::Fill { area, color: fill });
            }
            borders.push((area, style.borders, scale));
            if cell.value.is_empty() {
                continue;
            }
            let rendered = job.locale.render(&cell.value, style.number_format.as_str());
            let color = rendered
                .color
                .or(highlight.color)
                .or(style.color)
                .unwrap_or(Color::BLACK);
            let overflow = (merge.is_none() && !style.wrap)
                .then(|| overflow_area(sheet, &cols, &xs, ci, row, area, style, &cell.value));
            texts.push((
                area,
                rendered.text,
                style.clone(),
                color,
                cell.value.clone(),
                overflow,
            ));
        }
    }
    for (area, sides, scale) in borders {
        push_borders(&mut items, area, sides, scale);
    }
    for (area, text, style, color, value, overflow) in texts {
        push_text(
            &mut items,
            fonts,
            &TextCell {
                area,
                text: &text,
                style: &style,
                color,
                value: &value,
                scale,
                overflow: overflow.flatten(),
            },
        );
    }
    let font = fonts.id(DEFAULT_FONT, false, false);
    for (code, top) in [(&setup.header, true), (&setup.footer, false)] {
        if code.is_empty() {
            continue;
        }
        let sections = page::header_sections(code, context);
        let face = fonts.face(font);
        let baseline = if top {
            m.header as f32 * inch + face.ascent() * HEADER_SIZE
        } else {
            height - m.footer as f32 * inch - face.descent() * HEADER_SIZE
        };
        let left = m.left as f32 * inch;
        let right = width - m.right as f32 * inch;
        for (index, text) in sections.iter().enumerate() {
            if text.is_empty() {
                continue;
            }
            let w = fonts.width(font, text, HEADER_SIZE);
            let x = match index {
                0 => left,
                1 => (width - w) / 2.0,
                _ => right - w,
            };
            items.push(Item::Text {
                x,
                baseline,
                text: text.clone(),
                font,
                size: HEADER_SIZE,
                color: Color::BLACK,
                underline: false,
                strike: false,
                clip: None,
            });
        }
    }
    PrintPage {
        width,
        height,
        items,
    }
}

#[allow(clippy::too_many_arguments)]
fn overflow_area(
    sheet: &Sheet,
    cols: &[u32],
    xs: &[f32],
    ci: usize,
    row: u32,
    area: Area,
    style: &Style,
    value: &CellValue,
) -> Option<Area> {
    if !matches!(value, CellValue::Text(_)) {
        return None;
    }
    let empty = |i: usize| {
        let addr = CellAddr::new(row, cols[i]).expect("inside the sheet");
        sheet.get(addr).is_none_or(|c| c.value.is_empty())
            && sheet.layout().merge_at(addr).is_none()
    };
    let mut first = ci;
    let mut last = ci;
    let grow_right = matches!(
        style.h_align,
        HAlign::General | HAlign::Left | HAlign::Center
    );
    let grow_left = matches!(style.h_align, HAlign::Right | HAlign::Center);
    let grow_right = grow_right || style.h_align == HAlign::CenterAcross;
    if grow_right {
        while last + 1 < cols.len() && empty(last + 1) {
            last += 1;
        }
    }
    if grow_left {
        while first > 0 && empty(first - 1) {
            first -= 1;
        }
    }
    Some(Area {
        x: xs[first],
        y: area.y,
        w: xs[last + 1] - xs[first],
        h: area.h,
    })
}

fn push_borders(items: &mut Vec<Item>, area: Area, borders: vellum_grid_core::Borders, scale: f32) {
    let sides = [
        (borders.top, (area.x, area.y), (area.right(), area.y)),
        (
            borders.bottom,
            (area.x, area.bottom()),
            (area.right(), area.bottom()),
        ),
        (borders.left, (area.x, area.y), (area.x, area.bottom())),
        (
            borders.right,
            (area.right(), area.y),
            (area.right(), area.bottom()),
        ),
    ];
    let diagonals = borders.diagonals().map(|(border, up)| {
        if up {
            (
                Some(border),
                (area.x, area.bottom()),
                (area.right(), area.y),
            )
        } else {
            (
                Some(border),
                (area.x, area.y),
                (area.right(), area.bottom()),
            )
        }
    });
    for (border, from, to) in sides.into_iter().chain(diagonals) {
        let Some(Border { style, color }) = border else {
            continue;
        };
        let width = match (style, style.weight()) {
            (LineStyle::Double, _) => 0.6,
            (LineStyle::Hair, _) => 0.5,
            (_, 1) => 0.75,
            (_, 2) => 1.5,
            _ => 2.25,
        };
        let dash = match style.dashes() {
            [on, off, ..] => Some((*on, *off)),
            _ => None,
        };
        let width = width * scale.max(0.5);
        items.push(Item::Line {
            from,
            to,
            width,
            color,
            dash,
        });
        if style == LineStyle::Double {
            let gap = 1.5 * scale.max(0.5);
            let (dx, dy) = if from.1 == to.1 {
                (0.0, gap)
            } else {
                (gap, 0.0)
            };
            items.push(Item::Line {
                from: (from.0 + dx, from.1 + dy),
                to: (to.0 + dx, to.1 + dy),
                width,
                color,
                dash,
            });
        }
    }
}

struct TextCell<'a> {
    area: Area,
    text: &'a str,
    style: &'a Style,
    color: Color,
    value: &'a CellValue,
    scale: f32,
    overflow: Option<Area>,
}

fn push_text(items: &mut Vec<Item>, fonts: &mut FontBook, cell: &TextCell) {
    let style = cell.style;
    let font = fonts.id(&style.font, style.bold, style.italic);
    let size = style.points() * cell.scale;
    let pad = PADDING * cell.scale;
    let available = (cell.area.w - 2.0 * pad).max(0.0);
    let numeric = matches!(cell.value, CellValue::Number(_));
    let mut lines: Vec<String> = if style.wrap {
        wrap(fonts, font, size, cell.text, available)
    } else {
        cell.text.split('\n').map(str::to_owned).collect()
    };
    if numeric && !style.wrap && fonts.width(font, &lines[0], size) > available {
        let hash = fonts.width(font, "#", size).max(0.1);
        lines = vec!["#".repeat((available / hash).floor().max(1.0) as usize)];
    }
    let face = fonts.face(font);
    let (ascent, descent) = (face.ascent() * size, face.descent() * size);
    let line_height = size * LINE_FACTOR;
    let block = line_height * (lines.len() as f32 - 1.0) + ascent + descent;
    let area = cell.area;
    let first = match style.v_align {
        VAlign::Top | VAlign::Justify | VAlign::Distributed => area.y + pad + ascent,
        VAlign::Center => area.y + (area.h - block) / 2.0 + ascent,
        VAlign::Bottom => area.bottom() - pad - descent - line_height * (lines.len() as f32 - 1.0),
    };
    let align = match (style.h_align, cell.value) {
        (HAlign::General, CellValue::Number(_)) => HAlign::Right,
        (HAlign::General, CellValue::Bool(_) | CellValue::Error(_)) => HAlign::Center,
        (HAlign::General, _) => HAlign::Left,
        (other, _) => other,
    };
    let clip = cell.overflow.unwrap_or(area);
    for (index, line) in lines.iter().enumerate() {
        let w = fonts.width(font, line, size);
        let x = match align {
            HAlign::Right => area.right() - pad - w,
            HAlign::Center => area.x + (area.w - w) / 2.0,
            _ => area.x + pad,
        };
        items.push(Item::Text {
            x,
            baseline: first + line_height * index as f32,
            text: line.clone(),
            font,
            size,
            color: cell.color,
            underline: style.underline != vellum_grid_core::Underline::None,
            strike: style.strike,
            clip: Some(clip),
        });
    }
}

fn wrap(fonts: &FontBook, font: usize, size: f32, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if fonts.width(font, &candidate, size) <= width || line.is_empty() {
                line = candidate;
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_owned();
            }
        }
        lines.push(line);
    }
    lines
}

fn centered(
    items: &mut Vec<Item>,
    fonts: &mut FontBook,
    font: usize,
    size: f32,
    text: &str,
    area: Area,
) {
    let w = fonts.width(font, text, size);
    let face = fonts.face(font);
    let baseline = area.y + (area.h + face.ascent() * size - face.descent() * size) / 2.0;
    items.push(Item::Text {
        x: area.x + (area.w - w) / 2.0,
        baseline,
        text: text.to_owned(),
        font,
        size,
        color: Color::BLACK,
        underline: false,
        strike: false,
        clip: Some(area),
    });
}

pub fn used_fonts(pages: &[PrintPage]) -> HashSet<usize> {
    pages
        .iter()
        .flat_map(|p| &p.items)
        .filter_map(|item| match item {
            Item::Text { font, .. } => Some(*font),
            _ => None,
        })
        .collect()
}
