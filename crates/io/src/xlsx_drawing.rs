use std::fmt::Write;

use vellum_grid_core::drawing::{Picture, Shape, ShapeKind, TextAlign, TextAnchor};
use vellum_grid_core::{CellAddr, Color, Layout};

use crate::ods::escape;
use crate::xlsx_format::{Tag, attr, flag, name, walk};

const EMU_PER_PIXEL: f64 = 9525.0;
const EMU_PER_HUNDREDTH_POINT: u32 = 127;
const DIGIT_PIXELS: f64 = 7.0;
const LINE_STYLES: [u16; 3] = [50, 100, 150];
const FIRST_SHAPE_ID: usize = 1000;
pub const PICTURE_MARKER: &str = "vellum-picture-";

#[derive(Debug, Default, Clone, Copy)]
pub struct Marker {
    pub col: u32,
    pub col_off: i64,
    pub row: u32,
    pub row_off: i64,
}

#[derive(Debug, Default)]
pub struct Anchor {
    pub from: Marker,
    pub to: Option<Marker>,
    pub ext: Option<(i64, i64)>,
    pub chart: Option<String>,
    pub picture: Option<String>,
    shape: Option<Draft>,
}

#[derive(Debug, Default)]
struct Draft {
    stack: Vec<String>,
    preset: String,
    text_box: bool,
    flip: (bool, bool),
    fill: Option<Option<Color>>,
    line: Option<Option<Color>>,
    line_width: Option<u16>,
    ends: (bool, bool),
    style_index: u16,
    style_fill: Option<Color>,
    style_line: Option<(Color, u16)>,
    style_font: Option<Color>,
    font: Option<Color>,
    size: Option<u16>,
    bold: bool,
    italic: bool,
    align: Option<TextAlign>,
    anchor: Option<TextAnchor>,
    paragraphs: Vec<String>,
    in_text: bool,
    color: Option<(Color, f64, f64)>,
}

impl Draft {
    fn inside(&self, tag: &str) -> bool {
        self.stack.iter().any(|t| t == tag)
    }

    fn assign(&mut self, color: Color) {
        if self.inside("rPr") || self.inside("defRPr") || self.inside("endParaRPr") {
            self.font.get_or_insert(color);
        } else if self.inside("ln") {
            self.line.get_or_insert(Some(color));
        } else if self.inside("spPr") {
            self.fill.get_or_insert(Some(color));
        } else if self.inside("lnRef") {
            let width = LINE_STYLES[usize::from(self.style_index.clamp(1, 3)) - 1];
            self.style_line = Some((color, width));
        } else if self.inside("fillRef") && self.style_index > 0 {
            self.style_fill = Some(color);
        } else if self.inside("fontRef") {
            self.style_font = Some(color);
        }
    }

