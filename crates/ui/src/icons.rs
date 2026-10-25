use std::sync::Arc;

use eframe::egui::{
    Color32, CornerRadius, FontFamily, FontId, Galley, Painter, Pos2, Rect, Shape, Stroke,
    StrokeKind, Vec2, pos2, vec2,
};

use crate::fluent::{self, Glyph};
use crate::{fonts, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Paste,
    Cut,
    Copy,
    FormatPainter,
    Bold,
    Italic,
    Underline,
    GrowFont,
    ShrinkFont,
    Borders,
    Fill,
    FontColor,
    AlignTop,
    AlignMiddle,
    AlignBottom,
    AlignLeft,
    AlignCenter,
    AlignRight,
    Wrap,
    Merge,
    Currency,
    Percent,
    Thousands,
    MoreDecimals,
    FewerDecimals,
    AutoSum,
    Clear,
    CellFormat,
    Function,
    Save,
    Undo,
    Redo,
    SortAsc,
    SortDesc,
    Sort,
    Filter,
    ClearFilter,
    Reapply,
    Freeze,
    Find,
    Conditional,
    CellStyles,
    ChartColumn,
    ChartBar,
    ChartLine,
    ChartPie,
    ChartScatter,
    Plain(Glyph),
    Tinted(Glyph, Color32),
}

enum Look {
    Glyph(Glyph, Option<Color32>),
    Letter(&'static str, &'static str, bool),
}

fn look(icon: Icon) -> Look {
    let tinted = |glyph, tint| Look::Glyph(glyph, Some(tint));
    let plain = |glyph| Look::Glyph(glyph, None);
    match icon {
        Icon::Paste => tinted(fluent::CLIPBOARD_PASTE, theme::tint_clipboard()),
        Icon::Cut => plain(fluent::CUT),
        Icon::Copy => tinted(fluent::COPY, theme::tint_blue()),
        Icon::FormatPainter => tinted(fluent::PAINT_BRUSH, theme::tint_yellow()),
        Icon::Bold => Look::Letter(
            if crate::english() { "B" } else { "K" },
            fonts::CHROME_BOLD,
            false,
        ),
        Icon::Italic => Look::Letter(
            if crate::english() { "I" } else { "T" },
            fonts::CHROME_ITALIC,
            false,
        ),
        Icon::Underline => Look::Letter(
            if crate::english() { "U" } else { "A" },
            fonts::CHROME,
            true,
        ),
        Icon::GrowFont => plain(fluent::FONT_INCREASE),
        Icon::ShrinkFont => plain(fluent::FONT_DECREASE),
        Icon::Borders => plain(fluent::BORDER_BOTTOM),
        Icon::Fill => plain(fluent::PAINT_BUCKET),
        Icon::FontColor => plain(fluent::TEXT_COLOR),
        Icon::AlignTop => plain(fluent::ALIGN_TOP),
        Icon::AlignMiddle => plain(fluent::ALIGN_MIDDLE),
        Icon::AlignBottom => plain(fluent::ALIGN_BOTTOM),
        Icon::AlignLeft => plain(fluent::TEXT_ALIGN_LEFT),
        Icon::AlignCenter => plain(fluent::TEXT_ALIGN_CENTER),
        Icon::AlignRight => plain(fluent::TEXT_ALIGN_RIGHT),
        Icon::Wrap => plain(fluent::TEXT_WRAP),
        Icon::Merge => tinted(fluent::TABLE_CELLS_MERGE, theme::tint_blue()),
        Icon::Currency => tinted(fluent::MONEY, theme::tint_green()),
        Icon::Percent => Look::Letter("%", fonts::CHROME, false),
        Icon::Thousands => Look::Letter("000", fonts::CHROME_BOLD, false),
        Icon::MoreDecimals => plain(fluent::DECIMAL_LEFT),
        Icon::FewerDecimals => plain(fluent::DECIMAL_RIGHT),
        Icon::AutoSum => plain(fluent::AUTOSUM),
        Icon::Clear => tinted(fluent::ERASER, theme::tint_pink()),
        Icon::CellFormat => tinted(fluent::TABLE_SETTINGS, theme::tint_green()),
        Icon::Function => plain(fluent::MATH_FORMULA),
        Icon::Save => tinted(fluent::SAVE, theme::tint_purple()),
        Icon::Undo => plain(fluent::ARROW_UNDO),
        Icon::Redo => plain(fluent::ARROW_REDO),
        Icon::SortAsc => plain(fluent::SORT_ASCENDING),
        Icon::SortDesc => plain(fluent::SORT_DESCENDING),
        Icon::Sort => plain(fluent::SORT),
        Icon::Filter => plain(fluent::FILTER),
        Icon::ClearFilter => plain(fluent::FILTER_DISMISS),
        Icon::Reapply => plain(fluent::FILTER_SYNC),
        Icon::Freeze => tinted(fluent::TABLE_FREEZE, theme::tint_blue()),
        Icon::Find => plain(fluent::SEARCH),
        Icon::Conditional => tinted(fluent::TABLE_LIGHTNING, theme::tint_orange()),
        Icon::CellStyles => tinted(fluent::TABLE_CELL_EDIT, theme::tint_orange()),
        Icon::ChartColumn => tinted(fluent::DATA_BAR_VERTICAL, theme::tint_blue()),
        Icon::ChartBar => tinted(fluent::DATA_BAR_HORIZONTAL, theme::tint_blue()),
        Icon::ChartLine => tinted(fluent::DATA_LINE, theme::tint_blue()),
        Icon::ChartPie => tinted(fluent::DATA_PIE, theme::tint_blue()),
        Icon::ChartScatter => tinted(fluent::DATA_SCATTER, theme::tint_blue()),
        Icon::Plain(glyph) => plain(glyph),
        Icon::Tinted(glyph, tint) => tinted(glyph, tint),
    }
}

pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32, bar: Option<Color32>) {
    paint_look(painter, rect, look(icon), color, bar);
}

pub fn paint_muted(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let muted = match look(icon) {
        Look::Glyph(glyph, _) => Look::Glyph(glyph, None),
        letter => letter,
    };
    paint_look(painter, rect, muted, color, None);
}

fn paint_look(painter: &Painter, rect: Rect, look: Look, color: Color32, bar: Option<Color32>) {
    let size = rect.width().min(rect.height());
    match look {
        Look::Glyph(glyph, tint) => {
            let large = size > theme::SMALL_ICON_LIMIT;
            let outline = if large { glyph.large } else { glyph.small };
            let outline = layout(painter, &outline.to_string(), size, fonts::FLUENT, color);
            let pos = rect.center() - ink_center(&outline);
            if let Some(tint) = tint {
                let fill = if large {
                    glyph.large_fill
                } else {
                    glyph.small_fill
                };
                let fill = layout(painter, &fill.to_string(), size, fonts::FLUENT_FILLED, tint);
                painter.galley(pos, fill, tint);
            }
            painter.galley(pos, outline, color);
        }
        Look::Letter(text, family, underline) => {
            let scale = theme::LETTER_ICON_SCALE / (text.chars().count() as f32).sqrt();
            let galley = layout(painter, text, size * scale, family, color);
            let pos = rect.center() - ink_center(&galley);
            let width = galley.size().x;
            painter.galley(pos, galley, color);
            if underline {
                let y = rect.bottom() - size * 0.12;
                painter.hline(
                    rect.center().x - width / 2.0..=rect.center().x + width / 2.0,
                    y,
                    Stroke::new(1.0, color),
                );
            }
        }
    }
    if let Some(bar) = bar {
        let height = (size * theme::COLOR_BAR_SCALE).max(2.0);
        let strip = Rect::from_min_max(
            pos2(rect.left() + size * 0.1, rect.bottom() - height),
            pos2(rect.right() - size * 0.1, rect.bottom()),
        );
        painter.rect_filled(strip, CornerRadius::ZERO, bar);
    }
}

fn layout(painter: &Painter, text: &str, size: f32, family: &str, color: Color32) -> Arc<Galley> {
    let font = FontId::new(size, FontFamily::Name(family.into()));
    painter.layout_no_wrap(text.to_owned(), font, color)
}

fn ink_center(galley: &Galley) -> Vec2 {
    galley
        .rows
        .iter()
        .flat_map(|row| {
            row.glyphs
                .iter()
                .filter(|g| !g.uv_rect.is_nothing())
                .map(|g| {
                    let min = row.pos + g.pos.to_vec2() + g.uv_rect.offset;
                    Rect::from_min_size(min, g.uv_rect.size)
                })
        })
        .reduce(|a, b| a.union(b))
        .map_or(galley.size() / 2.0, |ink| ink.center().to_vec2())
}

fn draw(painter: &Painter, rect: Rect, glyph: char, size: f32, family: &str, color: Color32) {
    let galley = layout(painter, &glyph.to_string(), size, family, color);
    painter.galley(rect.center() - ink_center(&galley), galley, color);
}

pub fn funnel(painter: &Painter, rect: Rect, color: Color32) {
    draw(
        painter,
        rect,
        fluent::FILTER.small_fill,
        rect.height(),
        fonts::FLUENT_FILLED,
        color,
    );
}

pub fn chevron(painter: &Painter, center: Pos2, color: Color32) {
    let s = theme::CHEVRON_SIZE;
    painter.add(Shape::line(
        vec![
            center + vec2(-s, -s / 2.0),
            center + vec2(0.0, s / 2.0),
            center + vec2(s, -s / 2.0),
        ],
        Stroke::new(1.0, color),
    ));
}

pub fn swatch(painter: &Painter, rect: Rect, color: Option<Color32>) {
    match color {
        Some(color) => {
            painter.rect_filled(rect, CornerRadius::ZERO, color);
        }
        None => {
            painter.rect_filled(rect, CornerRadius::ZERO, theme::input_bg());
            painter.line_segment(
                [
                    pos2(rect.left(), rect.bottom()),
                    pos2(rect.right(), rect.top()),
                ],
                Stroke::new(1.0, theme::icon_remove()),
            );
        }
    }
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme::swatch_border()),
        StrokeKind::Inside,
    );
}
