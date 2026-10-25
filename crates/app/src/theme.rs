use eframe::egui::{Color32, FontFamily, FontId};

use vellum_grid_core::DEFAULT_FONT;

pub use vellum_ui::theme::*;

vellum_ui::canvas_colors! {
    cell_bg: Color32::WHITE, DARK_CANVAS_BG;
    cell_text: Color32::BLACK, DARK_CANVAS_TEXT;
    grid_line: Color32::from_rgb(0xDA, 0xDA, 0xDA), Color32::from_rgb(0x3A, 0x3A, 0x3A);
    selection_fill: Color32::from_black_alpha(20), Color32::from_white_alpha(30);
    clipboard_marquee: Color32::from_rgb(0x10, 0x7C, 0x41), Color32::from_rgb(0x4C, 0xC3, 0x8A);
    freeze_line: Color32::from_rgb(0x9E, 0x9E, 0x9E), Color32::from_rgb(0x7A, 0x7A, 0x7A);
    chart_selected: Color32::from_rgb(0x80, 0x80, 0x80), Color32::from_rgb(0xA0, 0xA0, 0xA0);
    fill_preview: Color32::from_rgb(0x80, 0x80, 0x80), Color32::from_rgb(0xA0, 0xA0, 0xA0);
}

vellum_ui::colors! {
    surface: Color32::WHITE, DARK_CANVAS_BG;
    tip_bg: Color32::from_rgb(0xFF, 0xFF, 0xE1), Color32::from_rgb(0x2B, 0x2B, 0x2B);
    note_bg: Color32::from_rgb(0xFF, 0xFF, 0xE1), Color32::from_rgb(0x3A, 0x38, 0x26);
    note_border: Color32::from_rgb(0x9E, 0x9E, 0x9E), Color32::from_rgb(0x6A, 0x68, 0x56);
    header_bg: Color32::from_rgb(0xF5, 0xF5, 0xF5), Color32::from_rgb(0x26, 0x26, 0x26);
    header_bg_selected: Color32::from_rgb(0xD3, 0xF0, 0xE0), Color32::from_rgb(0x23, 0x41, 0x2F);
    header_text: Color32::from_rgb(0x44, 0x44, 0x44), Color32::from_rgb(0xC8, 0xC8, 0xC8);
    header_text_selected: Color32::from_rgb(0x10, 0x7C, 0x41), Color32::from_rgb(0x4C, 0xC3, 0x8A);
    header_line: Color32::from_rgb(0xC6, 0xC6, 0xC6), Color32::from_rgb(0x4A, 0x4A, 0x4A);
    preview_bg: Color32::from_rgb(0xF3, 0xF3, 0xF3), Color32::from_rgb(0x1F, 0x1F, 0x1F);
}

