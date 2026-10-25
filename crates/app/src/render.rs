use std::sync::Arc;

use eframe::egui::epaint::TextShape;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Color32, ColorImage, Context, FontId, Galley, Id, Mesh, Painter, Pos2, Rect, Shape,
    Stroke, TextureHandle, TextureOptions, Vec2, pos2, vec2,
};
use vellum_grid_core::{
    Border, Borders, Cell, CellValue, Color, HAlign, LineStyle, Pattern, PatternKind, Script,
    Style, Underline, VAlign,
};

use crate::app::locale;
use crate::{fonts, theme};

pub struct Visual {
    pub galley: Arc<Galley>,
    pub align: HAlign,
    pub v_align: VAlign,
    pub size: f32,
    pub faux_bold: bool,
    pub overflow: bool,
    pub underline: Option<(Color32, Underline)>,
    pub indent: f32,
    pub angle: f32,
    pub shift: f32,
}

impl Visual {
    pub fn extent(&self) -> Vec2 {
        let size = self.galley.size();
        let (sin, cos) = self.angle.sin_cos();
        vec2(
            size.x * cos.abs() + size.y * sin.abs() + self.indent,
            size.x * sin.abs() + size.y * cos.abs(),
        )
    }
}

pub fn color(c: Color) -> Color32 {
    let [r, g, b] = c.rgb();
    Color32::from_rgb(r, g, b)
}

pub fn cell_color(c: Color) -> Color32 {
    theme::on_canvas(color(c))
}

pub fn visual(
    painter: &Painter,
    cell: &Cell,
    style: &Style,
    width: f32,
    show_formula: bool,
    zoom: f32,
) -> Option<Visual> {
    let font = fonts::cell_font(style, zoom);
    let mut font_id = font.id.clone();
    if style.script != Script::None {
        font_id.size *= theme::SCRIPT_SCALE;
    }
    let indent = if style.h_align.indents() {
        f32::from(style.indent) * theme::INDENT_STEP * zoom
    } else {
        0.0
    };
    let room = (width - 2.0 * theme::CELL_PADDING - indent).max(0.0);
    let measure = |text: &str, font: &FontId| {
        painter
            .layout_no_wrap(text.to_owned(), font.clone(), theme::cell_text())
            .size()
            .x
    };
    let hashes = |font: &FontId| "#".repeat((room / measure("#", font)).floor().max(1.0) as usize);
    let angle = -f32::from(style.angle()).to_radians();
    let rotated = angle != 0.0 || style.is_stacked();
    let (mut text, format_color, default_align, numeric) = match (&cell.value, &cell.formula) {
        (_, Some(_)) if show_formula => (cell.input_text_in(locale()), None, HAlign::Left, false),
        (CellValue::Empty, _) => return None,
        (CellValue::Number(n), _) if style.has_general_format() && !rotated => {
            let chars = (room / measure("0", &font_id)).floor() as usize;
            let text = locale()
                .format_number_fit(*n, chars)
                .unwrap_or_else(|| hashes(&font_id));
            (text, None, HAlign::Right, true)
        }
        (value, _) => {
            let rendered = locale().render(value, &style.number_format);
            let align = match value {
                CellValue::Number(_) => HAlign::Right,
                CellValue::Bool(_) | CellValue::Error(_) => HAlign::Center,
                _ => HAlign::Left,
            };
            let numeric = matches!(value, CellValue::Number(_));
            (rendered.text, rendered.color, align, numeric)
        }
    };
    let align = match style.h_align {
        HAlign::General => default_align,
        HAlign::CenterAcross => HAlign::Center,
        explicit => explicit,
    };
    let justify = matches!(style.h_align, HAlign::Justify | HAlign::Distributed)
        || matches!(style.v_align, VAlign::Justify | VAlign::Distributed);
    let wrap = (style.wrap || justify) && !numeric && !rotated;
    if style.is_stacked() {
        text = text
            .chars()
            .map(String::from)
            .collect::<Vec<_>>()
            .join("\n");
    }
    if style.h_align == HAlign::Fill && !text.is_empty() {
        let copies = (room / measure(&text, &font_id)).floor().max(1.0) as usize;
        text = text.repeat(copies);
    }
    if style.shrink && !wrap {
        let natural = measure(&text, &font_id);
        if natural > room && natural > 0.0 {
            font_id.size = (font_id.size * room / natural).max(1.0);
        }
    }
    let text_color = format_color
        .or(style.color)
        .map_or(theme::cell_text(), cell_color);
    let strike = if style.strike {
        Stroke::new(1.0, text_color)
    } else {
        Stroke::NONE
    };
    let layout = |text: String| {
        let mut job = LayoutJob::single_section(
            text,
            TextFormat {
                font_id: font_id.clone(),
                color: text_color,
                italics: font.faux_italic,
                strikethrough: strike,
                ..TextFormat::default()
            },
        );
        job.wrap.max_width = if wrap { room } else { f32::INFINITY };
        job.justify = justify && wrap;
        if style.is_stacked() {
            job.halign = egui::Align::Center;
        }
        painter.layout_job(job)
    };
    let mut galley = layout(text);
    if numeric && !rotated && galley.size().x > room {
        galley = layout(hashes(&font_id));
    }
    let shift = match style.script {
        Script::None => 0.0,
        Script::Super => -font_id.size * theme::SCRIPT_RISE,
        Script::Sub => font_id.size * theme::SCRIPT_DROP,
    };
    Some(Visual {
        galley,
        align,
        v_align: style.v_align,
        size: font_id.size,
        faux_bold: font.faux_bold,
        overflow: !wrap && !numeric && !rotated && align == HAlign::Left && indent == 0.0,
        underline: (style.underline != Underline::None).then_some((text_color, style.underline)),
        indent,
        angle,
        shift,
    })
}