    fn open(&mut self, e: &quick_xml::events::BytesStart, tag: &str, empty: bool, theme: &[u32]) {
        match tag {
            "cNvSpPr" => self.text_box = flag(e, "txBox"),
            "xfrm" if self.inside("spPr") => self.flip = (flag(e, "flipH"), flag(e, "flipV")),
            "prstGeom" => self.preset = attr(e, "prst").unwrap_or_default(),
            "ln" if self.inside("spPr") => {
                self.line_width = attr(e, "w")
                    .and_then(|w| w.parse::<u32>().ok())
                    .map(|w| (w / EMU_PER_HUNDREDTH_POINT) as u16);
            }
            "noFill" if self.inside("ln") => self.line = Some(None),
            "noFill" if self.inside("spPr") => self.fill = Some(None),
            "headEnd" | "tailEnd" => {
                let on = attr(e, "type").is_some_and(|t| t != "none");
                if tag == "headEnd" {
                    self.ends.0 = on;
                } else {
                    self.ends.1 = on;
                }
            }
            "lnRef" | "fillRef" | "fontRef" => {
                self.style_index = attr(e, "idx").and_then(|i| i.parse().ok()).unwrap_or(0);
            }
            "bodyPr" => {
                self.anchor = attr(e, "anchor").map(|a| match a.as_str() {
                    "ctr" => TextAnchor::Middle,
                    "b" => TextAnchor::Bottom,
                    _ => TextAnchor::Top,
                });
            }
            "p" => self.paragraphs.push(String::new()),
            "pPr" if self.align.is_none() => {
                self.align = attr(e, "algn").map(|a| match a.as_str() {
                    "ctr" => TextAlign::Center,
                    "r" => TextAlign::Right,
                    _ => TextAlign::Left,
                });
            }
            "rPr" | "defRPr" | "endParaRPr" => {
                if self.size.is_none() {
                    self.size = attr(e, "sz")
                        .and_then(|s| s.parse::<u16>().ok())
                        .map(|s| s / 10);
                }
                if tag == "rPr" {
                    self.bold |= flag(e, "b");
                    self.italic |= flag(e, "i");
                }
            }
            "t" => self.in_text = !empty,
            "srgbClr" | "schemeClr" | "sysClr" | "prstClr" => {
                let color = match tag {
                    "srgbClr" => attr(e, "val").and_then(|v| Color::from_hex(&v)),
                    "schemeClr" => attr(e, "val").and_then(|v| scheme(&v, theme)),
                    "sysClr" => attr(e, "lastClr").and_then(|v| Color::from_hex(&v)),
                    _ => Some(match attr(e, "val").as_deref() {
                        Some("white") => Color(0xFFFFFF),
                        _ => Color(0x000000),
                    }),
                };
                if let Some(color) = color {
                    if empty {
                        self.assign(color);
                    } else {
                        self.color = Some((color, 1.0, 0.0));
                    }
                }
            }
            "shade" | "tint" | "lumMod" | "lumOff" => {
                if let Some((color, lum_mod, lum_off)) = self.color.as_mut() {
                    let value = attr(e, "val")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(100_000.0)
                        / 100_000.0;
                    match tag {
                        "shade" => *color = color.tinted(value - 1.0),
                        "tint" => *color = color.tinted(1.0 - value),
                        "lumMod" => *lum_mod = value,
                        _ => *lum_off = value,
                    }
                }
            }
            _ => {}
        }
        if !empty {
            self.stack.push(tag.to_owned());
        }
    }

    fn close(&mut self, tag: &str) {
        match tag {
            "t" => self.in_text = false,
            "srgbClr" | "schemeClr" | "sysClr" | "prstClr" => {
                if let Some((color, lum_mod, lum_off)) = self.color.take() {
                    let color = if lum_off > 0.0 {
                        color.tinted(lum_off)
                    } else {
                        color.tinted(lum_mod - 1.0)
                    };
                    self.assign(color);
                }
            }
            _ => {}
        }
        self.stack.pop();
    }

    fn finish(self, anchor: CellAddr) -> Option<Shape> {
        let kind = ShapeKind::from_preset(&self.preset, self.text_box, self.ends.0, self.ends.1)?;
        let mut shape = Shape::new(kind, anchor);
        (shape.flip_h, shape.flip_v) = self.flip;
        shape.fill = if kind.is_line() {
            None
        } else {
            self.fill.unwrap_or(self.style_fill)
        };
        shape.line = self.line.unwrap_or(self.style_line.map(|(c, _)| c));
        shape.line_width = self
            .line_width
            .or(self.style_line.map(|(_, w)| w))
            .unwrap_or(shape.line_width);
        shape.color = self.font.or(self.style_font).unwrap_or(Color(0x000000));
        shape.font_size = self.size.unwrap_or(shape.font_size);
        shape.bold = self.bold;
        shape.italic = self.italic;
        shape.align = self.align.unwrap_or(TextAlign::Left);
        shape.text_anchor = self.anchor.unwrap_or(TextAnchor::Top);
        let mut paragraphs = self.paragraphs;
        while paragraphs.last().is_some_and(String::is_empty) {
            paragraphs.pop();
        }
        shape.text = paragraphs.join("\n");
        Some(shape)
    }
}

fn scheme(name: &str, theme: &[u32]) -> Option<Color> {
    let index = match name {
        "dk1" | "tx1" => 0,
        "lt1" | "bg1" => 1,
        "dk2" | "tx2" => 2,
        "lt2" | "bg2" => 3,
        "accent1" => 4,
        "accent2" => 5,
        "accent3" => 6,
        "accent4" => 7,
        "accent5" => 8,
        "accent6" => 9,
        "hlink" => 10,
        "folHlink" => 11,
        _ => return None,
    };
    theme.get(index).map(|c| Color(*c))
}