pub const REFERENCE: Color32 = Color32::from_rgb(0x41, 0x72, 0xC4);
pub const REFERENCE_COLORS: [Color32; 7] = [
    Color32::from_rgb(0x1F, 0x5F, 0xD1),
    Color32::from_rgb(0xD8, 0x3B, 0x01),
    Color32::from_rgb(0x8E, 0x44, 0xAD),
    Color32::from_rgb(0x10, 0x7C, 0x10),
    Color32::from_rgb(0xC2, 0x39, 0xB3),
    Color32::from_rgb(0x00, 0x83, 0x8F),
    Color32::from_rgb(0xB8, 0x86, 0x0B),
];
pub const REFERENCE_FILL_ALPHA: u8 = 28;
pub const TRACE: Color32 = Color32::from_rgb(0x1F, 0x5F, 0xD1);
pub const TRACE_ERROR: Color32 = Color32::from_rgb(0xD1, 0x1F, 0x1F);
pub const TRACE_WIDTH: f32 = 1.5;
pub const TRACE_HEAD: f32 = 9.0;
pub const TRACE_DOT: f32 = 3.0;
pub const TRACE_ICON: f32 = 16.0;
pub const TRACE_OFFSHEET: f32 = 48.0;
pub const ISSUE_MARK: Color32 = Color32::from_rgb(0x10, 0x7C, 0x10);
pub const ISSUE_MARK_SIZE: f32 = 6.0;
pub const ISSUE_TAG: f32 = 20.0;
pub const ISSUE_GAP: f32 = 2.0;
pub const SPLIT_DIALOG_WIDTH: f32 = 620.0;
pub const PREVIEW_HEIGHT: f32 = 200.0;
pub const PREVIEW_FONT: f32 = 13.0;
pub const PREVIEW_LINE: f32 = 18.0;
pub const PREVIEW_PADDING: f32 = 6.0;
pub const PREVIEW_MIN_CHARS: usize = 40;
pub const PREVIEW_COLUMN: f32 = 60.0;
pub const PREVIEW_GAP: f32 = 16.0;
pub const PREVIEW_ROW_GAP: f32 = 4.0;
pub const VALIDATION_WIDTH: f32 = 440.0;
pub const ALERT_ICON: f32 = 32.0;
pub const LIST_BUTTON: f32 = 16.0;
pub const LIST_MIN_WIDTH: f32 = 120.0;
pub const CIRCLE_PAD: f32 = 4.0;
pub const CIRCLE_WIDTH: f32 = 1.5;
pub const FILTER_OP_WIDTH: f32 = 160.0;
pub const FILTER_VALUE_WIDTH: f32 = 180.0;
pub const CUSTOM_FILTER_WIDTH: f32 = 420.0;
pub const OUTLINE_STEP: f32 = 16.0;
pub const OUTLINE_BUTTON: f32 = 13.0;
pub const OUTLINE_TICK: f32 = 4.0;
pub const NOTE_MARK: Color32 = Color32::from_rgb(0xD1, 0x1F, 0x1F);
pub const COMMENT_MARK: Color32 = Color32::from_rgb(0x7B, 0x3F, 0xD1);
pub const COMMENT_RESOLVED: Color32 = Color32::from_rgb(0xA0, 0xA0, 0xA0);
pub const NOTE_WIDTH: f32 = 200.0;
pub const NOTE_GAP: f32 = 8.0;
pub const COMMENT_WIDTH: f32 = 280.0;
pub const COMMENTS_PANE_WIDTH: f32 = 320.0;
pub const LINK_DIALOG_WIDTH: f32 = 420.0;
pub const ISSUE_ICON: Color32 = Color32::from_rgb(0xC1, 0x7A, 0x00);
pub const REFERENCE_HANDLE: f32 = 5.0;

pub const HEADER_BG_FULL: Color32 = Color32::from_rgb(0x10, 0x7C, 0x41);

pub const COL_HEADER_HEIGHT: f32 = 20.0;
pub const ROW_HEADER_MIN_WIDTH: f32 = 32.0;
pub const ROW_HEADER_DIGIT_WIDTH: f32 = 8.0;
pub const ROW_HEADER_PADDING: f32 = 12.0;
pub const CELL_PADDING: f32 = 3.0;
pub const TEXT_BOTTOM_GAP: f32 = 2.0;
pub const FONT_ASCENT: f32 = 0.75;
pub const FONT_DESCENT: f32 = 0.25;
pub const CAP_HEIGHT: f32 = 0.66;
pub const LINE_FACTOR: f32 = 1.25;
pub const UNDERLINE_OFFSET: f32 = 0.1;
pub const ACCOUNTING_UNDERLINE_OFFSET: f32 = 0.22;
pub const DOUBLE_UNDERLINE_GAP: f32 = 2.0;
pub const SCRIPT_SCALE: f32 = 0.66;
pub const SCRIPT_RISE: f32 = 0.45;
pub const SCRIPT_DROP: f32 = 0.2;
pub const INDENT_STEP: f32 = 9.0;
pub const PATTERN_TILE: f32 = 8.0;
pub const FAUX_BOLD_OFFSET: f32 = 0.6;
pub const RESIZE_GRIP: f32 = 3.0;
pub const EDITOR_SLACK: f32 = 16.0;
pub const SELECTION_BORDER: f32 = 2.0;
pub const FILL_HANDLE: f32 = 6.0;
pub const MOVE_GRIP: f32 = 3.0;
pub const MARQUEE_DASH: f32 = 4.0;
pub const SCROLLBAR_SIZE: f32 = 14.0;
pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;