pub fn position(visual: &Visual, rect: Rect) -> Pos2 {
    if visual.angle != 0.0 {
        return rotated_position(visual, rect);
    }
    let galley = &visual.galley;
    let x = match visual.align {
        HAlign::Right => rect.right() - theme::CELL_PADDING - galley.size().x - visual.indent,
        HAlign::Center => rect.center().x - galley.size().x / 2.0,
        _ => rect.left() + theme::CELL_PADDING + visual.indent,
    };
    let height = galley.size().y;
    let y = if galley.rows.len() > 1 {
        match visual.v_align {
            VAlign::Top | VAlign::Justify | VAlign::Distributed => rect.top() + 1.0,
            VAlign::Center => rect.center().y - height / 2.0,
            VAlign::Bottom => rect.bottom() - height - 1.0,
        }
    } else {
        let baseline = galley
            .rows
            .first()
            .and_then(|row| row.row.glyphs.first().map(|glyph| row.pos.y + glyph.pos.y))
            .unwrap_or(height);
        let ascent = visual.size * theme::FONT_ASCENT;
        let descent = visual.size * theme::FONT_DESCENT;
        let target = match visual.v_align {
            VAlign::Top | VAlign::Justify | VAlign::Distributed => {
                rect.top() + theme::TEXT_BOTTOM_GAP + ascent
            }
            VAlign::Center => rect.center().y + (ascent - descent) / 2.0,
            VAlign::Bottom => rect.bottom() - theme::TEXT_BOTTOM_GAP - descent,
        };
        target - baseline + visual.shift
    };
    pos2(x, y)
}

fn rotated_position(visual: &Visual, rect: Rect) -> Pos2 {
    let size = visual.galley.size();
    let rot = egui::emath::Rot2::from_angle(visual.angle);
    let corners = [
        Vec2::ZERO,
        rot * vec2(size.x, 0.0),
        rot * vec2(0.0, size.y),
        rot * size,
    ];
    let min = corners
        .iter()
        .fold(vec2(f32::MAX, f32::MAX), |a, c| a.min(*c));
    let extent = visual.extent() - vec2(visual.indent, 0.0);
    let pad = theme::CELL_PADDING;
    let left = match visual.align {
        HAlign::Right => rect.right() - pad - extent.x - visual.indent,
        HAlign::Center => rect.center().x - extent.x / 2.0,
        _ => rect.left() + pad + visual.indent,
    };
    let top = match visual.v_align {
        VAlign::Top | VAlign::Justify | VAlign::Distributed => rect.top() + 1.0,
        VAlign::Center => rect.center().y - extent.y / 2.0,
        VAlign::Bottom => rect.bottom() - 1.0 - extent.y,
    };
    pos2(left, top) - min
}

