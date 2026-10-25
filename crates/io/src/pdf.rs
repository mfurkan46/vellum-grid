use std::collections::BTreeMap;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use subsetter::GlyphRemapper;
use vellum_grid_core::Color;

use crate::print::{Area, FontBook, Item, PrintPage};

const FALLBACK_FONT: &[u8] = b"Helvetica";
const STEM: f32 = 80.0;
const UNDERLINE: f32 = 0.1;
const STRIKE: f32 = 0.3;
const LINE: f32 = 0.05;

struct Embedded {
    remapper: GlyphRemapper,
    chars: BTreeMap<u16, char>,
}

pub fn write_pdf(pages: &[PrintPage], fonts: &FontBook) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut alloc = || {
        let id = Ref::new(next);
        next += 1;
        id
    };
    let catalog = alloc();
    let tree = alloc();
    let mut embedded: BTreeMap<usize, Embedded> = BTreeMap::new();
    let mut page_ids = Vec::new();
    let mut contents = Vec::new();
    for page in pages {
        let content = draw(page, fonts, &mut embedded);
        let page_id = alloc();
        let content_id = alloc();
        page_ids.push(page_id);
        contents.push((page_id, content_id, content, page.width, page.height));
    }
    let font_ids: BTreeMap<usize, Ref> = embedded.keys().map(|&id| (id, alloc())).collect();
    let fallback = alloc();
    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    for (page_id, content_id, content, width, height) in contents {
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, width, height))
            .parent(tree)
            .contents(content_id);
        let mut resources = page.resources();
        let mut fonts_dict = resources.fonts();
        for (&id, &font_ref) in &font_ids {
            fonts_dict.pair(Name(font_name(id).as_bytes()), font_ref);
        }
        fonts_dict.pair(Name(b"F0"), fallback);
        fonts_dict.finish();
        resources.finish();
        page.finish();
        pdf.stream(content_id, &content);
    }
    pdf.type1_font(fallback)
        .base_font(Name(FALLBACK_FONT))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    for (id, font_ref) in font_ids {
        let face = fonts.face(id);
        let Some(data) = &face.data else {
            continue;
        };
        let used = &embedded[&id];
        let Ok(subset) = subsetter::subset(data, 0, &used.remapper) else {
            continue;
        };
        let cid = alloc();
        let descriptor = alloc();
        let file = alloc();
        let cmap = alloc();
        let base = format!(
            "{}+{}",
            subset_tag(id),
            postscript_name(&face.key.family, face.key.bold, face.key.italic)
        );
        let base = Name(base.as_bytes());
        pdf.type0_font(font_ref)
            .base_font(base)
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(cid)
            .to_unicode(cmap);
        let units = face.units();
        let widths: Vec<f32> = used
            .remapper
            .remapped_gids()
            .map(|old| face.advance(old) * 1000.0)
            .collect();
        let mut font = pdf.cid_font(cid);
        font.subtype(CidFontType::Type2)
            .base_font(base)
            .system_info(system_info())
            .font_descriptor(descriptor)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        font.widths().consecutive(0, widths);
        font.finish();
        let mut flags = FontFlags::NON_SYMBOLIC;
        if face.key.italic {
            flags |= FontFlags::ITALIC;
        }
        let ascent = face.ascent() * 1000.0;
        let descent = -face.descent() * 1000.0;
        pdf.font_descriptor(descriptor)
            .name(base)
            .flags(flags)
            .bbox(Rect::new(0.0, descent, units.max(1000.0), ascent))
            .italic_angle(if face.key.italic { -12.0 } else { 0.0 })
            .ascent(ascent)
            .descent(descent)
            .cap_height(ascent * 0.7)
            .stem_v(STEM)
            .font_file2(file);
        pdf.stream(file, &subset)
            .pair(Name(b"Length1"), subset.len() as i32);
        let mut unicode = pdf_writer::types::UnicodeCmap::new(Name(b"Custom"), system_info());
        for (&glyph, &c) in &used.chars {
            unicode.pair(glyph, c);
        }
        pdf.cmap(cmap, &unicode.finish());
    }
    pdf.finish()
}

fn system_info() -> SystemInfo<'static> {
    SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    }
}

fn subset_tag(id: usize) -> String {
    (0..6)
        .map(|i| char::from(b'A' + ((id >> (i * 4)) & 15) as u8))
        .collect()
}

fn font_name(id: usize) -> String {
    format!("F{}", id + 1)
}

fn postscript_name(family: &str, bold: bool, italic: bool) -> String {
    let base: String = family
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let style = match (bold, italic) {
        (false, false) => "",
        (true, false) => "-Bold",
        (false, true) => "-Italic",
        (true, true) => "-BoldItalic",
    };
    format!("{base}{style}")
}

fn rgb(color: Color) -> (f32, f32, f32) {
    let [r, g, b] = color.rgb();
    (
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    )
}

