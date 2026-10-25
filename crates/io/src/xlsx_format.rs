use std::collections::HashMap;
use std::io::{Cursor, Read};

use vellum_grid_core::{
    Border, Borders, CellAddr, CellRange, Color, DEFAULT_FONT, DEFAULT_SIZE, GENERAL, HAlign,
    Highlight, Layout, LineStyle, Pattern, PatternKind, Script, Style, Underline, VAlign, numfmt,
};

use crate::{xlsx_drawing, xlsx_extras};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

const MAX_COLUMN_SPAN: u32 = 1024;
const DEFAULT_THEME: [u32; 12] = [
    0x000000, 0xFFFFFF, 0x44546A, 0xE7E6E6, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5,
    0x70AD47, 0x0563C1, 0x954F72,
];

#[derive(Debug, Default)]
pub struct SheetFormat {
    pub styles: Vec<(CellAddr, Style)>,
    pub layout: Layout,
}

pub(crate) enum Tag<'a> {
    Open(BytesStart<'a>),
    Empty(BytesStart<'a>),
    Text(String),
    Close(String),
}

pub(crate) fn walk(xml: &str, mut f: impl FnMut(Tag)) {
    let mut reader = Reader::from_str(xml);
    let mut text: Option<String> = None;
    loop {
        let event = reader.read_event();
        match &event {
            Ok(Event::Text(t)) => {
                let raw = String::from_utf8_lossy(t.as_ref()).into_owned();
                let part = quick_xml::escape::unescape(&raw)
                    .map(|v| v.into_owned())
                    .unwrap_or(raw);
                text.get_or_insert_with(String::new).push_str(&part);
                continue;
            }
            Ok(Event::GeneralRef(r)) => {
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c.to_string()),
                    _ => r.decode().ok().and_then(|name| {
                        quick_xml::escape::resolve_predefined_entity(&name).map(str::to_owned)
                    }),
                };
                if let Some(resolved) = resolved {
                    text.get_or_insert_with(String::new).push_str(&resolved);
                }
                continue;
            }
            _ => {}
        }
        if let Some(text) = text.take() {
            f(Tag::Text(text));
        }
        match event {
            Ok(Event::Start(e)) => f(Tag::Open(e)),
            Ok(Event::Empty(e)) => f(Tag::Empty(e)),
            Ok(Event::End(e)) => f(Tag::Close(
                String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
            )),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

pub(crate) fn name(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}

pub(crate) fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.local_name().as_ref() == key.as_bytes())
            .then(|| {
                let raw = std::str::from_utf8(&a.value).ok()?;
                quick_xml::escape::unescape(raw)
                    .ok()
                    .map(|v| v.into_owned())
            })
            .flatten()
    })
}

pub(crate) fn flag(e: &BytesStart, key: &str) -> bool {
    attr(e, key).is_some_and(|v| v == "1" || v == "true")
}

fn on(e: &BytesStart) -> bool {
    attr(e, "val").is_none_or(|v| v != "0" && v != "false" && v != "none")
}

fn read_entry(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, path: &str) -> Option<String> {
    let mut text = String::new();
    archive.by_name(path).ok()?.read_to_string(&mut text).ok()?;
    Some(text)
}

pub fn read(bytes: &[u8]) -> HashMap<String, SheetFormat> {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return HashMap::new();
    };
    let theme = read_entry(&mut archive, "xl/theme/theme1.xml")
        .map_or(DEFAULT_THEME.to_vec(), |t| theme(&t));
    let (styles, dxfs) = read_entry(&mut archive, "xl/styles.xml")
        .map(|xml| cell_styles(&xml, &theme))
        .unwrap_or_default();
    let relations = read_entry(&mut archive, "xl/_rels/workbook.xml.rels")
        .map(|xml| relations(&xml))
        .unwrap_or_default();
    let sheets = read_entry(&mut archive, "xl/workbook.xml")
        .map(|xml| sheets(&xml))
        .unwrap_or_default();
    let names = read_entry(&mut archive, "xl/workbook.xml")
        .map(|xml| defined_names(&xml))
        .unwrap_or_default();
    let mut out = HashMap::new();
    for (index, (sheet_name, id, hidden)) in sheets.into_iter().enumerate() {
        let Some(target) = relations.get(&id) else {
            continue;
        };
        let path = match target.strip_prefix('/') {
            Some(absolute) => absolute.to_owned(),
            None => format!("xl/{target}"),
        };
        if let Some(xml) = read_entry(&mut archive, &path) {
            let mut format = sheet(&xml, &styles);
            format.layout.sheet_hidden = hidden;
            let drawing = xlsx_extras::sheet_extras(&xml, &dxfs, &mut format.layout);
            crate::xlsx_review::read_sheet(&mut archive, &path, &xml, &mut format.layout);
            if let Some(id) = drawing {
                drawings(&mut archive, &path, &id, &mut format.layout, &theme);
            }
            for (name, local, text, _) in &names {
                if *local == Some(index) {
                    match name.as_str() {
                        "_xlnm.Print_Area" => {
                            xlsx_extras::print_names(text, &mut format.layout, false);
                        }
                        "_xlnm.Print_Titles" => {
                            xlsx_extras::print_names(text, &mut format.layout, true);
                        }
                        _ => {}
                    }
                }
            }
            out.insert(sheet_name, format);
        }
    }
    out
}

