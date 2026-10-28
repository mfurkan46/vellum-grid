use std::fmt::Debug;
use std::hash::Hash;
use std::path::Path;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Id, Margin, Rect, Response,
    RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::icons::{self, Icon};
use crate::{fluent, fonts, motion, settings, text, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Nav,
    Page,
}

/// The File screen: `add` draws the accent navigation column, which slides in from the left,
/// and the current `page`, which slides in from the right. `shown` is the opening animation;
/// `pages` lists every page so switching between them animates.
pub fn screen<K: Hash + Debug + Copy + Eq + Send + Sync + 'static>(
    ui: &mut Ui,
    shown: f32,
    pages: &[K],
    page: K,
    mut add: impl FnMut(&mut Ui, Part),
) {
    egui::Panel::left("backstage-nav")
        .exact_size(theme::BACKSTAGE_NAV_WIDTH)
        .resizable(false)
        .frame(Frame::NONE.fill(theme::surface()))
        .show(ui, |ui| {
            let slide = vec2(-theme::BACKSTAGE_NAV_WIDTH, 0.0);
            motion::revealed(ui, shown, slide, |ui| {
                ui.painter()
                    .rect_filled(ui.max_rect(), CornerRadius::ZERO, theme::backstage_nav());
                add(ui, Part::Nav);
            });
        });
    egui::CentralPanel::no_frame().show(ui, |ui| {
        Frame::NONE
            .fill(theme::surface())
            .inner_margin(Margin::symmetric(
                theme::BACKSTAGE_PADDING_X,
                theme::BACKSTAGE_PADDING_Y,
            ))
            .show(ui, |ui| {
                let mut current = shown;
                for &other in pages {
                    let t = motion::reveal(
                        ui.ctx(),
                        Id::unique(("backstage-page", other)),
                        other == page,
                        theme::BACKSTAGE_ANIMATION,
                    );
                    if other == page {
                        current = current.min(t);
                    }
                }
                let slide = vec2(theme::BACKSTAGE_SLIDE, 0.0);
                motion::revealed(ui, current, slide, |ui| {
                    ui.set_min_size(ui.available_size());
                    add(ui, Part::Page);
                });
            });
    });
}

/// The back arrow at the top of the File screen navigation.
pub fn back_row(ui: &mut Ui) -> bool {
    ui.add_space(theme::BACKSTAGE_NAV_TOP);
    let back = nav_row(ui, Some(fluent::ARROW_LEFT), "", false);
    ui.add_space(theme::BACKSTAGE_NAV_TOP);
    back.on_hover_text(text::back()).clicked()
}

pub fn page_title(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .size(theme::BACKSTAGE_TITLE)
            .color(theme::chrome_text()),
    );
    ui.add_space(theme::BACKSTAGE_GAP);
}

pub fn nav_row(ui: &mut Ui, glyph: Option<fluent::Glyph>, label: &str, selected: bool) -> Response {
    let size = vec2(ui.available_width(), theme::BACKSTAGE_NAV_ITEM);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::ZERO, theme::backstage_nav_selected());
    } else {
        let hover = crate::motion::hover_color(
            ui,
            &response,
            response.hovered(),
            theme::backstage_nav_hover(),
        );
        painter.rect_filled(rect, CornerRadius::ZERO, hover);
    }
    if let Some(glyph) = glyph {
        let area = Rect::from_center_size(
            pos2(
                rect.left() + theme::BACKSTAGE_NAV_ICON_COLUMN / 2.0,
                rect.center().y,
            ),
            vec2(theme::ICON_SIZE, theme::ICON_SIZE),
        );
        icons::paint(painter, area, Icon::Plain(glyph), theme::ON_ACCENT, None);
    }
    painter.text(
        pos2(
            rect.left() + theme::BACKSTAGE_NAV_ICON_COLUMN,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        label,
        theme::backstage_font(),
        theme::ON_ACCENT,
    );
    response
}

pub fn separator(ui: &mut Ui) {
    nav_line(ui, theme::BACKSTAGE_NAV_LINE);
}

pub fn side_separator(ui: &mut Ui) {
    nav_line(ui, theme::chrome_line());
}

fn nav_line(ui: &mut Ui, color: Color32) {
    ui.add_space(theme::DIALOG_SPACING);
    let rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        rect.left() + theme::BACKSTAGE_NAV_ICON_COLUMN / 2.0..=rect.right() - theme::DIALOG_SPACING,
        rect.top(),
        Stroke::new(1.0, color),
    );
    ui.add_space(theme::DIALOG_SPACING);
}

