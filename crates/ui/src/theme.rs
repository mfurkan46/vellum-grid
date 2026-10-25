use std::sync::RwLock;

use eframe::egui::{self, Color32, Context, FontFamily, FontId};

use crate::fonts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accent {
    pub base: Color32,
    pub bright: Color32,
    pub pressed: Color32,
    pub hover: Color32,
    pub checked: (Color32, Color32),
    pub checked_line: (Color32, Color32),
}

const fn rgb(value: u32) -> Color32 {
    Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

const fn palette(values: [u32; 8]) -> Accent {
    Accent {
        base: rgb(values[0]),
        bright: rgb(values[1]),
        pressed: rgb(values[2]),
        hover: rgb(values[3]),
        checked: (rgb(values[4]), rgb(values[5])),
        checked_line: (rgb(values[6]), rgb(values[7])),
    }
}

pub const GRID: Accent = palette([
    0x107C41, 0x4CC38A, 0x0A5A2E, 0x158C4B, 0xC9E4D3, 0x1E4A33, 0x8CC4A2, 0x3D8A5E,
]);
pub const WORD: Accent = palette([
    0x185ABD, 0x6CA0F2, 0x124A9C, 0x2A6BD0, 0xC5D7F2, 0x1E3555, 0x8CB0E6, 0x3D63A0,
]);
pub const TEXT: Accent = palette([
    0x0E7C86, 0x5CC8CF, 0x0A5E66, 0x178D98, 0xC3E3E6, 0x17393D, 0x86C6CC, 0x2E7A82,
]);
pub const PDF: Accent = palette([
    0xC4314B, 0xF07A8E, 0x9E2238, 0xD3445D, 0xF2CDD3, 0x4A2228, 0xE09AA6, 0x9A3A4B,
]);
pub const STUDIO: Accent = palette([
    0x2B2F36, 0xC9CDD6, 0x1B1E23, 0x3A3F48, 0xD3D5D8, 0x3A3D44, 0xA3A7AE, 0x6A707A,
]);

static ACCENT: RwLock<Accent> = RwLock::new(GRID);

pub fn set_accent(accent: Accent) {
    if let Ok(mut current) = ACCENT.write() {
        *current = accent;
    }
}

fn current() -> Accent {
    ACCENT.read().map_or(GRID, |accent| *accent)
}

fn pick(pair: (Color32, Color32)) -> Color32 {
    if crate::dark() { pair.1 } else { pair.0 }
}

pub fn accent() -> Color32 {
    let accent = current();
    pick((accent.base, accent.bright))
}

pub fn accent_fill() -> Color32 {
    current().base
}

pub fn backstage_nav() -> Color32 {
    current().base
}

pub fn backstage_nav_selected() -> Color32 {
    current().pressed
}

pub fn backstage_nav_hover() -> Color32 {
    current().hover
}

pub fn tool_checked() -> Color32 {
    pick(current().checked)
}

pub fn tool_checked_line() -> Color32 {
    pick(current().checked_line)
}

pub const ON_ACCENT: Color32 = Color32::WHITE;
pub const PAPER: Color32 = Color32::WHITE;
pub const DARK_CANVAS_BG: Color32 = rgb(0x1E1E1E);
pub const DARK_CANVAS_TEXT: Color32 = rgb(0xE6E6E6);

pub fn on_canvas(color: Color32) -> Color32 {
    if crate::dark_canvas() {
        invert_lightness(color)
    } else {
        color
    }
}

fn invert_lightness(color: Color32) -> Color32 {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let channels = [r, g, b].map(|c| f32::from(c) / 255.0);
    let max = channels.iter().copied().fold(0.0, f32::max);
    let min = channels.iter().copied().fold(1.0, f32::min);
    let lightness = (max + min) / 2.0;
    let low = f32::from(DARK_CANVAS_BG.r()) / 255.0;
    let high = f32::from(DARK_CANVAS_TEXT.r()) / 255.0;
    let inverted = low + (high - low) * (1.0 - lightness);
    let room = |l: f32| 1.0 - (2.0 * l - 1.0).abs();
    let scale = if room(lightness) > f32::EPSILON {
        room(inverted) / room(lightness)
    } else {
        0.0
    };
    let [r, g, b] = channels
        .map(|c| ((inverted + (c - lightness) * scale).clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
pub const RIBBON_TABS_HEIGHT: f32 = 34.0;
pub const RIBBON_HEIGHT: f32 = 110.0;
pub const RIBBON_CONTENT_HEIGHT: f32 = 72.0;
pub const RIBBON_RADIUS: u8 = 8;
pub const RIBBON_SHADOW_BLUR: u8 = 6;
pub const RIBBON_PADDING_X: i8 = 8;
pub const RIBBON_PADDING_Y: i8 = 2;
pub const RIBBON_MARGIN: i8 = 8;
pub const COLLAPSE_SIZE: f32 = 18.0;
pub const LAUNCHER_SIZE: f32 = 14.0;
pub const TAB_FONT_SIZE: f32 = 14.0;
pub const RIBBON_LABEL_HEIGHT: f32 = 18.0;
pub const RIBBON_GROUP_GAP: f32 = 8.0;
pub const RIBBON_TAB_PADDING: f32 = 11.0;
pub const RIBBON_TAB_MARKER: f32 = 3.0;
pub const RIBBON_ROW_GAP: f32 = 4.0;
pub const TOOL_SIZE: f32 = 24.0;
pub const SMALL_TOOL_HEIGHT: f32 = 24.0;
pub const BIG_TOOL_WIDTH: f32 = 52.0;
pub const ICON_SIZE: f32 = 20.0;
pub const BIG_ICON_SIZE: f32 = 32.0;
pub const BIG_LABEL_MAX: f32 = 72.0;
pub const DROP_WIDTH: f32 = 11.0;
pub const CHEVRON_SIZE: f32 = 3.0;
pub const SWATCH_SIZE: f32 = 16.0;
pub const SWATCH_GAP: f32 = 2.0;
pub const MENU_WIDTH: f32 = 240.0;
pub const DIALOG_WIDTH: f32 = 420.0;
pub const SMALL_DIALOG_WIDTH: f32 = 240.0;
pub const COLOR_BUTTON_WIDTH: f32 = 90.0;
pub const SMALL_ICON_LIMIT: f32 = 22.0;
pub const LETTER_ICON_SCALE: f32 = 0.8;
pub const COLOR_BAR_SCALE: f32 = 0.18;
pub const CONTEXT_MENU_WIDTH: f32 = 260.0;
pub const MENU_ITEM_HEIGHT: f32 = 28.0;
pub const MENU_ICON_COLUMN: f32 = 36.0;
pub const MENU_ICON: f32 = 18.0;
pub const MENU_ITEM_PADDING: f32 = 16.0;
pub const MENU_SHORTCUT_GAP: f32 = 32.0;
pub const MENU_ARROW: f32 = 12.0;
pub const MENU_RADIUS: u8 = 4;
pub const MENU_DETAIL_HEIGHT: f32 = 44.0;
pub const WIDE_MENU_WIDTH: f32 = 320.0;
pub const MENU_SECTION_GAP: f32 = 4.0;
pub const PASTE_OPTION_SIZE: f32 = 32.0;
pub const PASTE_OPTION_ICON: f32 = 24.0;
pub const MENU_MARGIN: i8 = 6;
pub const POPUP_RADIUS: u8 = 8;
pub const MENU_FONT_SIZE: f32 = 13.0;
pub const DIALOG_FONT_SIZE: f32 = 13.5;
pub const DIALOG_TITLE_SIZE: f32 = 16.0;
pub const DIALOG_MARGIN: i8 = 20;
pub const WINDOW_MARGIN: i8 = 14;
pub const SWITCH_GAP: f32 = 6.0;
pub const DIALOG_BUTTON_HEIGHT: f32 = 32.0;
pub const DIALOG_BUTTON_MIN_WIDTH: f32 = 88.0;
pub const DIALOG_BUTTON_PADDING: f32 = 16.0;
pub const DIALOG_SPACING: f32 = 10.0;
pub const INPUT_HEIGHT: f32 = 28.0;
pub const SEPARATOR_SPACING: f32 = 8.0;
pub const POPUP_SHADOW_BLUR: u8 = 16;
pub const SWITCH_KNOB_GROW: f32 = 1.0;
pub const SWITCH_ANIMATION: f32 = 0.15;
pub const HOVER_ANIMATION: f32 = 0.12;
pub const TAB_MARKER_ANIMATION: f32 = 0.2;
pub const BACKSTAGE_ANIMATION: f32 = 0.28;
pub const BACKSTAGE_SLIDE: f32 = 48.0;
pub const RIBBON_SWITCH_ANIMATION: f32 = 0.2;
pub const RIBBON_SWITCH_SLIDE: f32 = 12.0;
pub const TITLE_FONT_SIZE: f32 = 14.0;
pub const LIST_HEIGHT: f32 = 160.0;
pub const TITLE_BAR_HEIGHT: f32 = 38.0;
pub const BACKSTAGE_NAV_LINE: Color32 = Color32::from_rgba_premultiplied(0x40, 0x40, 0x40, 0x40);
pub const BACKSTAGE_NAV_WIDTH: f32 = 200.0;
pub const BACKSTAGE_NAV_ITEM: f32 = 40.0;
pub const BACKSTAGE_NAV_ICON_COLUMN: f32 = 48.0;
pub const BACKSTAGE_NAV_TOP: f32 = 8.0;
pub const BACKSTAGE_PADDING_X: i8 = 40;
pub const BACKSTAGE_PADDING_Y: i8 = 28;
pub const BACKSTAGE_TITLE: f32 = 28.0;
pub const BACKSTAGE_HEADING: f32 = 16.0;
pub const BACKSTAGE_GAP: f32 = 20.0;
pub const BACKSTAGE_FONT_SIZE: f32 = 14.0;
pub const RECENT_ROW_HEIGHT: f32 = 48.0;
pub const RECENT_MAX_WIDTH: f32 = 860.0;
pub const RECENT_ICON_COLUMN: f32 = 52.0;
pub const WIDE_BUTTON_WIDTH: f32 = 300.0;
pub const WIDE_BUTTON_HEIGHT: f32 = 56.0;
pub const WIDE_BUTTON_PADDING: f32 = 24.0;
pub const TITLE_LOGO: f32 = 18.0;
pub const SWITCH_WIDTH: f32 = 34.0;
pub const SWITCH_HEIGHT: f32 = 18.0;
pub const CAPTION_WIDTH: f32 = 46.0;
pub const CAPTION_GLYPH: f32 = 14.0;
pub const CLOSE_HOVER: Color32 = Color32::from_rgb(0xE8, 0x11, 0x23);
pub const RESIZE_EDGE: f32 = 5.0;
pub const STATUS_BAR_HEIGHT: f32 = 24.0;
pub const CHROME_SPACING: f32 = 8.0;
pub const BUTTON_RADIUS: f32 = 3.0;
pub const CHROME_FONT_SIZE: f32 = 12.0;
pub const SMALL_FONT_SIZE: f32 = 11.5;

crate::colors! {
    tint_clipboard: Color32::from_rgb(0xF2, 0xD9, 0xA6), Color32::from_rgb(0x6B, 0x5A, 0x33);
    tint_blue: Color32::from_rgb(0xBF, 0xD9, 0xF5), Color32::from_rgb(0x2E, 0x4B, 0x6B);
    tint_green: Color32::from_rgb(0xBD, 0xE5, 0xC8), Color32::from_rgb(0x2D, 0x5A, 0x3B);
    tint_yellow: Color32::from_rgb(0xFA, 0xDD, 0x80), Color32::from_rgb(0x7A, 0x64, 0x20);
    tint_pink: Color32::from_rgb(0xF6, 0xBF, 0xCA), Color32::from_rgb(0x6B, 0x36, 0x40);
    tint_purple: Color32::from_rgb(0xD6, 0xC6, 0xF0), Color32::from_rgb(0x4B, 0x3D, 0x6B);
    tint_orange: Color32::from_rgb(0xFB, 0xD5, 0xA5), Color32::from_rgb(0x7A, 0x53, 0x26);
    hover_fill: Color32::from_black_alpha(10), Color32::from_white_alpha(14);
    chrome_bg: Color32::from_rgb(0xF3, 0xF3, 0xF3), Color32::from_rgb(0x1F, 0x1F, 0x1F);
    tab_hover: Color32::from_rgb(0xE6, 0xE6, 0xE6), Color32::from_rgb(0x33, 0x33, 0x33);
    chrome_line: Color32::from_rgb(0xD1, 0xD1, 0xD1), Color32::from_rgb(0x45, 0x45, 0x45);
    chrome_text: Color32::from_rgb(0x24, 0x24, 0x24), Color32::from_rgb(0xE8, 0xE8, 0xE8);
    chrome_text_muted: Color32::from_rgb(0x61, 0x61, 0x61), Color32::from_rgb(0xAD, 0xAD, 0xAD);
    input_bg: Color32::WHITE, Color32::from_rgb(0x26, 0x26, 0x26);
    scrollbar_track: Color32::from_rgb(0xF0, 0xF0, 0xF0), Color32::from_rgb(0x26, 0x26, 0x26);
    scrollbar_thumb: Color32::from_rgb(0xC2, 0xC2, 0xC2), Color32::from_rgb(0x5A, 0x5A, 0x5A);
    scrollbar_thumb_active: Color32::from_rgb(0x9E, 0x9E, 0x9E), Color32::from_rgb(0x7A, 0x7A, 0x7A);
    ribbon_shadow: Color32::from_black_alpha(18), Color32::from_black_alpha(90);
    ribbon_bg: Color32::from_rgb(0xFB, 0xFB, 0xFB), Color32::from_rgb(0x2B, 0x2B, 0x2B);
    ribbon_line: Color32::from_rgb(0xE1, 0xE1, 0xE1), Color32::from_rgb(0x3B, 0x3B, 0x3B);
    tool_hover: Color32::from_rgb(0xE5, 0xE5, 0xE5), Color32::from_rgb(0x3A, 0x3A, 0x3A);
    icon: Color32::from_rgb(0x42, 0x42, 0x42), Color32::from_rgb(0xD6, 0xD6, 0xD6);
    icon_disabled: Color32::from_rgb(0xB4, 0xB4, 0xB4), Color32::from_rgb(0x5C, 0x5C, 0x5C);
    chrome_text_disabled: Color32::from_rgb(0xA6, 0xA6, 0xA6), Color32::from_rgb(0x6E, 0x6E, 0x6E);
    icon_remove: Color32::from_rgb(0xD1, 0x34, 0x38), Color32::from_rgb(0xF1, 0x70, 0x7B);
    swatch_border: Color32::from_rgb(0xC8, 0xC8, 0xC8), Color32::from_rgb(0x5A, 0x5A, 0x5A);
    popup_shadow: Color32::from_black_alpha(40), Color32::from_black_alpha(115);
    list_selected: Color32::from_rgb(0xCC, 0xE4, 0xF7), Color32::from_rgb(0x26, 0x4F, 0x78);
    error_text: Color32::from_rgb(0xC4, 0x2B, 0x1C), Color32::from_rgb(0xFF, 0x99, 0xA4);
}

pub fn tab_font() -> FontId {
    FontId::new(TAB_FONT_SIZE, FontFamily::Name(fonts::CHROME.into()))
}

pub fn tab_font_bold() -> FontId {
    FontId::new(TAB_FONT_SIZE, FontFamily::Name(fonts::CHROME_BOLD.into()))
}

pub fn small_font() -> FontId {
    FontId::new(SMALL_FONT_SIZE, FontFamily::Name(fonts::CHROME.into()))
}

pub fn chrome_font() -> FontId {
    FontId::new(CHROME_FONT_SIZE, FontFamily::Name(fonts::CHROME.into()))
}

pub fn menu_font() -> FontId {
    FontId::new(MENU_FONT_SIZE, FontFamily::Name(fonts::CHROME.into()))
}

pub fn backstage_font() -> FontId {
    FontId::new(BACKSTAGE_FONT_SIZE, FontFamily::Name(fonts::CHROME.into()))
}

pub fn title_font() -> FontId {
    FontId::new(TITLE_FONT_SIZE, FontFamily::Name(fonts::CHROME_BOLD.into()))
}

pub fn install(ctx: &Context) {
    fonts::install(ctx);
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
}

pub fn apply(ctx: &Context) {
    let theme = if crate::dark() {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(theme);
    ctx.style_mut_of(theme, |style| {
        style.visuals = match theme {
            egui::Theme::Dark => egui::Visuals::dark(),
            egui::Theme::Light => egui::Visuals::light(),
        };
        for (text_style, font) in style.text_styles.iter_mut() {
            if *text_style != egui::TextStyle::Monospace {
                font.family = egui::FontFamily::Name(fonts::CHROME.into());
            }
        }
        style.interaction.selectable_labels = false;
        style.visuals.panel_fill = chrome_bg();
        style.visuals.window_fill = ribbon_bg();
        style.visuals.extreme_bg_color = input_bg();
        style.visuals.faint_bg_color = chrome_bg();
        style.visuals.override_text_color = Some(chrome_text());
        style.visuals.selection.bg_fill = tool_checked();
        style.visuals.selection.stroke = egui::Stroke::new(1.0, accent());
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, accent());
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, chrome_line());
        style.spacing.menu_margin = egui::Margin::same(MENU_MARGIN);
        style.spacing.window_margin = egui::Margin::same(WINDOW_MARGIN);
        style.visuals.menu_corner_radius = egui::CornerRadius::same(POPUP_RADIUS);
        style.visuals.window_corner_radius = egui::CornerRadius::same(POPUP_RADIUS);
        style.visuals.window_stroke = egui::Stroke::new(1.0, ribbon_line());
        let shadow = egui::Shadow {
            offset: [0, 4],
            blur: POPUP_SHADOW_BLUR,
            spread: 0,
            color: popup_shadow(),
        };
        style.visuals.popup_shadow = shadow;
        style.visuals.window_shadow = shadow;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverted_lightness_swaps_black_and_white_and_keeps_hue() {
        assert_eq!(invert_lightness(Color32::WHITE), DARK_CANVAS_BG);
        assert_eq!(invert_lightness(Color32::BLACK), DARK_CANVAS_TEXT);
        let red = invert_lightness(Color32::from_rgb(0xFF, 0, 0));
        assert!(red.r() > 0xF0 && red.g() < 0x10 && red.b() < 0x10);
        let pale = invert_lightness(Color32::from_rgb(0xFF, 0xFF, 0xCC));
        assert!(pale.r() < 0x80 && pale.b() < pale.r());
    }
}