fn drawings(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    sheet_path: &str,
    drawing_id: &str,
    layout: &mut Layout,
    theme: &[u32],
) {
    let target = read_entry(archive, &xlsx_extras::rels_path(sheet_path))
        .and_then(|xml| relations(&xml).remove(drawing_id));
    let Some(target) = target else {
        return;
    };
    let drawing_path = xlsx_extras::resolve(sheet_path, &target);
    let Some(drawing) = read_entry(archive, &drawing_path) else {
        return;
    };
    let links = read_entry(archive, &xlsx_extras::rels_path(&drawing_path))
        .map(|xml| relations(&xml))
        .unwrap_or_default();
    for mut anchor in xlsx_drawing::drawing_anchors(&drawing, theme) {
        let target = |id: &Option<String>| {
            let target = links.get(id.as_deref()?)?;
            Some(xlsx_extras::resolve(&drawing_path, target))
        };
        if let Some(path) = target(&anchor.chart) {
            if let Some(chart) = read_entry(archive, &path)
                .and_then(|xml| xlsx_extras::parse_chart(&xml, &anchor, layout))
            {
                layout.charts.push(chart);
            }
        } else if let Some(path) = target(&anchor.picture) {
            let mut bytes = Vec::new();
            let read = archive
                .by_name(&path)
                .is_ok_and(|mut file| file.read_to_end(&mut bytes).is_ok());
            if let Some(picture) = read.then(|| anchor.picture(bytes, layout)).flatten() {
                layout.pictures.push(picture);
            }
        } else if let Some(shape) = anchor.shape(layout) {
            layout.shapes.push(shape);
        }
    }
}

fn theme(xml: &str) -> Vec<u32> {
    let mut colors = Vec::new();
    let mut inside = false;
    walk(xml, |tag| match tag {
        Tag::Open(e) if name(&e) == "clrScheme" => inside = true,
        Tag::Close(n) if n == "clrScheme" => inside = false,
        Tag::Open(e) | Tag::Empty(e) if inside => {
            let value = match name(&e).as_str() {
                "srgbClr" => attr(&e, "val"),
                "sysClr" => attr(&e, "lastClr"),
                _ => None,
            };
            if let Some(color) = value.and_then(|v| Color::from_hex(&v)) {
                colors.push(color.0);
            }
        }
        _ => {}
    });
    if colors.len() < 10 {
        DEFAULT_THEME.to_vec()
    } else {
        colors
    }
}

fn color(e: &BytesStart, theme: &[u32]) -> Option<Color> {
    if flag(e, "auto") {
        return None;
    }
    if let Some(rgb) = attr(e, "rgb") {
        return Color::from_hex(&rgb);
    }
    if let Some(index) = attr(e, "theme").and_then(|v| v.parse::<usize>().ok()) {
        let mapped = match index {
            0 => 1,
            1 => 0,
            2 => 3,
            3 => 2,
            other => other,
        };
        let base = Color(*theme.get(mapped)?);
        let tint: f64 = attr(e, "tint").and_then(|t| t.parse().ok()).unwrap_or(0.0);
        return Some(base.tinted(tint));
    }
    let indexed = match attr(e, "indexed")?.parse::<u32>().ok()? {
        0 | 8 | 64 => 0x000000,
        1 | 9 | 65 => 0xFFFFFF,
        2 | 10 => 0xFF0000,
        3 | 11 => 0x00FF00,
        4 | 12 => 0x0000FF,
        5 | 13 => 0xFFFF00,
        6 | 14 => 0xFF00FF,
        7 | 15 => 0x00FFFF,
        _ => return None,
    };
    Some(Color(indexed))
}

