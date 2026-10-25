use eframe::egui::containers::menu::{MenuState, SubMenu};
use eframe::egui::{self, CornerRadius, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

use crate::icons::{self, Icon};
use crate::{fluent, text, theme};

enum Trailing<'a> {
    None,
    Shortcut(&'a str),
    Arrow,
}

pub fn item(ui: &mut Ui, label: &str) -> bool {
    row(ui, None, label, Trailing::None, false).clicked()
}

pub fn hint_item(ui: &mut Ui, label: &str, hint: &str) -> bool {
    row(ui, None, label, Trailing::None, false)
        .on_hover_text(hint)
        .clicked()
}

pub fn icon_item(ui: &mut Ui, icon: Icon, label: &str) -> bool {
    row(ui, Some(icon), label, Trailing::None, false).clicked()
}

pub fn shortcut_item(ui: &mut Ui, icon: Option<Icon>, label: &str, shortcut: &str) -> bool {
    row(ui, icon, label, Trailing::Shortcut(shortcut), false).clicked()
}

pub fn icon_or_blank(ui: &mut Ui, icon: Option<Icon>, label: &str) -> bool {
    row(ui, icon, label, Trailing::None, false).clicked()
}

pub fn detail_item(ui: &mut Ui, icon: Option<Icon>, label: &str, detail: &str) -> bool {
    let painter = ui.painter().clone();
    let title = painter.layout_no_wrap(label.to_owned(), theme::menu_font(), theme::chrome_text());
    let note = painter.layout_no_wrap(
        detail.to_owned(),
        theme::small_font(),
        theme::chrome_text_muted(),
    );
    let width =
        theme::MENU_ICON_COLUMN + title.size().x.max(note.size().x) + theme::MENU_ITEM_PADDING;
    let response = ui.allocate_response(vec2(width, theme::MENU_DETAIL_HEIGHT), Sense::click());
    let rect = response.rect;
    if response.hovered() || response.has_focus() {
        painter.rect_filled(
            rect,
            CornerRadius::same(theme::MENU_RADIUS),
            theme::tool_hover(),
        );
    }
    if let Some(icon) = icon {
        let area = Rect::from_center_size(
            pos2(rect.left() + theme::MENU_ICON_COLUMN / 2.0, rect.center().y),
            vec2(theme::MENU_ICON, theme::MENU_ICON),
        );
        icons::paint(&painter, area, icon, theme::icon(), None);
    }
    let x = rect.left() + theme::MENU_ICON_COLUMN;
    let top = rect.center().y - (title.size().y + note.size().y) / 2.0;
    let below = top + title.size().y;
    painter.galley(pos2(x, top), title, theme::chrome_text());
    painter.galley(pos2(x, below), note, theme::chrome_text_muted());
    response.clicked()
}

pub fn enabled_item(ui: &mut Ui, enabled: bool, icon: Option<Icon>, label: &str) -> bool {
    ui.add_enabled_ui(enabled, |ui| {
        row(ui, icon, label, Trailing::None, false).clicked()
    })
    .inner
}

pub fn soon(ui: &mut Ui, icon: Option<Icon>, label: &str) {
    ui.add_enabled_ui(false, |ui| {
        row(ui, icon, label, Trailing::None, false).on_disabled_hover_text(text::coming_soon());
    });
}

pub fn check(ui: &mut Ui, value: &mut bool, label: &str) {
    let icon = value.then_some(Icon::Plain(fluent::CHECKMARK));
    if row(ui, icon, label, Trailing::None, false).clicked() {
        *value = !*value;
    }
}

pub fn sub(ui: &mut Ui, icon: Option<Icon>, label: &str, add: impl FnOnce(&mut Ui)) {
    let id = ui.next_auto_id();
    let open = MenuState::from_ui(ui, |state, _| {
        state.open_item == Some(SubMenu::id_from_widget_id(id))
    });
    let response = row(ui, icon, label, Trailing::Arrow, open);
    SubMenu::new().show(ui, &response, |ui| {
        ui.set_min_width(theme::MENU_WIDTH);
        add(ui);
    });
}

pub fn arrow_item(ui: &mut Ui, label: &str, open: bool) -> Response {
    row(ui, None, label, Trailing::Arrow, open)
}

pub fn swatch_item(ui: &mut Ui, color: Option<u32>) -> bool {
    let response = ui.allocate_response(
        vec2(ui.available_width(), theme::MENU_ITEM_HEIGHT),
        Sense::click(),
    );
    let rect = response.rect;
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(
            rect,
            CornerRadius::same(theme::MENU_RADIUS),
            theme::tool_hover(),
        );
    }
    let swatch = Rect::from_min_size(
        pos2(
            rect.left() + theme::MENU_ITEM_PADDING,
            rect.center().y - theme::MENU_ICON / 2.0,
        ),
        vec2(
            rect.width() - 2.0 * theme::MENU_ITEM_PADDING,
            theme::MENU_ICON,
        ),
    );
    match color {
        Some(color) => {
            painter.rect_filled(swatch, CornerRadius::ZERO, crate::rgb(color));
        }
        None => {
            painter.rect_stroke(
                swatch,
                CornerRadius::ZERO,
                Stroke::new(1.0, theme::chrome_line()),
                egui::StrokeKind::Inside,
            );
            painter.text(
                swatch.center(),
                egui::Align2::CENTER_CENTER,
                text::no_fill(),
                theme::menu_font(),
                theme::chrome_text(),
            );
        }
    }
    response.clicked()
}

