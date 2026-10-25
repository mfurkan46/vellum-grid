use eframe::egui::{
    Align2, Color32, CornerRadius, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, pos2,
    vec2,
};

use crate::icons::{self, Icon};
use crate::{motion, palette, text, theme};

pub fn group(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    group_with_launcher(ui, label, None, add);
}

pub fn launcher_group(ui: &mut Ui, label: &str, hint: &str, add: impl FnOnce(&mut Ui)) -> bool {
    group_with_launcher(ui, label, Some(hint), add)
}

fn group_with_launcher(
    ui: &mut Ui,
    label: &str,
    launcher: Option<&str>,
    add: impl FnOnce(&mut Ui),
) -> bool {
    let top = ui.max_rect().top();
    let label_width = ui
        .painter()
        .layout_no_wrap(
            label.to_owned(),
            theme::small_font(),
            theme::chrome_text_muted(),
        )
        .size()
        .x
        + if launcher.is_some() {
            2.0 * theme::LAUNCHER_SIZE
        } else {
            0.0
        };
    let rect = ui
        .vertical(|ui| {
            ui.set_height(theme::RIBBON_CONTENT_HEIGHT);
            ui.set_min_width(label_width);
            add(ui);
        })
        .response
        .rect;
    let label_rect = Rect::from_min_max(
        pos2(rect.left(), top + theme::RIBBON_CONTENT_HEIGHT),
        pos2(
            rect.right(),
            top + theme::RIBBON_CONTENT_HEIGHT + theme::RIBBON_LABEL_HEIGHT,
        ),
    );
    let painter = ui.painter().clone();
    painter.text(
        label_rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::small_font(),
        theme::chrome_text_muted(),
    );
    let mut clicked = false;
    if let Some(hint) = launcher {
        let area = Rect::from_center_size(
            pos2(
                label_rect.right() - theme::LAUNCHER_SIZE / 2.0,
                label_rect.center().y,
            ),
            vec2(theme::LAUNCHER_SIZE, theme::LAUNCHER_SIZE),
        );
        let response = ui.interact(
            area,
            ui.scope_id().with(("launcher", label)),
            Sense::click(),
        );
        let hover = motion::hover_color(ui, &response, response.hovered(), theme::tool_hover());
        painter.rect_filled(area, CornerRadius::same(2), hover);
        let inner = area.shrink(3.5);
        let stroke = Stroke::new(1.0, theme::icon());
        painter.line_segment([inner.left_top(), inner.right_bottom()], stroke);
        painter.line_segment(
            [inner.right_bottom(), pos2(inner.right(), inner.top() + 3.0)],
            stroke,
        );
        painter.line_segment(
            [
                inner.right_bottom(),
                pos2(inner.left() + 3.0, inner.bottom()),
            ],
            stroke,
        );
        clicked = response.on_hover_text(hint).clicked();
    }
    ui.add_space(theme::RIBBON_GROUP_GAP / 2.0);
    let x = ui.cursor().left();
    painter.vline(
        x,
        top + 6.0..=label_rect.bottom() - 6.0,
        Stroke::new(1.0, theme::ribbon_line()),
    );
    ui.add_space(theme::RIBBON_GROUP_GAP / 2.0);
    clicked
}

pub fn soon(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.add_enabled_ui(false, add);
}

fn background(ui: &Ui, rect: Rect, response: &Response, checked: bool) {
    let painter = ui.painter();
    let radius = CornerRadius::same(theme::BUTTON_RADIUS as u8);
    if checked {
        painter.rect(
            rect,
            radius,
            theme::tool_checked(),
            Stroke::new(1.0, theme::tool_checked_line()),
            StrokeKind::Inside,
        );
    } else {
        let hot = response.enabled() && (response.hovered() || response.has_focus());
        painter.rect_filled(
            rect,
            radius,
            motion::hover_color(ui, response, hot, theme::tool_hover()),
        );
    }
}

fn paint_icon(ui: &Ui, rect: Rect, icon: Icon, bar: Option<Color32>) {
    if ui.is_enabled() {
        icons::paint(ui.painter(), rect, icon, theme::icon(), bar);
    } else {
        icons::paint_muted(ui.painter(), rect, icon, theme::icon_disabled());
    }
}