pub fn side_row(ui: &mut Ui, glyph: fluent::Glyph, label: &str, selected: bool) -> Response {
    let size = vec2(ui.available_width(), theme::BACKSTAGE_NAV_ITEM);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let row = rect.shrink2(vec2(theme::SIDE_ROW_INSET, theme::SIDE_ROW_GAP));
    let fill = if selected {
        theme::tab_hover()
    } else {
        crate::motion::hover_color(ui, &response, response.hovered(), theme::hover_fill())
    };
    let painter = ui.painter();
    painter.rect_filled(row, CornerRadius::same(theme::MENU_RADIUS), fill);
    if selected {
        let marker = Rect::from_min_size(
            pos2(row.left(), row.center().y - theme::SIDE_MARKER_HEIGHT / 2.0),
            vec2(theme::SIDE_MARKER_WIDTH, theme::SIDE_MARKER_HEIGHT),
        );
        painter.rect_filled(marker, CornerRadius::same(2), theme::accent());
    }
    let area = Rect::from_center_size(
        pos2(
            rect.left() + theme::BACKSTAGE_NAV_ICON_COLUMN / 2.0,
            rect.center().y,
        ),
        vec2(theme::ICON_SIZE, theme::ICON_SIZE),
    );
    icons::paint(painter, area, Icon::Plain(glyph), theme::icon(), None);
    painter.text(
        pos2(
            rect.left() + theme::BACKSTAGE_NAV_ICON_COLUMN,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        label,
        theme::backstage_font(),
        theme::chrome_text(),
    );
    response
}

pub fn recent_row(ui: &mut Ui, path: &Path, icon: Icon) -> Response {
    let width = ui.available_width().min(theme::RECENT_MAX_WIDTH);
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, theme::RECENT_ROW_HEIGHT), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(
            rect,
            CornerRadius::same(theme::MENU_RADIUS),
            theme::tool_hover(),
        );
    }
    let area = Rect::from_center_size(
        pos2(
            rect.left() + theme::RECENT_ICON_COLUMN / 2.0,
            rect.center().y,
        ),
        vec2(theme::BIG_ICON_SIZE, theme::BIG_ICON_SIZE),
    );
    icons::paint(painter, area, icon, theme::accent(), None);
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let folder = path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let x = rect.left() + theme::RECENT_ICON_COLUMN;
    painter.text(
        pos2(x, rect.center().y - 1.0),
        Align2::LEFT_BOTTOM,
        name,
        FontId::new(
            theme::MENU_FONT_SIZE,
            FontFamily::Name(fonts::CHROME_BOLD.into()),
        ),
        theme::chrome_text(),
    );
    painter.text(
        pos2(x, rect.center().y + 1.0),
        Align2::LEFT_TOP,
        folder,
        theme::small_font(),
        theme::chrome_text_muted(),
    );
    if let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) {
        painter.text(
            pos2(rect.right() - theme::DIALOG_SPACING, rect.center().y),
            Align2::RIGHT_CENTER,
            settings::ago(modified),
            theme::small_font(),
            theme::chrome_text_muted(),
        );
    }
    response.on_hover_text(path.display().to_string())
}

pub fn wide_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let width = wide_width(ui, &[label]);
    sized_button(ui, icon, label, width)
}

pub fn wide_width(ui: &Ui, labels: &[&str]) -> f32 {
    labels
        .iter()
        .map(|label| {
            let galley = ui.painter().layout_no_wrap(
                (*label).to_owned(),
                theme::backstage_font(),
                theme::chrome_text(),
            );
            theme::RECENT_ICON_COLUMN + galley.size().x + theme::WIDE_BUTTON_PADDING
        })
        .fold(theme::WIDE_BUTTON_WIDTH, f32::max)
}

pub fn sized_button(ui: &mut Ui, icon: Icon, label: &str, width: f32) -> Response {
    let size = vec2(width, theme::WIDE_BUTTON_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let fill = if response.hovered() {
        theme::tool_hover()
    } else {
        theme::ribbon_bg()
    };
    painter.rect(
        rect,
        CornerRadius::same(theme::MENU_RADIUS),
        fill,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
    let area = Rect::from_center_size(
        pos2(
            rect.left() + theme::RECENT_ICON_COLUMN / 2.0,
            rect.center().y,
        ),
        vec2(theme::BIG_ICON_SIZE, theme::BIG_ICON_SIZE),
    );
    icons::paint(painter, area, icon, theme::icon(), None);
    painter.text(
        pos2(rect.left() + theme::RECENT_ICON_COLUMN, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::backstage_font(),
        theme::chrome_text(),
    );
    response
}