pub fn separator(ui: &mut Ui) {
    ui.add_space(theme::SEPARATOR_SPACING / 2.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        rect.x_range(),
        rect.top(),
        Stroke::new(1.0, theme::ribbon_line()),
    );
    ui.add_space(theme::SEPARATOR_SPACING / 2.0);
}

fn row(ui: &mut Ui, icon: Option<Icon>, label: &str, trailing: Trailing, open: bool) -> Response {
    let enabled = ui.is_enabled();
    let color = if enabled {
        theme::chrome_text()
    } else {
        theme::chrome_text_disabled()
    };
    let painter = ui.painter().clone();
    let galley = painter.layout_no_wrap(label.to_owned(), theme::menu_font(), color);
    let shortcut = match trailing {
        Trailing::Shortcut(keys) if !keys.is_empty() => Some(painter.layout_no_wrap(
            keys.to_owned(),
            theme::menu_font(),
            theme::chrome_text_muted(),
        )),
        _ => None,
    };
    let trailing_width = match (&trailing, &shortcut) {
        (_, Some(keys)) => theme::MENU_SHORTCUT_GAP + keys.size().x,
        (Trailing::Arrow, _) => theme::MENU_SHORTCUT_GAP,
        _ => 0.0,
    };
    let width =
        theme::MENU_ICON_COLUMN + galley.size().x + trailing_width + theme::MENU_ITEM_PADDING;
    let response = ui.allocate_response(vec2(width, theme::MENU_ITEM_HEIGHT), Sense::click());
    let rect = response.rect;
    if enabled && (open || response.hovered() || response.has_focus()) {
        painter.rect_filled(
            rect,
            CornerRadius::same(theme::MENU_RADIUS),
            theme::tool_hover(),
        );
    }
    if let Some(icon) = icon {
        let area = Rect::from_center_size(
            pos2(rect.left() + theme::MENU_ICON_COLUMN / 2.0, rect.center().y),
            vec2(theme::MENU_ICON, theme::MENU_ICON),
        );
        if enabled {
            icons::paint(&painter, area, icon, theme::icon(), None);
        } else {
            icons::paint_muted(&painter, area, icon, theme::icon_disabled());
        }
    }
    let y = rect.center().y - galley.size().y / 2.0;
    painter.galley(
        pos2(rect.left() + theme::MENU_ICON_COLUMN, y),
        galley,
        color,
    );
    let right = rect.right() - theme::MENU_ITEM_PADDING;
    if let Some(keys) = shortcut {
        let y = rect.center().y - keys.size().y / 2.0;
        painter.galley(
            pos2(right - keys.size().x, y),
            keys,
            theme::chrome_text_muted(),
        );
    } else if let Trailing::Arrow = trailing {
        let area = Rect::from_center_size(
            pos2(right - theme::MENU_ARROW / 2.0, rect.center().y),
            vec2(theme::MENU_ARROW, theme::MENU_ARROW),
        );
        icons::paint(
            &painter,
            area,
            Icon::Plain(fluent::CHEVRON_RIGHT),
            color,
            None,
        );
    }
    response
}
