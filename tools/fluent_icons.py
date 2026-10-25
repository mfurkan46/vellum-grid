"""Subset Fluent UI System Icons fonts to the icons Vellum Grid uses.

Usage:
    python tools/fluent_icons.py <fluentui-system-icons/fonts directory>

The directory must contain FluentSystemIcons-{Regular,Filled}.{ttf,json}
from https://github.com/microsoft/fluentui-system-icons (MIT).
Writes assets/fonts/fluent-{regular,filled}.ttf and crates/ui/src/fluent.rs.
"""

import json
import pathlib
import sys

from fontTools import subset
from fontTools.ttLib import TTFont

ICONS = {
    "ADD": "add",
    "ALIGN_BOTTOM": "align_bottom",
    "ALIGN_MIDDLE": "align_center_vertical",
    "ALIGN_TOP": "align_top",
    "ARROW_EXPORT": "arrow_export",
    "ARROW_FORWARD": "arrow_forward",
    "ARROW_IMPORT": "arrow_import",
    "ARROW_LEFT": "arrow_left",
    "ARROW_REDO": "arrow_redo",
    "ARROW_SYNC": "arrow_sync",
    "ARROW_UNDO": "arrow_undo",
    "AUTOSUM": "autosum",
    "BOOK": "book",
    "BORDER_ALL": "border_all",
    "BORDER_BOTTOM": "border_bottom",
    "BORDER_BOTTOM_DOUBLE": "border_bottom_double",
    "BORDER_BOTTOM_THICK": "border_bottom_thick",
    "BORDER_LEFT": "border_left",
    "BORDER_NONE": "border_none",
    "BORDER_OUTSIDE": "border_outside",
    "BORDER_OUTSIDE_THICK": "border_outside_thick",
    "BORDER_RIGHT": "border_right",
    "BORDER_TOP": "border_top",
    "BORDER_TOP_BOTTOM": "border_top_bottom",
    "CALCULATOR": "calculator",
    "CALENDAR": "calendar",
    "CHECKMARK": "checkmark",
    "CHEVRON_DOWN": "chevron_down",
    "CHEVRON_LEFT": "chevron_left",
    "CHEVRON_RIGHT": "chevron_right",
    "CHEVRON_UP": "chevron_up",
    "CLIPBOARD_BRUSH": "clipboard_brush",
    "CLIPBOARD_FORMULA": "clipboard_math_formula",
    "CLIPBOARD_NUMBER": "clipboard_number_123",
    "CLIPBOARD_PASTE": "clipboard_paste",
    "CLOCK": "clock",
    "COLOR": "color",
    "COMMENT": "comment",
    "COMMENT_ADD": "comment_add",
    "COPY": "copy",
    "COPY_SELECT": "copy_select",
    "CUT": "cut",
    "DATA_BAR_HORIZONTAL": "data_bar_horizontal",
    "DATA_BAR_VERTICAL": "data_bar_vertical",
    "DATA_HISTOGRAM": "data_histogram",
    "DATA_LINE": "data_line",
    "DATA_PIE": "data_pie",
    "DATA_SCATTER": "data_scatter",
    "DATA_TRENDING": "data_trending",
    "DECIMAL_LEFT": "decimal_arrow_left",
    "DECIMAL_RIGHT": "decimal_arrow_right",
    "DELETE": "delete",
    "DISMISS": "dismiss",
    "DOCUMENT": "document",
    "DOCUMENT_ADD": "document_add",
    "DOCUMENT_COPY": "document_copy",
    "DOCUMENT_HEADER_FOOTER": "document_header_footer",
    "DOCUMENT_MARGINS": "document_margins",
    "DOCUMENT_ONE_PAGE": "document_one_page",
    "DOCUMENT_PAGE_BREAK": "document_page_break",
    "DOCUMENT_PDF": "document_pdf",
    "DOCUMENT_PRINT": "document_print",
    "DOCUMENT_SYNC": "document_sync",
    "DOCUMENT_TABLE": "document_table",
    "ERASER": "eraser",
    "ERROR_CIRCLE": "error_circle",
    "EYE": "eye",
    "EYE_OFF": "eye_off",
    "FILTER": "filter",
    "FILTER_DISMISS": "filter_dismiss",
    "FILTER_SYNC": "filter_sync",
    "FLASH": "flash",
    "FOLDER": "folder",
    "FOLDER_OPEN": "folder_open",
    "FONT_DECREASE": "font_decrease",
    "FONT_INCREASE": "font_increase",
    "GLOBE": "globe",
    "GRID": "grid",
    "GROUP": "group",
    "HISTORY": "history",
    "HOME": "home",
    "IMAGE": "image",
    "INFO": "info",
    "KEYBOARD": "keyboard",
    "LINK": "link",
    "LOCK_CLOSED": "lock_closed",
    "MATH_FORMULA": "math_formula",
    "MONEY": "money",
    "NOTE": "note",
    "NOTE_ADD": "note_add",
    "NUMBER_SYMBOL": "number_symbol",
    "OPEN": "open",
    "ORIENTATION": "orientation",
    "PAGE_FIT": "page_fit",
    "PAINT_BRUSH": "paint_brush",
    "PAINT_BUCKET": "paint_bucket",
    "PRINT": "print",
    "PRINT_ADD": "print_add",
    "QUESTION_CIRCLE": "question_circle",
    "RENAME": "rename",
    "RULER": "ruler",
    "SAVE": "save",
    "SAVE_EDIT": "save_edit",
    "SEARCH": "search",
    "SETTINGS": "settings",
    "SHAPES": "shapes",
    "SHIELD": "shield",
    "SORT": "arrow_sort",
    "SORT_ASCENDING": "text_sort_ascending",
    "SORT_DESCENDING": "text_sort_descending",
    "SPLIT_VERTICAL": "split_vertical",
    "SQUARE": "square",
    "SQUARE_MULTIPLE": "square_multiple",
    "SUBTRACT": "subtract",
    "SYMBOLS": "symbols",
    "TABLE": "table",
    "TABLE_ADD": "table_add",
    "TABLE_CELLS_MERGE": "table_cells_merge",
    "TABLE_CELL_EDIT": "table_cell_edit",
    "TABLE_DELETE_COLUMN": "table_delete_column",
    "TABLE_DELETE_ROW": "table_delete_row",
    "TABLE_EDIT": "table_edit",
    "TABLE_FREEZE": "table_freeze_column_and_row",
    "TABLE_FREEZE_COLUMN": "table_freeze_column",
    "TABLE_FREEZE_ROW": "table_freeze_row",
    "TABLE_INSERT_COLUMN": "table_insert_column",
    "TABLE_INSERT_ROW": "table_insert_row",
    "TABLE_LIGHTNING": "table_lightning",
    "TABLE_RESIZE_COLUMN": "table_resize_column",
    "TABLE_RESIZE_ROW": "table_resize_row",
    "TABLE_SETTINGS": "table_settings",
    "TABLE_SIMPLE": "table_simple",
    "TABLE_SWITCH": "table_switch",
    "TAB_ADD": "tab_add",
    "TAG": "tag",
    "TARGET_ARROW": "target_arrow",
    "TEXTBOX": "textbox",
    "TEXT_ALIGN_CENTER": "text_align_center",
    "TEXT_ALIGN_LEFT": "text_align_left",
    "TEXT_ALIGN_RIGHT": "text_align_right",
    "TEXT_COLOR": "text_color",
    "TEXT_COLUMN_TWO": "text_column_two",
    "TEXT_DIRECTION": "text_direction_rotate_90_left",
    "TEXT_INDENT_DECREASE": "text_indent_decrease",
    "TEXT_INDENT_INCREASE": "text_indent_increase",
    "TEXT_PROOFING": "text_proofing_tools",
    "TEXT_STRIKETHROUGH": "text_strikethrough",
    "TEXT_WRAP": "text_wrap",
    "WARNING": "warning",
    "WINDOW_NEW": "window_new",
    "ZOOM_FIT": "zoom_fit",
    "ZOOM_IN": "zoom_in",
    "ZOOM_OUT": "zoom_out",
}