#[derive(Default, Clone)]
struct Font {
    name: Option<String>,
    size: Option<u16>,
    bold: bool,
    italic: bool,
    underline: Underline,
    strike: bool,
    script: Script,
    color: Option<Color>,
}

#[derive(Default, Clone, Copy)]
struct Fill {
    pattern: Option<PatternKind>,
    solid: bool,
    fg: Option<Color>,
    bg: Option<Color>,
}

impl Fill {
    fn resolve(self) -> (Option<Color>, Option<Pattern>) {
        match self.pattern {
            Some(kind) => (
                self.bg,
                Some(Pattern {
                    kind,
                    color: self.fg.unwrap_or(Color::BLACK),
                }),
            ),
            None if self.solid => (self.fg, None),
            None => (None, None),
        }
    }
}

fn builtin_format(id: u32) -> &'static str {
    match id {
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 => numfmt::DATE,
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => numfmt::TIME,
        21 => numfmt::TIME_SECONDS,
        22 => numfmt::DATE_TIME,
        37 => "#,##0 ;(#,##0)",
        38 => "#,##0 ;[Red](#,##0)",
        39 => "#,##0.00;(#,##0.00)",
        40 => "#,##0.00;[Red](#,##0.00)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mm:ss.0",
        48 => "##0.0E+0",
        49 => numfmt::TEXT,
        _ => GENERAL,
    }
}

const SECTIONS: [&str; 7] = [
    "fonts",
    "fills",
    "borders",
    "cellXfs",
    "cellStyleXfs",
    "numFmts",
    "dxfs",
];
const SIDES: [&str; 5] = ["left", "right", "top", "bottom", "diagonal"];

#[derive(Default)]
struct StyleParser {
    section: String,
    formats: HashMap<u32, String>,
    fonts: Vec<Font>,
    fills: Vec<Fill>,
    borders: Vec<Borders>,
    styles: Vec<Style>,
    font: Font,
    fill: Fill,
    border: Borders,
    side: Option<String>,
    style: Option<Style>,
    dxfs: Vec<Highlight>,
    dxf: Highlight,
    dxf_border: bool,
}