fn text_color(ui: &Ui) -> Color32 {
    if ui.is_enabled() {
        theme::chrome_text()
    } else {
        theme::chrome_text_disabled()
    }
}

fn hint(response: Response, tip: &str) -> Response {
    let unavailable = format!("{tip}\n{}", text::coming_soon());
    response
        .on_hover_text(tip)
        .on_disabled_hover_text(unavailable)
}

fn icon_rect(rect: Rect, size: f32) -> Rect {
    Rect::from_center_size(rect.center(), vec2(size, size))
}

pub fn tool(ui: &mut Ui, icon: Icon, tip: &str, checked: bool) -> Response {
    tool_with(ui, icon, tip, checked, None)
}

pub fn tool_with_bar(ui: &mut Ui, icon: Icon, tip: &str, bar: Color32) -> Response {
    tool_with(ui, icon, tip, false, Some(bar))
}

fn tool_with(ui: &mut Ui, icon: Icon, tip: &str, checked: bool, bar: Option<Color32>) -> Response {
    let size = vec2(theme::TOOL_SIZE, theme::SMALL_TOOL_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    background(ui, rect, &response, checked);
    paint_icon(ui, icon_rect(rect, theme::ICON_SIZE), icon, bar);
    hint(response, tip)
}

pub fn paste_option(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let size = vec2(theme::PASTE_OPTION_SIZE, theme::PASTE_OPTION_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    background(ui, rect, &response, false);
    paint_icon(ui, icon_rect(rect, theme::PASTE_OPTION_ICON), icon, None);
    hint(response, tip)
}

pub fn drop(ui: &mut Ui, tip: &str) -> Response {
    let size = vec2(theme::DROP_WIDTH, theme::SMALL_TOOL_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    background(ui, rect, &response, false);
    icons::chevron(ui.painter(), rect.center(), text_color(ui));
    hint(response, tip)
}

pub fn small(ui: &mut Ui, icon: Icon, label: &str, tip: &str, checked: bool) -> Response {
    let response = small_tool(ui, icon, label, checked, false);
    hint(response, tip)
}

pub fn small_menu(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let response = small_tool(ui, icon, label, false, true);
    hint(response, label)
}

fn small_tool(ui: &mut Ui, icon: Icon, label: &str, checked: bool, dropdown: bool) -> Response {
    let color = text_color(ui);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::chrome_font(), color);
    let arrow = if dropdown { theme::DROP_WIDTH } else { 0.0 };
    let width = theme::TOOL_SIZE + galley.size().x + theme::CHROME_SPACING + arrow;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, theme::SMALL_TOOL_HEIGHT), Sense::click());
    background(ui, rect, &response, checked);
    let icon_area = Rect::from_min_size(rect.min, vec2(theme::TOOL_SIZE, rect.height()));
    paint_icon(ui, icon_rect(icon_area, theme::ICON_SIZE), icon, None);
    let text_pos = pos2(icon_area.right(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(text_pos, galley, color);
    if dropdown {
        let center = pos2(rect.right() - theme::DROP_WIDTH / 2.0, rect.center().y);
        icons::chevron(ui.painter(), center, color);
    }
    response
}

pub fn small_drop(ui: &mut Ui, label: &str) -> Response {
    let color = text_color(ui);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::chrome_font(), color);
    let width = galley.size().x + theme::CHROME_SPACING + theme::DROP_WIDTH;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, theme::SMALL_TOOL_HEIGHT), Sense::click());
    background(ui, rect, &response, false);
    let text_pos = pos2(
        rect.left() + theme::CHROME_SPACING / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, color);
    icons::chevron(
        ui.painter(),
        pos2(rect.right() - theme::DROP_WIDTH / 2.0, rect.center().y),
        color,
    );
    hint(response, label)
}

pub fn big(ui: &mut Ui, icon: Icon, label: &str, tip: &str, dropdown: bool) -> Response {
    big_tool(ui, icon, label, tip, dropdown, false)
}

pub fn big_checked(ui: &mut Ui, icon: Icon, label: &str, tip: &str, checked: bool) -> Response {
    big_tool(ui, icon, label, tip, false, checked)
}