pub const FONT_COMBO_WIDTH: f32 = 112.0;
pub const SIZE_COMBO_WIDTH: f32 = 46.0;
pub const FORMAT_COMBO_WIDTH: f32 = 104.0;
pub const STYLE_SAMPLE_WIDTH: f32 = 112.0;
pub const STYLE_GALLERY_COLUMNS: usize = 6;
pub const TABLE_THUMB_WIDTH: f32 = 44.0;
pub const TABLE_THUMB_HEIGHT: f32 = 34.0;
pub const TABLE_THUMB_MARGIN: f32 = 3.0;
pub const TABLE_NAME_WIDTH: f32 = 120.0;
pub const RIBBON_COLUMN_GAP: f32 = 12.0;
pub const CF_ICON_SIZE: f32 = 14.0;
pub const CF_SAMPLE: f32 = 40.0;
pub const CF_RULE_WIDTH: f32 = 560.0;
pub const CF_MANAGE_WIDTH: f32 = 680.0;
pub const CF_POINT_KIND_WIDTH: f32 = 200.0;
pub const CF_PREVIEW_WIDTH: f32 = 240.0;
pub const CF_FORMAT_SAMPLE_WIDTH: f32 = 120.0;
pub const CF_FORMAT_SAMPLE_HEIGHT: f32 = 22.0;
pub const CF_RULE_COLUMN: f32 = 260.0;
pub const CF_RANGE_COLUMN: f32 = 150.0;
pub const CF_STOP_COLUMN: f32 = 90.0;
pub const GO_TO_WIDTH: f32 = 340.0;
pub const FORMAT_DIALOG_WIDTH: f32 = 560.0;
pub const FORMAT_DIALOG_HEIGHT: f32 = 440.0;
pub const CATEGORY_LIST_HEIGHT: f32 = 340.0;
pub const TYPE_LIST_HEIGHT: f32 = 150.0;
pub const LIST_INSET: f32 = 10.0;
pub const ALIGN_COLUMN_WIDTH: f32 = 260.0;
pub const ALIGN_COMBO_WIDTH: f32 = 220.0;
pub const MAX_INDENT: u8 = 15;
pub const AUTOFIT_ROW_LIMIT: u32 = 1000;
pub const ORIENTATION_SIZE: f32 = 140.0;
pub const FONT_LIST_WIDTH: f32 = 220.0;
pub const FONT_STYLE_WIDTH: f32 = 130.0;
pub const FONT_SIZE_WIDTH: f32 = 70.0;
pub const FONT_PREVIEW_WIDTH: f32 = 260.0;
pub const FONT_PREVIEW_HEIGHT: f32 = 60.0;
pub const LINE_SAMPLE_WIDTH: f32 = 110.0;
pub const LINE_SAMPLE_HEIGHT: f32 = 16.0;
pub const EDGE_BUTTON: f32 = 28.0;
pub const BORDER_PREVIEW_WIDTH: f32 = 200.0;
pub const BORDER_PREVIEW_HEIGHT: f32 = 132.0;
pub const BORDER_PREVIEW_MARGIN: f32 = 14.0;
pub const FILL_PALETTE_WIDTH: f32 = 200.0;
pub const SPELL_DIALOG_WIDTH: f32 = 440.0;
pub const SPELL_FIELD_WIDTH: f32 = 300.0;
pub const SPELL_BUTTON_OFFSET: f32 = 20.0;
pub const SPELL_BUTTON_WIDTH: f32 = 136.0;
pub const NUMBER_INPUT_WIDTH: f32 = 80.0;
pub const FONT_SIZES: [u16; 16] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72];
pub const MAX_DECIMALS: u8 = 30;
pub const MAX_COLUMN_CHARS: f64 = 255.0;
pub const MAX_ROW_POINTS: f64 = 409.0;
pub const SAMPLE_SERIAL: f64 = 46312.5;
pub const DEFAULT_DIGIT_WIDTH: f32 = 7.0;
pub const CATEGORY_LIST_WIDTH: f32 = 140.0;
pub const RENAME_MIN_WIDTH: f32 = 40.0;