impl StyleParser {
    fn open(&mut self, e: &BytesStart, empty: bool, theme: &[u32]) {
        let tag = name(e);
        if SECTIONS.contains(&tag.as_str()) {
            self.section = tag;
            return;
        }
        match (self.section.as_str(), tag.as_str()) {
            ("numFmts", "numFmt") => {
                if let (Some(id), Some(code)) = (
                    attr(e, "numFmtId").and_then(|v| v.parse().ok()),
                    attr(e, "formatCode"),
                ) {
                    self.formats.insert(id, code);
                }
            }
            ("fonts", "font") => self.font = Font::default(),
            ("fonts", "b") => self.font.bold = on(e),
            ("fonts", "i") => self.font.italic = on(e),
            ("fonts", "u") => {
                self.font.underline = match attr(e, "val").as_deref() {
                    Some("none") => Underline::None,
                    Some("double") => Underline::Double,
                    Some("singleAccounting") => Underline::SingleAccounting,
                    Some("doubleAccounting") => Underline::DoubleAccounting,
                    _ => Underline::Single,
                }
            }
            ("fonts", "vertAlign") => {
                self.font.script = match attr(e, "val").as_deref() {
                    Some("superscript") => Script::Super,
                    Some("subscript") => Script::Sub,
                    _ => Script::None,
                }
            }
            ("fonts", "strike") => self.font.strike = on(e),
            ("fonts", "sz") => {
                self.font.size = attr(e, "val")
                    .and_then(|v| v.parse::<f32>().ok())
                    .map(|v| (v * 10.0).round() as u16);
            }
            ("fonts", "name") => self.font.name = attr(e, "val"),
            ("fonts", "color") => self.font.color = color(e, theme),
            ("fills", "fill") => self.fill = Fill::default(),
            ("fills", "patternFill") => {
                let kind = attr(e, "patternType");
                self.fill.solid = kind.as_deref() == Some("solid");
                self.fill.pattern = kind.as_deref().and_then(PatternKind::from_key);
            }
            ("fills", "fgColor") => self.fill.fg = color(e, theme),
            ("fills", "bgColor") => self.fill.bg = color(e, theme),
            ("borders", "border") => {
                self.border = Borders {
                    diagonal_up: flag(e, "diagonalUp"),
                    diagonal_down: flag(e, "diagonalDown"),
                    ..Borders::default()
                }
            }
            ("borders", side) if SIDES.contains(&side) => {
                if let Some(style) = attr(e, "style").and_then(|s| LineStyle::from_key(&s)) {
                    set_side(
                        &mut self.border,
                        side,
                        Border {
                            style,
                            color: Color::BLACK,
                        },
                    );
                    self.side = (!empty).then(|| side.to_owned());
                }
            }
            ("borders", "color") => {
                if let Some(side) = self.side.clone() {
                    let line = side_of(&self.border, &side).map(|b| b.style);
                    if let Some(style) = line {
                        let c = color(e, theme).unwrap_or(Color::BLACK);
                        set_side(&mut self.border, &side, Border { style, color: c });
                    }
                }
            }
            ("dxfs", "dxf") => {
                self.dxf = Highlight::default();
                if empty {
                    self.dxfs.push(self.dxf);
                }
            }
            ("dxfs", "border") => self.dxf_border = !empty,
            ("dxfs", "b") => self.dxf.bold = on(e),
            ("dxfs", "i") => self.dxf.italic = on(e),
            ("dxfs", "strike") => self.dxf.strike = on(e),
            ("dxfs", side) if self.dxf_border && SIDES.contains(&side) => {
                if let Some(style) = attr(e, "style").and_then(|s| LineStyle::from_key(&s)) {
                    self.dxf.border = Some(Border {
                        style,
                        color: self.dxf.border.map_or(Color::BLACK, |b| b.color),
                    });
                }
            }
            ("dxfs", "color") if self.dxf_border => {
                if let Some(border) = self.dxf.border.as_mut() {
                    border.color = color(e, theme).unwrap_or(Color::BLACK);
                }
            }
            ("dxfs", "color") => self.dxf.color = color(e, theme),
            ("dxfs", "bgColor") => self.dxf.fill = color(e, theme),
            ("dxfs", "fgColor") => self.dxf.fill = self.dxf.fill.or(color(e, theme)),
            ("cellXfs", "xf") => {
                self.style = Some(self.xf(e));
                if empty {
                    self.styles.push(self.style.take().unwrap_or_default());
                }
            }
            ("cellXfs", "protection") => {
                if let Some(s) = self.style.as_mut() {
                    s.locked = attr(e, "locked").as_deref() != Some("0");
                    s.hidden = attr(e, "hidden").as_deref() == Some("1");
                }
            }
            ("cellXfs", "alignment") => {
                if let Some(s) = self.style.as_mut() {
                    s.h_align = match attr(e, "horizontal").as_deref() {
                        Some("left") => HAlign::Left,
                        Some("center") => HAlign::Center,
                        Some("right") => HAlign::Right,
                        Some("fill") => HAlign::Fill,
                        Some("justify") => HAlign::Justify,
                        Some("centerContinuous") => HAlign::CenterAcross,
                        Some("distributed") => HAlign::Distributed,
                        _ => HAlign::General,
                    };
                    s.v_align = match attr(e, "vertical").as_deref() {
                        Some("top") => VAlign::Top,
                        Some("center") => VAlign::Center,
                        Some("justify") => VAlign::Justify,
                        Some("distributed") => VAlign::Distributed,
                        _ => VAlign::Bottom,
                    };
                    s.wrap = flag(e, "wrapText");
                    s.shrink = flag(e, "shrinkToFit");
                    s.indent = attr(e, "indent").and_then(|v| v.parse().ok()).unwrap_or(0);
                    s.rotation = match attr(e, "textRotation").and_then(|v| v.parse::<i16>().ok()) {
                        Some(255) => vellum_grid_core::STACKED,
                        Some(angle @ 91..=180) => 90 - angle,
                        Some(angle @ 0..=90) => angle,
                        _ => 0,
                    };
                }
            }
            _ => {}
        }
    }