fn draw(page: &PrintPage, fonts: &FontBook, embedded: &mut BTreeMap<usize, Embedded>) -> Vec<u8> {
    let mut content = Content::new();
    let flip = |y: f32| page.height - y;
    for item in &page.items {
        match item {
            Item::Fill { area, color } => {
                let (r, g, b) = rgb(*color);
                content.set_fill_rgb(r, g, b);
                content.rect(area.x, flip(area.y + area.h), area.w, area.h);
                content.fill_nonzero();
            }
            Item::Line {
                from,
                to,
                width,
                color,
                dash,
            } => {
                let (r, g, b) = rgb(*color);
                content.set_stroke_rgb(r, g, b);
                content.set_line_width(*width);
                match dash {
                    Some((on, off)) => content.set_dash_pattern([*on, *off], 0.0),
                    None => content.set_dash_pattern([], 0.0),
                };
                content.move_to(from.0, flip(from.1));
                content.line_to(to.0, flip(to.1));
                content.stroke();
            }
            Item::Text {
                x,
                baseline,
                text,
                font,
                size,
                color,
                underline,
                strike,
                clip,
            } => {
                content.save_state();
                if let Some(clip) = clip {
                    clip_to(&mut content, *clip, page.height);
                }
                let (r, g, b) = rgb(*color);
                content.set_fill_rgb(r, g, b);
                content.begin_text();
                let face = fonts.face(*font);
                let bytes = if face.data.is_some() {
                    let used = embedded.entry(*font).or_insert_with(|| Embedded {
                        remapper: GlyphRemapper::new(),
                        chars: BTreeMap::new(),
                    });
                    content.set_font(Name(font_name(*font).as_bytes()), *size);
                    let mut bytes = Vec::with_capacity(text.len() * 2);
                    for c in text.chars() {
                        let glyph = face.glyph(c).unwrap_or(0);
                        let new = used.remapper.remap(glyph);
                        used.chars.entry(new).or_insert(c);
                        bytes.extend_from_slice(&new.to_be_bytes());
                    }
                    bytes
                } else {
                    content.set_font(Name(b"F0"), *size);
                    text.chars()
                        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
                        .collect()
                };
                content.set_text_matrix([1.0, 0.0, 0.0, 1.0, *x, flip(*baseline)]);
                content.show(Str(&bytes));
                content.end_text();
                let width = fonts.width(*font, text, *size);
                for (on, offset) in [(*underline, -UNDERLINE), (*strike, STRIKE)] {
                    if on {
                        let y = flip(*baseline) + offset * size;
                        content.set_stroke_rgb(r, g, b);
                        content.set_line_width(size * LINE);
                        content.set_dash_pattern([], 0.0);
                        content.move_to(*x, y);
                        content.line_to(x + width, y);
                        content.stroke();
                    }
                }
                content.restore_state();
            }
        }
    }
    content.finish().into_vec()
}

fn clip_to(content: &mut Content, area: Area, height: f32) {
    content.rect(area.x, height - area.y - area.h, area.w, area.h);
    content.clip_nonzero();
    content.end_path();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{FontSource, Job, layout};
    use vellum_grid_core::{CellAddr, EN, Sheet, Workbook};

    struct NoFonts;

    impl FontSource for NoFonts {
        fn load(&self, _: &str, _: bool, _: bool) -> Option<Vec<u8>> {
            None
        }
    }

    struct SystemArial;

    impl FontSource for SystemArial {
        fn load(&self, _: &str, _: bool, _: bool) -> Option<Vec<u8>> {
            [
                "C:/Windows/Fonts/arial.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            ]
            .iter()
            .find_map(|path| std::fs::read(path).ok())
        }
    }

    fn book() -> Workbook {
        let mut sheet = Sheet::new("Rapor");
        for row in 0..120 {
            let addr = CellAddr::new(row, 0).unwrap();
            sheet.set_input(addr, &format!("Satır {row} — ğüşiöç"));
            sheet.set_input(CellAddr::new(row, 1).unwrap(), &(row * 3).to_string());
        }
        sheet.layout_mut().page.footer = "&CSayfa &P / &N".to_owned();
        Workbook::from_sheets(vec![sheet]).unwrap()
    }

    fn job() -> Job<'static> {
        Job {
            book: "Kitap",
            date: "19.10.2026",
            time: "12:00",
            locale: &EN,
            today: 0.0,
        }
    }

    #[test]
    fn lays_out_pages_with_footer_numbers() {
        let source = NoFonts;
        let mut fonts = FontBook::new(&source);
        let pages = layout(&book(), &[0], None, &job(), &mut fonts);
        assert_eq!(pages.len(), 3);
        let footer = pages[1]
            .items
            .iter()
            .any(|item| matches!(item, Item::Text { text, .. } if text == "Sayfa 2 / 3"));
        assert!(footer);
        let selection = Some("A1:B10".parse().unwrap());
        assert_eq!(
            layout(&book(), &[0], selection, &job(), &mut fonts).len(),
            1
        );
    }

    #[test]
    fn writes_a_pdf_with_or_without_fonts() {
        for source in [&NoFonts as &dyn FontSource, &SystemArial] {
            let mut fonts = FontBook::new(source);
            let pages = layout(&book(), &[0], None, &job(), &mut fonts);
            let bytes = write_pdf(&pages, &fonts);
            assert!(bytes.starts_with(b"%PDF-"));
            let text = String::from_utf8_lossy(&bytes);
            assert!(text.contains("/Count 3"));
        }
    }
}