SMALL = (20, 24, 16)
LARGE = (24, 28, 32, 20)
STYLES = ("Regular", "Filled")


def codepoint(names: dict, base: str, sizes: tuple, style: str) -> int:
    for size in sizes:
        key = f"ic_fluent_{base}_{size}_{style.lower()}"
        if key in names:
            return names[key]
    raise SystemExit(f"missing {base} ({style})")


def main() -> None:
    source = pathlib.Path(sys.argv[1])
    root = pathlib.Path(__file__).resolve().parent.parent
    fonts = root / "assets" / "fonts"
    fonts.mkdir(parents=True, exist_ok=True)
    table = {}
    for style in STYLES:
        names = json.loads((source / f"FluentSystemIcons-{style}.json").read_text(encoding="utf-8"))
        used = []
        for name, base in ICONS.items():
            small = codepoint(names, base, SMALL, style)
            large = codepoint(names, base, LARGE, style)
            table.setdefault(name, {})[style] = (small, large)
            used += [small, large]
        options = subset.Options()
        options.layout_features = []
        options.notdef_outline = True
        font = TTFont(source / f"FluentSystemIcons-{style}.ttf")
        subsetter = subset.Subsetter(options)
        subsetter.populate(unicodes=sorted(set(used)))
        subsetter.subset(font)
        font.save(fonts / f"fluent-{style.lower()}.ttf")
    lines = [
        "// Generated by tools/fluent_icons.py from Fluent UI System Icons (MIT).",
        "#![allow(dead_code)]",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct Glyph {",
        "    pub small: char,",
        "    pub large: char,",
        "    pub small_fill: char,",
        "    pub large_fill: char,",
        "}",
        "",
    ]
    for name in ICONS:
        (small, large), (small_fill, large_fill) = table[name]["Regular"], table[name]["Filled"]
        lines += [
            f"pub const {name}: Glyph = Glyph {{",
            f"    small: '\\u{{{small:x}}}',",
            f"    large: '\\u{{{large:x}}}',",
            f"    small_fill: '\\u{{{small_fill:x}}}',",
            f"    large_fill: '\\u{{{large_fill:x}}}',",
            "};",
        ]
    (root / "crates" / "ui" / "src" / "fluent.rs").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