    fn close(&mut self, tag: &str) {
        match (self.section.as_str(), tag) {
            ("fonts", "font") => self.fonts.push(std::mem::take(&mut self.font)),
            ("fills", "fill") => self.fills.push(std::mem::take(&mut self.fill)),
            ("borders", "border") => self.borders.push(std::mem::take(&mut self.border)),
            ("borders", side) if SIDES.contains(&side) => self.side = None,
            ("dxfs", "dxf") => self.dxfs.push(self.dxf),
            ("dxfs", "border") => self.dxf_border = false,
            ("cellXfs", "xf") => {
                if let Some(style) = self.style.take() {
                    self.styles.push(style);
                }
            }
            (_, section) if SECTIONS.contains(&section) => self.section.clear(),
            _ => {}
        }
    }

    fn xf(&self, e: &BytesStart) -> Style {
        let index = |key: &str| {
            attr(e, key)
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
        };
        let number = attr(e, "numFmtId")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        let font = self.fonts.get(index("fontId")).cloned().unwrap_or_default();
        let (fill, pattern) = self
            .fills
            .get(index("fillId"))
            .copied()
            .unwrap_or_default()
            .resolve();
        let base = self.fonts.first().cloned().unwrap_or_default();
        let mut style = Style {
            number_format: self
                .formats
                .get(&number)
                .cloned()
                .unwrap_or_else(|| builtin_format(number).to_owned()),
            bold: font.bold,
            italic: font.italic,
            underline: font.underline,
            strike: font.strike,
            script: font.script,
            color: font.color.filter(|c| *c != Color::BLACK),
            fill,
            pattern,
            borders: self
                .borders
                .get(index("borderId"))
                .copied()
                .unwrap_or_default(),
            ..Style::default()
        };
        if font.name != base.name {
            style.font = font.name.unwrap_or_else(|| DEFAULT_FONT.to_owned());
        }
        if font.size != base.size {
            style.size = font.size.unwrap_or(DEFAULT_SIZE);
        }
        style
    }
}

fn cell_styles(xml: &str, theme: &[u32]) -> (Vec<Style>, Vec<Highlight>) {
    let mut parser = StyleParser::default();
    walk(xml, |tag| match tag {
        Tag::Open(e) => parser.open(&e, false, theme),
        Tag::Empty(e) => parser.open(&e, true, theme),
        Tag::Text(_) => {}
        Tag::Close(n) => parser.close(&n),
    });
    (parser.styles, parser.dxfs)
}

fn side_of<'a>(borders: &'a Borders, side: &str) -> Option<&'a Border> {
    match side {
        "left" => borders.left.as_ref(),
        "right" => borders.right.as_ref(),
        "top" => borders.top.as_ref(),
        "diagonal" => borders.diagonal.as_ref(),
        _ => borders.bottom.as_ref(),
    }
}

fn set_side(borders: &mut Borders, side: &str, border: Border) {
    match side {
        "left" => borders.left = Some(border),
        "right" => borders.right = Some(border),
        "top" => borders.top = Some(border),
        "diagonal" => borders.diagonal = Some(border),
        _ => borders.bottom = Some(border),
    }
}

pub(crate) fn relations(xml: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    walk(xml, |tag| {
        if let Tag::Open(e) | Tag::Empty(e) = tag
            && name(&e) == "Relationship"
            && let (Some(id), Some(target)) = (attr(&e, "Id"), attr(&e, "Target"))
        {
            out.insert(id, target);
        }
    });
    out
}

pub fn names(bytes: &[u8]) -> Vec<(String, Option<usize>, String, String)> {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return Vec::new();
    };
    read_entry(&mut archive, "xl/workbook.xml")
        .map(|xml| defined_names(&xml))
        .unwrap_or_default()
        .into_iter()
        .filter(|(name, ..)| !name.starts_with("_xlnm."))
        .collect()
}

pub fn manual_calculation(bytes: &[u8]) -> bool {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return false;
    };
    let mut manual = false;
    if let Some(xml) = read_entry(&mut archive, "xl/workbook.xml") {
        walk(&xml, |tag| {
            if let Tag::Open(e) | Tag::Empty(e) = tag
                && name(&e) == "calcPr"
            {
                manual = attr(&e, "calcMode").as_deref() == Some("manual");
            }
        });
    }
    manual
}