pub fn drawing_anchors(xml: &str, theme: &[u32]) -> Vec<Anchor> {
    let mut anchors = Vec::new();
    let mut current: Option<Anchor> = None;
    let mut marker: Option<(bool, Marker)> = None;
    let mut field = String::new();
    let mut in_frame = false;
    let mut group = 0;
    let mut draft: Option<Draft> = None;
    walk(xml, |tag| {
        let empty = matches!(tag, Tag::Empty(_));
        match tag {
            Tag::Open(e) | Tag::Empty(e) => {
                let tag = name(&e);
                if let Some(draft) = draft.as_mut() {
                    draft.open(&e, &tag, empty, theme);
                    return;
                }
                match tag.as_str() {
                    "twoCellAnchor" | "oneCellAnchor" => current = Some(Anchor::default()),
                    "from" => marker = Some((true, Marker::default())),
                    "to" => marker = Some((false, Marker::default())),
                    "col" | "colOff" | "row" | "rowOff" => field = tag,
                    "graphicFrame" => in_frame = true,
                    "grpSp" if !empty => group += 1,
                    "sp" | "cxnSp" if group == 0 && !empty && current.is_some() => {
                        draft = Some(Draft::default());
                    }
                    "ext" if !in_frame => {
                        if let Some(anchor) = current.as_mut() {
                            let size =
                                |key: &str| attr(&e, key).and_then(|v| v.parse::<i64>().ok());
                            if let (Some(cx), Some(cy)) = (size("cx"), size("cy")) {
                                anchor.ext.get_or_insert((cx, cy));
                            }
                        }
                    }
                    "chart" => {
                        if let Some(anchor) = current.as_mut() {
                            anchor.chart = attr(&e, "id");
                        }
                    }
                    "blip" if group == 0 => {
                        if let Some(anchor) = current.as_mut() {
                            anchor.picture = attr(&e, "embed");
                        }
                    }
                    _ => {}
                }
            }
            Tag::Text(text) => {
                if let Some(draft) = draft.as_mut() {
                    if draft.in_text
                        && let Some(paragraph) = draft.paragraphs.last_mut()
                    {
                        paragraph.push_str(&text);
                    }
                    return;
                }
                if let Some((_, m)) = marker.as_mut() {
                    let value = text.trim().parse::<i64>().unwrap_or(0);
                    match field.as_str() {
                        "col" => m.col = value.max(0) as u32,
                        "colOff" => m.col_off = value,
                        "row" => m.row = value.max(0) as u32,
                        "rowOff" => m.row_off = value,
                        _ => {}
                    }
                }
            }
            Tag::Close(tag) => {
                if let Some(open) = draft.as_mut() {
                    if open.stack.is_empty() {
                        if let Some(anchor) = current.as_mut() {
                            anchor.shape = draft.take();
                        }
                    } else {
                        open.close(&tag);
                    }
                    return;
                }
                match tag.as_str() {
                    "from" | "to" => {
                        if let (Some((from, m)), Some(anchor)) = (marker.take(), current.as_mut()) {
                            if from {
                                anchor.from = m;
                            } else {
                                anchor.to = Some(m);
                            }
                        }
                    }
                    "col" | "colOff" | "row" | "rowOff" => field.clear(),
                    "graphicFrame" => in_frame = false,
                    "grpSp" => group -= 1,
                    "twoCellAnchor" | "oneCellAnchor" => {
                        if let Some(anchor) = current.take().filter(|a| {
                            a.chart.is_some() || a.picture.is_some() || a.shape.is_some()
                        }) {
                            anchors.push(anchor);
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    anchors
}

fn px(emu: i64) -> f64 {
    (emu as f64 / EMU_PER_PIXEL).round()
}

fn col_px(layout: &Layout, col: u32) -> f64 {
    if layout.hidden_cols.contains(&col) {
        0.0
    } else {
        (layout.col_width(col) * DIGIT_PIXELS).round()
    }
}

fn row_px(layout: &Layout, row: u32) -> f64 {
    if layout.hidden_rows.contains(&row) {
        0.0
    } else {
        (layout.row_height(row) * 96.0 / 72.0).round()
    }
}

pub type Placement = (CellAddr, (u32, u32), (u32, u32));

pub fn placement(anchor: &Anchor, layout: &Layout) -> Option<Placement> {
    let from = anchor.from;
    let size = match (anchor.to, anchor.ext) {
        (Some(to), _) => {
            let width: f64 = (from.col..to.col).map(|c| col_px(layout, c)).sum::<f64>()
                + px(to.col_off)
                - px(from.col_off);
            let height: f64 = (from.row..to.row).map(|r| row_px(layout, r)).sum::<f64>()
                + px(to.row_off)
                - px(from.row_off);
            (width, height)
        }
        (None, Some((cx, cy))) => (px(cx), px(cy)),
        (None, None) => return None,
    };
    Some((
        CellAddr::new(from.row, from.col).ok()?,
        (
            px(from.col_off).max(0.0) as u32,
            px(from.row_off).max(0.0) as u32,
        ),
        (
            size.0.max(1.0).round() as u32,
            size.1.max(1.0).round() as u32,
        ),
    ))
}

impl Anchor {
    pub fn shape(&mut self, layout: &Layout) -> Option<Shape> {
        let (anchor, offset, size) = placement(self, layout)?;
        let mut shape = self.shape.take()?.finish(anchor)?;
        shape.offset = offset;
        shape.size = size;
        Some(shape)
    }

    pub fn picture(&self, data: Vec<u8>, layout: &Layout) -> Option<Picture> {
        let (anchor, offset, size) = placement(self, layout)?;
        let mut picture = Picture::new(data, anchor, size)?;
        picture.offset = offset;
        Some(picture)
    }
}

fn emu(pixels: u32) -> u64 {
    u64::from(pixels) * EMU_PER_PIXEL as u64
}

fn one_cell(anchor: CellAddr, offset: (u32, u32), size: (u32, u32), body: &str, out: &mut String) {
    let _ = write!(
        out,
        "<xdr:oneCellAnchor><xdr:from><xdr:col>{}</xdr:col><xdr:colOff>{}</xdr:colOff>\
         <xdr:row>{}</xdr:row><xdr:rowOff>{}</xdr:rowOff></xdr:from>\
         <xdr:ext cx=\"{}\" cy=\"{}\"/>{body}<xdr:clientData/></xdr:oneCellAnchor>",
        anchor.col(),
        emu(offset.0),
        anchor.row(),
        emu(offset.1),
        emu(size.0),
        emu(size.1),
    );
}

fn solid(color: Color) -> String {
    format!(
        "<a:solidFill><a:srgbClr val=\"{:06X}\"/></a:solidFill>",
        color.0
    )
}

fn shape_xml(shape: &Shape, id: usize) -> String {
    let kind = shape.kind;
    let text_box = kind == ShapeKind::TextBox;
    let mut out = String::new();
    let _ = write!(
        out,
        "<xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{id}\" name=\"{} {}\"/>\
         <xdr:cNvSpPr{}/></xdr:nvSpPr><xdr:spPr><a:xfrm{}{}><a:off x=\"0\" y=\"0\"/>\
         <a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"{}\"><a:avLst/></a:prstGeom>",
        if text_box { "TextBox" } else { "Shape" },
        id - FIRST_SHAPE_ID + 1,
        if text_box { " txBox=\"1\"" } else { "" },
        if shape.flip_h { " flipH=\"1\"" } else { "" },
        if shape.flip_v { " flipV=\"1\"" } else { "" },
        emu(shape.size.0),
        emu(shape.size.1),
        kind.preset(),
    );
    out.push_str(&match shape.fill {
        Some(color) if !kind.is_line() => solid(color),
        _ => "<a:noFill/>".to_owned(),
    });
    match shape.line {
        Some(color) => {
            let _ = write!(
                out,
                "<a:ln w=\"{}\">{}",
                u32::from(shape.line_width) * EMU_PER_HUNDREDTH_POINT,
                solid(color)
            );
            if kind == ShapeKind::DoubleArrow {
                out.push_str("<a:headEnd type=\"triangle\"/>");
            }
            if matches!(kind, ShapeKind::Arrow | ShapeKind::DoubleArrow) {
                out.push_str("<a:tailEnd type=\"triangle\"/>");
            }
            out.push_str("</a:ln>");
        }
        None => out.push_str("<a:ln><a:noFill/></a:ln>"),
    }
    out.push_str("</xdr:spPr>");
    if !kind.is_line() {
        let anchor = match shape.text_anchor {
            TextAnchor::Top => "t",
            TextAnchor::Middle => "ctr",
            TextAnchor::Bottom => "b",
        };
        let align = match shape.align {
            TextAlign::Left => "l",
            TextAlign::Center => "ctr",
            TextAlign::Right => "r",
        };
        let props = format!(
            "lang=\"tr-TR\" sz=\"{}\"{}{}",
            u32::from(shape.font_size) * 10,
            if shape.bold { " b=\"1\"" } else { "" },
            if shape.italic { " i=\"1\"" } else { "" },
        );
        let _ = write!(
            out,
            "<xdr:txBody><a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\" wrap=\"square\" \
             rtlCol=\"0\" anchor=\"{anchor}\"/><a:lstStyle/>"
        );
        for line in shape.text.split('\n') {
            let _ = write!(out, "<a:p><a:pPr algn=\"{align}\"/>");
            if !line.is_empty() {
                let _ = write!(
                    out,
                    "<a:r><a:rPr {props}>{}</a:rPr><a:t>{}</a:t></a:r>",
                    solid(shape.color),
                    escape(line)
                );
            }
            let _ = write!(
                out,
                "<a:endParaRPr {props}>{}</a:endParaRPr></a:p>",
                solid(shape.color)
            );
        }
        out.push_str("</xdr:txBody>");
    }
    out.push_str("</xdr:sp>");
    out
}

fn anchors(xml: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(start) = xml[at..]
        .find("<xdr:twoCellAnchor")
        .or_else(|| xml[at..].find("<xdr:oneCellAnchor"))
        .map(|i| i + at)
    {
        let close = if xml[start..].starts_with("<xdr:two") {
            "</xdr:twoCellAnchor>"
        } else {
            "</xdr:oneCellAnchor>"
        };
        let Some(end) = xml[start..].find(close).map(|i| i + start + close.len()) else {
            break;
        };
        out.push((start, end));
        at = end;
    }
    out
}

pub fn rewrite(xml: &str, layout: &Layout) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;
    for (start, end) in anchors(xml) {
        out.push_str(&xml[last..start]);
        last = end;
        let part = &xml[start..end];
        if part.contains("<xdr:sp ") || part.contains("<xdr:sp>") {
            continue;
        }
        let marker = part.find(PICTURE_MARKER).and_then(|at| {
            let digits: String = part[at + PICTURE_MARKER.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse::<usize>().ok()
        });
        let picture = marker.and_then(|i| layout.pictures.get(i).map(|p| (i, p)));
        let body = part
            .find("<xdr:pic>")
            .zip(part.find("</xdr:pic>").map(|i| i + "</xdr:pic>".len()));
        match (picture, body) {
            (Some((index, picture)), Some((from, to))) => {
                let pic =
                    part[from..to].replacen(&format!(" descr=\"{PICTURE_MARKER}{index}\""), "", 1);
                one_cell(picture.anchor, picture.offset, picture.size, &pic, &mut out);
            }
            _ => out.push_str(part),
        }
    }
    let tail = &xml[last..];
    let close = tail.rfind("</xdr:wsDr>").unwrap_or(tail.len());
    out.push_str(&tail[..close]);
    for (index, shape) in layout.shapes.iter().enumerate() {
        let body = shape_xml(shape, FIRST_SHAPE_ID + index);
        one_cell(shape.anchor, shape.offset, shape.size, &body, &mut out);
    }
    out.push_str(&tail[close..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const THEME: [u32; 12] = [
        0x000000, 0xFFFFFF, 0x44546A, 0xE7E6E6, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5,
        0x70AD47, 0x0563C1, 0x954F72,
    ];

    fn wrap(body: &str) -> String {
        format!("<xdr:wsDr xmlns:xdr=\"x\" xmlns:a=\"a\" xmlns:r=\"r\">{body}</xdr:wsDr>")
    }

    #[test]
    fn written_shapes_read_back() {
        let mut layout = Layout::default();
        let mut star = Shape::new(ShapeKind::Star5, "C3".parse().unwrap());
        star.offset = (4, 2);
        star.size = (120, 80);
        star.text = "Merhaba & hoş\n\nİkinci".to_owned();
        star.bold = true;
        star.font_size = 140;
        star.align = TextAlign::Right;
        star.text_anchor = TextAnchor::Bottom;
        star.fill = Some(Color(0x70AD47));
        star.line = None;
        let mut arrow = Shape::new(ShapeKind::DoubleArrow, "A1".parse().unwrap());
        arrow.flip_v = true;
        arrow.line_width = 225;
        let text_box = Shape::new(ShapeKind::TextBox, "F9".parse().unwrap());
        layout.shapes = vec![star, arrow, text_box];
        let xml = rewrite(&wrap(""), &layout);
        let read: Vec<Shape> = drawing_anchors(&xml, &THEME)
            .iter_mut()
            .filter_map(|a| a.shape(&layout))
            .collect();
        assert_eq!(read, layout.shapes);
    }

    #[test]
    fn reads_theme_styled_shapes() {
        let xml = wrap(
            r#"<xdr:twoCellAnchor><xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
            <xdr:to><xdr:col>2</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>4</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
            <xdr:sp macro="" textlink=""><xdr:nvSpPr><xdr:cNvPr id="2" name="Oval 1"/><xdr:cNvSpPr/></xdr:nvSpPr>
            <xdr:spPr><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom></xdr:spPr>
            <xdr:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef>
            <a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef>
            <a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></xdr:style>
            <xdr:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="tr-TR" sz="1100"/><a:t>Tamam</a:t></a:r></a:p></xdr:txBody>
            </xdr:sp><xdr:clientData/></xdr:twoCellAnchor>
            <xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
            <xdr:to><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
            <xdr:pic><xdr:blipFill><a:blip r:embed="rId3"/></xdr:blipFill></xdr:pic><xdr:clientData/></xdr:twoCellAnchor>"#,
        );
        let mut anchors = drawing_anchors(&xml, &THEME);
        let layout = Layout::default();
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[1].picture.as_deref(), Some("rId3"));
        let shape = anchors[0].shape(&layout).unwrap();
        assert_eq!(shape.kind, ShapeKind::Ellipse);
        assert_eq!(shape.anchor, "B3".parse().unwrap());
        assert_eq!(shape.size, (64, 40));
        assert_eq!(shape.fill, Some(Color(0x4472C4)));
        assert_eq!(shape.line, Some(Color(0x223962)));
        assert_eq!(shape.line_width, 100);
        assert_eq!(shape.color, Color(0xFFFFFF));
        assert_eq!(shape.text, "Tamam");
        assert_eq!(shape.align, TextAlign::Center);
        assert_eq!(shape.text_anchor, TextAnchor::Middle);
    }

    #[test]
    fn rewrite_moves_pictures_and_drops_placeholders() {
        let mut layout = Layout::default();
        let png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut picture = Picture::new(png, "D5".parse().unwrap(), (30, 20)).unwrap();
        picture.offset = (3, 0);
        layout.pictures.push(picture);
        let xml = wrap(&format!(
            "<xdr:twoCellAnchor editAs=\"oneCell\"><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>\
             <xdr:to><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>\
             <xdr:pic><xdr:nvPicPr><xdr:cNvPr id=\"2\" name=\"Picture 1\" descr=\"{PICTURE_MARKER}0\"/></xdr:nvPicPr><xdr:blipFill><a:blip r:embed=\"rId1\"/></xdr:blipFill></xdr:pic><xdr:clientData/></xdr:twoCellAnchor>\
             <xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>\
             <xdr:to><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>\
             <xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"3\" name=\"TextBox 2\"/><xdr:cNvSpPr txBox=\"1\"/></xdr:nvSpPr></xdr:sp><xdr:clientData/></xdr:twoCellAnchor>"
        ));
        let out = rewrite(&xml, &layout);
        assert!(!out.contains(PICTURE_MARKER));
        assert!(!out.contains("<xdr:sp "));
        let anchors = drawing_anchors(&out, &THEME);
        assert_eq!(anchors.len(), 1);
        let read = anchors[0]
            .picture(layout.pictures[0].data.to_vec(), &layout)
            .unwrap();
        assert_eq!(read, layout.pictures[0]);
    }
}