pub const BREAK_LINE: Color32 = Color32::from_rgb(0x2F, 0x5D, 0xC4);
pub const BREAK_LABEL: Color32 = Color32::from_rgba_premultiplied(0x38, 0x38, 0x38, 0x70);
pub const BREAK_OUTSIDE: Color32 = Color32::from_black_alpha(0x30);
pub const BREAK_WIDTH: f32 = 2.0;
pub const BREAK_DASH: f32 = 6.0;
pub const BREAK_LABEL_SIZE: f32 = 40.0;
pub const FILTERED_ROW: Color32 = Color32::from_rgb(0x00, 0x70, 0xC0);
pub const FILTER_BUTTON: f32 = 16.0;
pub const FILTER_MENU_WIDTH: f32 = 230.0;
pub const FILTER_LIST_HEIGHT: f32 = 180.0;
pub const SORT_COMBO_WIDTH: f32 = 150.0;
pub const ABOUT_LOGO: f32 = 56.0;
pub const STATUS_BUTTON: f32 = 22.0;
pub const STATUS_GLYPH: f32 = 14.0;
pub const ZOOM_SLIDER: f32 = 100.0;
pub const ZOOM_STEP: f32 = 0.1;
pub const STATUS_DIGITS: usize = 11;
pub const SETUP_WIDTH: f32 = 520.0;
pub const SETUP_HEIGHT: f32 = 260.0;
pub const HEADER_FIELD_WIDTH: f32 = 150.0;
pub const CURSOR_ARM: f32 = 9.0;
pub const CURSOR_HALF: f32 = 3.0;
pub const CURSOR_FILL: Color32 = Color32::WHITE;
pub const CURSOR_OUTLINE: Color32 = Color32::BLACK;
pub const CURSOR_ARROW: Color32 = Color32::BLACK;
pub const NAV_BUTTON: f32 = 20.0;
pub const NAV_GLYPH: f32 = 14.0;
pub const FUNCTION_DIALOG_WIDTH: f32 = 460.0;
pub const FUNCTION_CATEGORY_WIDTH: f32 = 200.0;
pub const FUNCTION_MENU_HEIGHT: f32 = 420.0;
pub const ASSIST_GAP: f32 = 2.0;
pub const ASSIST_WIDTH: f32 = 220.0;
pub const ASSIST_HEIGHT: f32 = 240.0;
pub const ASSIST_ROW_HEIGHT: f32 = 24.0;
pub const ASSIST_HELP_WIDTH: f32 = 260.0;
pub const NAME_INPUT_WIDTH: f32 = 300.0;
pub const NAME_MANAGER_WIDTH: f32 = 720.0;
pub const NAME_LIST_HEIGHT: f32 = 220.0;
pub const NAME_ROW_HEIGHT: f32 = 26.0;
pub const NAME_CELL_PADDING: f32 = 6.0;
pub const NAME_COLUMN: f32 = 140.0;
pub const NAME_VALUE_COLUMN: f32 = 130.0;
pub const NAME_REFERS_COLUMN: f32 = 190.0;
pub const NAME_SCOPE_COLUMN: f32 = 110.0;
pub const NEW_TILE_WIDTH: f32 = 168.0;
pub const NEW_TILE_HEIGHT: f32 = 150.0;
pub const NEW_TILE_LABEL: f32 = 28.0;
pub const NEW_TILE_INSET: f32 = 16.0;
pub const NEW_TILE_LINES: usize = 5;
pub const PRINT_PANEL_WIDTH: f32 = 320.0;
pub const PRINT_BUTTON: f32 = 88.0;
pub const PICKER_HEIGHT: f32 = 48.0;
pub const PICKER_ICON: f32 = 24.0;
pub const PICKER_ARROW: f32 = 28.0;
pub const PREVIEW_MARGIN: i8 = 16;
pub const PREVIEW_NAV_HEIGHT: f32 = 40.0;
pub const PREVIEW_NAV_WIDTH: f32 = 120.0;
pub const AUTOSAVE_DELAY: f64 = 3.0;
pub const RECOVERY_INTERVAL: f64 = 60.0;
pub const DIALOG_INDENT: f32 = 24.0;
pub const ABOUT_TITLE: f32 = 20.0;
pub const CHART_BG: Color32 = Color32::WHITE;
pub const CHART_BORDER: Color32 = Color32::from_rgb(0xD9, 0xD9, 0xD9);
pub const CHART_TEXT: Color32 = Color32::from_rgb(0x59, 0x59, 0x59);
pub const CHART_GRID: Color32 = Color32::from_rgb(0xD9, 0xD9, 0xD9);
pub const CHART_AXIS: Color32 = Color32::from_rgb(0xBF, 0xBF, 0xBF);
pub const CHART_COLORS: [Color32; 6] = [
    Color32::from_rgb(0x44, 0x72, 0xC4),
    Color32::from_rgb(0xED, 0x7D, 0x31),
    Color32::from_rgb(0xA5, 0xA5, 0xA5),
    Color32::from_rgb(0xFF, 0xC0, 0x00),
    Color32::from_rgb(0x5B, 0x9B, 0xD5),
    Color32::from_rgb(0x70, 0xAD, 0x47),
];
pub const CHART_PADDING: f32 = 10.0;
pub const CHART_TITLE_SIZE: f32 = 18.67;
pub const CHART_LABEL_SIZE: f32 = 12.0;
pub const CHART_LEGEND_HEIGHT: f32 = 22.0;
pub const CHART_SWATCH: f32 = 9.0;
pub const CHART_TICKS: usize = 5;
pub const CHART_AXIS_LABEL: f32 = 18.0;
pub const CHART_GAP: f32 = 2.19;
pub const CHART_LINE_WIDTH: f32 = 3.0;
pub const CHART_MARKER: f32 = 4.0;
pub const DOUGHNUT_HOLE: f32 = 0.5;
pub const SPARK_PADDING: f32 = 3.0;
pub const LINE_HIT: f32 = 5.0;
pub const DRAW_THRESHOLD: f32 = 4.0;
pub const SHAPE_INSET_X: f32 = 9.6;
pub const SHAPE_INSET_Y: f32 = 4.8;
pub const ARROW_HEAD: f32 = 6.0;
pub const PICTURE_FIT: f32 = 0.9;
pub const SHAPE_SWATCH: f32 = 24.0;
pub const SHAPE_SWATCH_PAD: f32 = 4.0;
pub const SHAPE_GALLERY_WIDTH: f32 = 300.0;
pub const SHAPE_SAMPLE_LINE: u32 = 0x404040;
pub const SIZE_FIELD_WIDTH: f32 = 80.0;
pub const OPTIONS_COMBO_WIDTH: f32 = 220.0;
pub const WEIGHT_COMBO_WIDTH: f32 = 72.0;
pub const SPARK_LINE: f32 = 1.25;
pub const SPARK_MARKER: f32 = 2.0;
pub const CHART_GRIP: f32 = 10.0;
pub const CHART_HANDLE: f32 = 7.0;
pub const CHART_MIN_SIZE: f32 = 80.0;
pub const RULE_INPUT_WIDTH: f32 = 90.0;
pub const PRESET_COMBO_WIDTH: f32 = 250.0;
pub const FIND_WIDTH: f32 = 380.0;
pub const FIND_TOP: f32 = 220.0;
pub const FIND_INPUT_WIDTH: f32 = 260.0;