pub fn paint_text(painter: &Painter, visual: &Visual, pos: Pos2) {
    if visual.angle != 0.0 {
        painter.add(
            TextShape::new(pos, visual.galley.clone(), theme::cell_text()).with_angle(visual.angle),
        );
        return;
    }
    painter.galley(pos, visual.galley.clone(), theme::cell_text());
    if visual.faux_bold {
        painter.galley(
            pos + vec2(theme::FAUX_BOLD_OFFSET, 0.0),
            visual.galley.clone(),
            theme::cell_text(),
        );
    }
    let Some((color, kind)) = visual.underline else {
        return;
    };
    let stroke = Stroke::new(1.0, color);
    let accounting = matches!(
        kind,
        Underline::SingleAccounting | Underline::DoubleAccounting
    );
    let offset = if accounting {
        theme::ACCOUNTING_UNDERLINE_OFFSET
    } else {
        theme::UNDERLINE_OFFSET
    };
    let clip = painter.clip_rect();
    for row in &visual.galley.rows {
        let (Some(first), Some(last)) = (row.row.glyphs.first(), row.row.glyphs.last()) else {
            continue;
        };
        let y = (pos.y + row.pos.y + first.pos.y + visual.size * offset).round() + 0.5;
        let (left, right) = if accounting {
            (
                clip.left() + theme::CELL_PADDING,
                clip.right() - theme::CELL_PADDING,
            )
        } else {
            (
                pos.x + row.pos.x + first.pos.x,
                pos.x + row.pos.x + last.pos.x + last.advance_width,
            )
        };
        painter.hline(left..=right, y, stroke);
        if kind.is_double() {
            painter.hline(left..=right, y + theme::DOUBLE_UNDERLINE_GAP, stroke);
        }
    }
}

pub fn paint_borders(painter: &Painter, rect: Rect, borders: &Borders) {
    let sides = [
        (
            borders.top,
            rect.left_top(),
            rect.right_top(),
            vec2(0.0, 1.0),
        ),
        (
            borders.bottom,
            rect.left_bottom(),
            rect.right_bottom(),
            vec2(0.0, -1.0),
        ),
        (
            borders.left,
            rect.left_top(),
            rect.left_bottom(),
            vec2(1.0, 0.0),
        ),
        (
            borders.right,
            rect.right_top(),
            rect.right_bottom(),
            vec2(-1.0, 0.0),
        ),
    ];
    for (border, a, b, inward) in sides {
        if let Some(border) = border {
            paint_line(painter, a, b, inward, border);
        }
    }
    for (border, up) in borders.diagonals() {
        let (a, b) = if up {
            (rect.left_bottom(), rect.right_top())
        } else {
            (rect.left_top(), rect.right_bottom())
        };
        let normal = (b - a).normalized().rot90();
        paint_line(painter, a, b, normal, border);
    }
}

pub fn paint_line(painter: &Painter, a: Pos2, b: Pos2, inward: Vec2, border: Border) {
    let stroke = Stroke::new(f32::from(border.style.weight()), cell_color(border.color));
    if border.style == LineStyle::Double {
        let thin = Stroke::new(1.0, stroke.color);
        painter.line_segment([a - inward, b - inward], thin);
        painter.line_segment([a + inward, b + inward], thin);
        return;
    }
    let dashes = border.style.dashes();
    if dashes.is_empty() {
        painter.line_segment([a, b], stroke);
        return;
    }
    if let [on, off, ..] = dashes
        && *on < 2.0
    {
        let length = (b - a).length();
        let direction = (b - a) / length.max(1.0);
        let mut dots = Mesh::default();
        let mut t = 0.0;
        while t <= length {
            let p = a + direction * t;
            let dot = Rect::from_min_size(pos2(p.x.floor(), p.y.floor()), vec2(1.0, 1.0));
            dots.add_colored_rect(dot, stroke.color);
            t += on + off;
        }
        painter.add(Shape::mesh(dots));
        return;
    }
    let on: Vec<f32> = dashes.iter().step_by(2).copied().collect();
    let off: Vec<f32> = dashes.iter().skip(1).step_by(2).copied().collect();
    painter.extend(Shape::dashed_line_with_offset(
        &[a, b],
        stroke,
        &on,
        &off,
        0.0,
    ));
}

fn pattern_texture(ctx: &Context, kind: PatternKind) -> TextureHandle {
    let id = Id::unique(("cell-pattern", kind));
    if let Some(texture) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return texture;
    }
    let tile = kind.tile();
    let pixels = (0..64)
        .map(|i| {
            if tile[i / 8] & (0x80 >> (i % 8)) != 0 {
                Color32::WHITE
            } else {
                Color32::TRANSPARENT
            }
        })
        .collect();
    let image = ColorImage::new([8, 8], pixels);
    let options = TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Nearest,
        wrap_mode: egui::TextureWrapMode::Repeat,
        mipmap_mode: None,
    };
    let texture = ctx.load_texture(format!("pattern-{}", kind.key()), image, options);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

pub fn paint_pattern(painter: &Painter, rect: Rect, pattern: Pattern) {
    let texture = pattern_texture(painter.ctx(), pattern.kind);
    let mut mesh = Mesh::with_texture(texture.id());
    let tile = theme::PATTERN_TILE;
    let uv = Rect::from_min_max(
        pos2(rect.left() / tile, rect.top() / tile),
        pos2(rect.right() / tile, rect.bottom() / tile),
    );
    mesh.add_rect_with_uv(rect, uv, cell_color(pattern.color));
    painter.add(Shape::mesh(mesh));
}