fn big_tool(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    tip: &str,
    dropdown: bool,
    checked: bool,
) -> Response {
    let color = text_color(ui);
    let measure = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), theme::chrome_font(), color)
    };
    let lines = split_label(label, |text| measure(text).size().x);
    let galleys: Vec<_> = lines.iter().map(|line| measure(line)).collect();
    let text_width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let width = (text_width + theme::CHROME_SPACING).max(theme::BIG_TOOL_WIDTH);
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, theme::RIBBON_CONTENT_HEIGHT), Sense::click());
    background(ui, rect, &response, checked);
    let icon_area = Rect::from_center_size(
        pos2(
            rect.center().x,
            rect.top() + 4.0 + theme::BIG_ICON_SIZE / 2.0,
        ),
        vec2(theme::BIG_ICON_SIZE, theme::BIG_ICON_SIZE),
    );
    paint_icon(ui, icon_area, icon, None);
    let mut y = icon_area.bottom() + 3.0;
    for galley in galleys {
        let height = galley.size().y;
        let pos = pos2(rect.center().x - galley.size().x / 2.0, y);
        ui.painter().galley(pos, galley, color);
        y += height - 3.0;
    }
    if dropdown {
        icons::chevron(
            ui.painter(),
            pos2(rect.center().x, y + 3.0 + theme::CHEVRON_SIZE),
            color,
        );
    }
    hint(response, tip)
}

fn split_label(label: &str, width: impl Fn(&str) -> f32) -> Vec<String> {
    if width(label) <= theme::BIG_LABEL_MAX {
        return vec![label.to_owned()];
    }
    let middle = label.len() / 2;
    label
        .match_indices(' ')
        .map(|(index, _)| index)
        .min_by_key(|index| index.abs_diff(middle))
        .map_or_else(
            || vec![label.to_owned()],
            |index| vec![label[..index].to_owned(), label[index + 1..].to_owned()],
        )
}

pub fn tip(label: &str, shortcut: &str) -> String {
    format!("{label} ({shortcut})")
}

pub fn chrome_text(s: &str) -> RichText {
    RichText::new(s)
        .font(theme::chrome_font())
        .color(theme::chrome_text())
}

pub fn palette_ui(ui: &mut Ui, none_label: &str) -> Option<Option<u32>> {
    let mut choice = None;
    if crate::menu::item(ui, none_label) {
        choice = Some(None);
    }
    ui.label(chrome_text(text::theme_colors()));
    for (index, row) in palette::theme_rows().iter().enumerate() {
        if index == 1 {
            ui.add_space(theme::SWATCH_GAP * 2.0);
        }
        if let Some(color) = swatch_row(ui, row) {
            choice = Some(Some(color));
        }
    }
    ui.label(chrome_text(text::standard_colors()));
    if let Some(color) = swatch_row(ui, &palette::STANDARD) {
        choice = Some(Some(color));
    }
    choice
}

fn swatch_row(ui: &mut Ui, colors: &[u32]) -> Option<u32> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme::SWATCH_GAP, 0.0);
        for color in colors {
            let size = vec2(theme::SWATCH_SIZE, theme::SWATCH_SIZE);
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            icons::swatch(ui.painter(), rect, Some(crate::rgb(*color)));
            if response.hovered() {
                ui.painter().rect_stroke(
                    rect.expand(1.0),
                    CornerRadius::ZERO,
                    Stroke::new(1.0, theme::accent()),
                    StrokeKind::Outside,
                );
            }
            if response.on_hover_text(format!("#{color:06X}")).clicked() {
                chosen = Some(*color);
            }
        }
    });
    chosen
}

pub fn tab_button(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let font = if selected {
        theme::tab_font_bold()
    } else {
        theme::tab_font()
    };
    let color = if selected {
        theme::accent()
    } else {
        theme::chrome_text()
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let regular = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::tab_font_bold(), color)
        .size()
        .x;
    let size = vec2(
        regular + 2.0 * theme::RIBBON_TAB_PADDING,
        ui.available_height(),
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hot = response.hovered() && !selected;
    ui.painter().rect_filled(
        rect.shrink2(vec2(2.0, 4.0)),
        CornerRadius::same(4),
        motion::hover_color(ui, &response, hot, theme::tab_hover()),
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}