pub const FORMULA_BAR_HEIGHT: f32 = 30.0;
pub const FORMULA_BAR_EXPANDED: f32 = 72.0;
pub const EXPANDED_ROWS: usize = 3;
pub const NAME_BOX_WIDTH: f32 = 96.0;
pub const SHEET_TABS_HEIGHT: f32 = 28.0;
pub const SHEET_TAB_PADDING: f32 = 16.0;
pub const TAB_COLOR_BAR: f32 = 5.0;
pub const TAB_COLOR_TINT: f32 = 0.18;
pub const TAB_COLOR_HOVER: f32 = 0.12;
pub const TEXT_ON_LIGHT: Color32 = Color32::BLACK;
pub const TEXT_ON_DARK: Color32 = Color32::WHITE;

pub fn text_on(fill: Color32) -> Color32 {
    let luma =
        0.299 * f32::from(fill.r()) + 0.587 * f32::from(fill.g()) + 0.114 * f32::from(fill.b());
    if luma > 150.0 {
        TEXT_ON_LIGHT
    } else {
        TEXT_ON_DARK
    }
}
pub const WHEEL_ROWS_PER_NOTCH: f32 = 3.0;
pub const WHEEL_NOTCH: f32 = 50.0;

#[cfg(not(target_arch = "wasm32"))]
pub const WINDOW_WIDTH: f32 = 1280.0;
#[cfg(not(target_arch = "wasm32"))]
pub const WINDOW_HEIGHT: f32 = 800.0;
#[cfg(not(target_arch = "wasm32"))]
pub const WINDOW_MIN_WIDTH: f32 = 480.0;
#[cfg(not(target_arch = "wasm32"))]
pub const WINDOW_MIN_HEIGHT: f32 = 320.0;

pub const CELL_FONT_SIZE: f32 = 14.67;

pub fn cell_font() -> FontId {
    FontId::new(CELL_FONT_SIZE, FontFamily::Name(DEFAULT_FONT.into()))
}

pub fn header_font() -> FontId {
    cell_font()
}

pub fn bar_font() -> FontId {
    cell_font()
}