fn defined_names(xml: &str) -> Vec<(String, Option<usize>, String, String)> {
    let mut out = Vec::new();
    let mut current: Option<(String, Option<usize>, String)> = None;
    walk(xml, |tag| match tag {
        Tag::Open(e) if name(&e) == "definedName" => {
            let local = attr(&e, "localSheetId").and_then(|v| v.parse().ok());
            let comment = attr(&e, "comment").unwrap_or_default();
            current = attr(&e, "name").map(|n| (n, local, comment));
        }
        Tag::Text(text) => {
            if let Some((name, local, comment)) = current.take() {
                out.push((name, local, text, comment));
            }
        }
        Tag::Close(_) => current = None,
        _ => {}
    });
    out
}

fn sheets(xml: &str) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    walk(xml, |tag| {
        if let Tag::Open(e) | Tag::Empty(e) = tag
            && name(&e) == "sheet"
            && let (Some(name), Some(id)) = (attr(&e, "name"), attr(&e, "id"))
        {
            let hidden = attr(&e, "state").is_some_and(|state| state != "visible");
            out.push((name, id, hidden));
        }
    });
    out
}

fn sheet(xml: &str, styles: &[Style]) -> SheetFormat {
    let mut out = SheetFormat::default();
    walk(xml, |tag| {
        let (Tag::Open(e) | Tag::Empty(e)) = tag else {
            return;
        };
        match name(&e).as_str() {
            "sheetView" => {
                out.layout.hide_gridlines = attr(&e, "showGridLines").is_some_and(|v| v == "0");
                out.layout.hide_headings = attr(&e, "showRowColHeaders").is_some_and(|v| v == "0");
            }
            "col" => {
                let number = |key: &str| attr(&e, key).and_then(|v| v.parse::<u32>().ok());
                let width = attr(&e, "width").and_then(|v| v.parse::<f64>().ok());
                if let (Some(min), Some(max), Some(width)) = (number("min"), number("max"), width)
                    && flag(&e, "customWidth")
                    && max >= min
                    && max - min < MAX_COLUMN_SPAN
                {
                    for col in min..=max {
                        out.layout.col_widths.insert(col - 1, width);
                    }
                }
            }
            "row" => {
                let row = attr(&e, "r").and_then(|v| v.parse::<u32>().ok());
                let height = attr(&e, "ht").and_then(|v| v.parse::<f64>().ok());
                if let (Some(row), Some(height)) = (row, height)
                    && flag(&e, "customHeight")
                    && row > 0
                {
                    out.layout.row_heights.insert(row - 1, height);
                }
            }
            "c" => {
                let addr = attr(&e, "r").and_then(|r| r.parse::<CellAddr>().ok());
                let style = attr(&e, "s")
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|i| styles.get(i))
                    .filter(|s| !s.is_default());
                if let (Some(addr), Some(style)) = (addr, style) {
                    out.styles.push((addr, style.clone()));
                }
            }
            "mergeCell" => {
                if let Some(range) = attr(&e, "ref").and_then(|r| r.parse::<CellRange>().ok()) {
                    out.layout.merges.push(range);
                }
            }
            _ => {}
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_styles_xml() {
        let xml = r##"<styleSheet>
            <numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.00 &quot;₺&quot;"/></numFmts>
            <fonts count="2">
                <font><sz val="11"/><color theme="1"/><name val="Calibri"/></font>
                <font><b/><i/><sz val="14"/><color rgb="FFFF0000"/><name val="Calibri"/></font>
            </fonts>
            <fills count="3">
                <fill><patternFill patternType="none"/></fill>
                <fill><patternFill patternType="gray125"/></fill>
                <fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/><bgColor indexed="64"/></patternFill></fill>
            </fills>
            <borders count="2">
                <border><left/><right/><top/><bottom/><diagonal/></border>
                <border><left style="thin"><color indexed="64"/></left><right/><top/><bottom style="double"><color rgb="FF0000FF"/></bottom></border>
            </borders>
            <cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
            <cellXfs count="3">
                <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
                <xf numFmtId="164" fontId="1" fillId="2" borderId="1" applyAlignment="1"><alignment horizontal="center" vertical="center" wrapText="1"/></xf>
                <xf numFmtId="14" fontId="0" fillId="0" borderId="0"/>
            </cellXfs>
            <dxfs count="1"><dxf><font><color rgb="FF9C0006"/></font><fill><patternFill><bgColor rgb="FFFFC7CE"/></patternFill></fill></dxf></dxfs>
        </styleSheet>"##;
        let (styles, dxfs) = cell_styles(xml, &DEFAULT_THEME);
        assert_eq!(
            dxfs,
            [Highlight {
                fill: Some(Color(0xFFC7CE)),
                color: Some(Color(0x9C0006)),
                ..Highlight::default()
            }]
        );
        assert_eq!(styles.len(), 3);
        assert!(styles[0].is_default());
        let s = &styles[1];
        assert_eq!(s.number_format, "#,##0.00 \"₺\"");
        assert!(s.bold && s.italic);
        assert_eq!(s.size, 140);
        assert_eq!(s.color, Some(Color(0xFF0000)));
        assert_eq!(s.fill, Some(Color(0xFFFF00)));
        assert_eq!(s.borders.left, Some(Border::thin()));
        assert_eq!(
            s.borders.bottom,
            Some(Border {
                style: LineStyle::Double,
                color: Color(0x0000FF)
            })
        );
        assert_eq!(
            (s.h_align, s.v_align, s.wrap),
            (HAlign::Center, VAlign::Center, true)
        );
        assert_eq!(styles[2].number_format, numfmt::DATE);
    }

    #[test]
    fn reads_sheet_layout() {
        let xml = r#"<worksheet>
            <sheetViews><sheetView showGridLines="0" workbookViewId="0"/></sheetViews>
            <cols><col min="2" max="3" width="20.5" customWidth="1"/><col min="4" max="16384" width="9" style="1"/></cols>
            <sheetData>
                <row r="1" ht="30" customHeight="1"><c r="A1" s="1" t="s"><v>0</v></c><c r="B1" s="0"/></row>
                <row r="2" ht="15"><c r="A2"/></row>
            </sheetData>
            <mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells>
        </worksheet>"#;
        let styles = vec![Style::default(), Style::with_number_format("0.00")];
        let format = sheet(xml, &styles);
        assert!(format.layout.hide_gridlines);
        assert!(!format.layout.hide_headings);
        assert_eq!(format.layout.col_width(1), 20.5);
        assert_eq!(format.layout.col_width(2), 20.5);
        assert_eq!(format.layout.col_widths.len(), 2);
        assert_eq!(format.layout.row_height(0), 30.0);
        assert_eq!(format.layout.row_heights.len(), 1);
        assert_eq!(
            format.layout.merges,
            ["A1:B1".parse::<CellRange>().unwrap()]
        );
        assert_eq!(format.styles, [("A1".parse().unwrap(), styles[1].clone())]);
    }

    #[test]
    fn theme_colors_and_tints() {
        let xml = r#"<a:theme><a:themeElements><a:clrScheme name="Office">
            <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
            <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
            <a:dk2><a:srgbClr val="0E2841"/></a:dk2><a:lt2><a:srgbClr val="E8E8E8"/></a:lt2>
            <a:accent1><a:srgbClr val="156082"/></a:accent1><a:accent2><a:srgbClr val="E97132"/></a:accent2>
            <a:accent3><a:srgbClr val="196B24"/></a:accent3><a:accent4><a:srgbClr val="0F9ED5"/></a:accent4>
            <a:accent5><a:srgbClr val="A02B93"/></a:accent5><a:accent6><a:srgbClr val="4EA72E"/></a:accent6>
            <a:hlink><a:srgbClr val="467886"/></a:hlink><a:folHlink><a:srgbClr val="96607D"/></a:folHlink>
        </a:clrScheme></a:themeElements></a:theme>"#;
        let colors = theme(xml);
        assert_eq!(colors[4], 0x156082);
        let xml = r#"<x><color theme="4"/><color theme="0" tint="-0.5"/><color theme="1"/></x>"#;
        let mut found = Vec::new();
        walk(xml, |tag| {
            if let Tag::Empty(e) = tag {
                found.push(color(&e, &colors));
            }
        });
        assert_eq!(
            found,
            [
                Some(Color(0x156082)),
                Some(Color(0x808080)),
                Some(Color(0x000000))
            ]
        );
    }
}
