use std::collections::BTreeSet;

use eframe::egui::{
    Align, Area, Context, Frame, Id, Key, Layout, Order, Pos2, Rect, ScrollArea, Stroke, TextEdit,
};
use vellum_grid_core::filter::ColumnFilter;
use vellum_grid_core::{
    Cell, CellAddr, CellRange, Color, Extras, Highlight, Highlighter, MAX_ROWS, SortError, SortKey,
    Style, current_region, fold, guess_header,
};

use crate::app::{Selection, Spreadsheet};
use crate::dialogs::{Dialog, accent_button, plain_button};
use crate::filter_ui::{FilterKind, Preset, preset_label};
use crate::icons::Icon;
use crate::{menu, text, theme};

#[derive(Debug)]
pub struct FilterMenu {
    pub col: u32,
    anchor: Pos2,
    search: String,
    values: Vec<(String, bool)>,
    fresh: bool,
    kind: FilterKind,
    fills: Vec<Option<Color>>,
    fonts: Vec<Option<Color>>,
    sub: Option<bool>,
}

enum MenuAction {
    Sort(bool),
    Clear,
    Apply,
    Cancel,
    Preset(Preset),
    Criteria(ColumnFilter),
}

impl Spreadsheet {
    pub fn effective_style(&self, highlighter: &Highlighter, addr: CellAddr, cell: &Cell) -> Style {
        self.effective(highlighter, addr, cell).0
    }

    pub fn effective(
        &self,
        highlighter: &Highlighter,
        addr: CellAddr,
        cell: &Cell,
    ) -> (Style, Extras) {
        let mut style = cell.style().clone();
        if let Some(look) = self.table_look_at(addr) {
            look.apply(&mut style);
        }
        if highlighter.is_empty() {
            return (style, Extras::default());
        }
        let (highlight, extras) = highlighter.evaluate(addr, &cell.value);
        apply_highlight(&mut style, &highlight);
        (style, extras)
    }

    pub fn fill_allowed(&self) -> bool {
        let range = self.selected();
        self.edit.is_none() && !range.is_full_rows() && !range.is_full_cols()
    }

    pub fn finish_fill(&mut self, copy: bool) {
        self.drag = None;
        let Some(target) = self.fill_target.take() else {
            return;
        };
        let source = self.selected();
        self.doc.fill(source, target, copy);
        self.select(Selection {
            active: self.sel.active,
            anchor: target.start(),
            corner: target.end(),
        });
    }

    pub fn fill_to_neighbor(&mut self) {
        let range = self.selected();
        let sheet = self.sheet();
        let filled = |row: u32, col: u32| {
            CellAddr::new(row, col)
                .ok()
                .and_then(|addr| sheet.get(addr))
                .is_some_and(|cell| cell.has_content())
        };
        let below = range.end().row() + 1;
        let neighbor = [
            range.start().col().checked_sub(1),
            Some(range.end().col() + 1),
        ]
        .into_iter()
        .flatten()
        .find(|&col| filled(below, col));
        let Some(col) = neighbor else {
            return;
        };
        let mut last = below;
        while last + 1 < MAX_ROWS && filled(last + 1, col) {
            last += 1;
        }
        let corner = CellAddr::new(last, range.end().col()).expect("inside the sheet");
        let target = range.including(corner);
        self.fill_target = Some(target);
        self.finish_fill(false);
    }

    pub fn filter_header(&self, col: u32) -> CellAddr {
        let row = self
            .sheet()
            .layout()
            .filter
            .as_ref()
            .map_or(0, |filter| filter.range.start().row());
        CellAddr::new(row, col).expect("inside the sheet")
    }

    pub fn open_filter_menu(&mut self, col: u32, button: Rect) {
        let shown = self.sheet().layout().filter.as_ref().and_then(|filter| {
            match filter.criteria.get(&col) {
                Some(ColumnFilter::Values(shown)) => Some(shown.clone()),
                _ => None,
            }
        });
        let values = self
            .doc
            .filter_values(col)
            .into_iter()
            .map(|value| {
                let checked = shown.as_ref().is_none_or(|set| set.contains(&value));
                (value, checked)
            })
            .collect();
        self.filter_menu = Some(FilterMenu {
            col,
            anchor: button.left_bottom(),
            search: String::new(),
            values,
            fresh: true,
            kind: self.column_kind(col),
            fills: self.column_colors(col, true),
            fonts: self.column_colors(col, false),
            sub: None,
        });
    }

    pub fn filter_menu_ui(&mut self, ctx: &Context) {
        let Some(mut menu) = self.filter_menu.take() else {
            return;
        };
        let mut action = None;
        let mut sub_anchor = None;
        let area = Area::new(Id::unique("filter-menu"))
            .order(Order::Foreground)
            .fixed_pos(menu.anchor)
            .show(ctx, |ui| {
                Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_width(theme::FILTER_MENU_WIDTH);
                    ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                        if menu::icon_item(ui, Icon::SortAsc, text::sort_asc()) {
                            action = Some(MenuAction::Sort(false));
                        }
                        if menu::icon_item(ui, Icon::SortDesc, text::sort_desc()) {
                            action = Some(MenuAction::Sort(true));
                        }
                        menu::separator(ui);
                        if menu::icon_item(ui, Icon::ClearFilter, text::clear_column_filter()) {
                            action = Some(MenuAction::Clear);
                        }
                        let colored = menu.fills.len() > 1
                            || menu.fonts.len() > 1
                            || menu.fills.iter().chain(&menu.fonts).any(Option::is_some);
                        if colored {
                            let open = menu.sub == Some(false);
                            let row = menu::arrow_item(ui, text::filter_by_color(), open);
                            if row.clicked() {
                                menu.sub = (!open).then_some(false);
                            }
                            if open {
                                sub_anchor = Some(row.rect.right_top());
                            }
                        }
                        let open = menu.sub == Some(true);
                        let row = menu::arrow_item(ui, menu.kind.label(), open);
                        if row.clicked() {
                            menu.sub = (!open).then_some(true);
                        }
                        if open {
                            sub_anchor = Some(row.rect.right_top());
                        }
                        menu::separator(ui);
                    });
                    ui.add(
                        TextEdit::singleline(&mut menu.search)
                            .hint_text(text::search())
                            .background_color(theme::input_bg()),
                    );
                    let needle = fold(&menu.search);
                    let matching: Vec<usize> = (0..menu.values.len())
                        .filter(|&i| fold(&menu.values[i].0).contains(&needle))
                        .collect();
                    Frame::NONE
                        .stroke(Stroke::new(1.0, theme::chrome_line()))
                        .fill(theme::input_bg())
                        .show(ui, |ui| {
                            ScrollArea::vertical()
                                .max_height(theme::FILTER_LIST_HEIGHT)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    let mut all = matching.iter().all(|&i| menu.values[i].1);
                                    if ui.checkbox(&mut all, text::select_all_items()).changed() {
                                        for &i in &matching {
                                            menu.values[i].1 = all;
                                        }
                                    }
                                    for &i in &matching {
                                        let (value, checked) = &mut menu.values[i];
                                        let label = if value.is_empty() {
                                            text::blanks().to_owned()
                                        } else {
                                            value.clone()
                                        };
                                        ui.checkbox(checked, label);
                                    }
                                });
                        });
                    ui.horizontal(|ui| {
                        let any = menu.values.iter().any(|(_, checked)| *checked);
                        if ui.add_enabled(any, accent_button(text::ok())).clicked() {
                            action = Some(MenuAction::Apply);
                        }
                        if ui.add(plain_button(text::cancel())).clicked() {
                            action = Some(MenuAction::Cancel);
                        }
                    });
                });
            });
        let mut sub_rect = None;
        if let (Some(anchor), Some(types)) = (sub_anchor, menu.sub) {
            let sub = Area::new(Id::unique("filter-submenu"))
                .order(Order::Foreground)
                .fixed_pos(anchor)
                .show(ctx, |ui| {
                    Frame::menu(ui.style()).show(ui, |ui| {
                        ui.set_min_width(theme::MENU_WIDTH);
                        ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                            if types {
                                for preset in menu.kind.presets() {
                                    if menu::item(ui, &preset_label(*preset, menu.kind)) {
                                        action = Some(MenuAction::Preset(*preset));
                                    }
                                }
                                return;
                            }
                            if menu.fills.len() > 1 || menu.fills.iter().any(Option::is_some) {
                                menu::soon(ui, None, text::filter_by_fill());
                                for color in &menu.fills {
                                    if menu::swatch_item(ui, color.map(|c| c.0)) {
                                        action =
                                            Some(MenuAction::Criteria(ColumnFilter::Fill(*color)));
                                    }
                                }
                            }
                            if menu.fonts.len() > 1 || menu.fonts.iter().any(Option::is_some) {
                                menu::soon(ui, None, text::filter_by_font());
                                for color in &menu.fonts {
                                    if menu::swatch_item(ui, color.map(|c| c.0)) {
                                        action = Some(MenuAction::Criteria(
                                            ColumnFilter::FontColor(*color),
                                        ));
                                    }
                                }
                            }
                        });
                    });
                });
            sub_rect = Some(sub.response.rect);
        }
        let outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer.interact_pos().is_some_and(|p| {
                    !area.response.rect.contains(p) && !sub_rect.is_some_and(|r| r.contains(p))
                })
        });
        if (outside && !menu.fresh) || ctx.input(|i| i.key_pressed(Key::Escape)) {
            action = action.or(Some(MenuAction::Cancel));
        }
        menu.fresh = false;
        match action {
            None => self.filter_menu = Some(menu),
            Some(MenuAction::Cancel) => {}
            Some(MenuAction::Clear) => {
                self.doc.set_filter_column(menu.col, None);
            }
            Some(MenuAction::Preset(preset)) => self.apply_preset(menu.col, menu.kind, preset),
            Some(MenuAction::Criteria(criteria)) => {
                self.doc.set_filter_column(menu.col, Some(criteria));
            }
            Some(MenuAction::Sort(descending)) => {
                if let Some(filter) = self.sheet().layout().filter.clone() {
                    let key = SortKey {
                        col: menu.col,
                        descending,
                    };
                    self.apply_sort(filter.range, &[key], true);
                }
            }
            Some(MenuAction::Apply) => {
                let needle = fold(&menu.search);
                let shown: BTreeSet<String> = menu
                    .values
                    .iter()
                    .filter(|(value, checked)| *checked && fold(value).contains(&needle))
                    .map(|(value, _)| value.clone())
                    .collect();
                let everything = needle.is_empty() && shown.len() == menu.values.len();
                self.doc.set_filter_column(
                    menu.col,
                    (!everything).then_some(ColumnFilter::Values(shown)),
                );
            }
        }
    }

    pub fn sort_target(&self) -> (CellRange, bool) {
        let selected = self.selected();
        let range = if selected.is_single() {
            current_region(self.sheet(), self.sel.active)
        } else {
            self.sheet().trim_to_extent(selected)
        };
        (range, guess_header(self.sheet(), range))
    }

    pub fn quick_sort(&mut self, descending: bool) {
        let (range, header) = match &self.sheet().layout().filter {
            Some(filter) if filter.range.contains(self.sel.active) => (filter.range, true),
            _ => self.sort_target(),
        };
        let col = self
            .sel
            .active
            .col()
            .clamp(range.start().col(), range.end().col());
        self.apply_sort(range, &[SortKey { col, descending }], header);
    }

    pub fn apply_sort(&mut self, range: CellRange, keys: &[SortKey], header: bool) {
        if let Err(SortError::MergedCells) = self.doc.sort(range, keys, header) {
            self.dialog = Some(Dialog::Error {
                title: text::app_name(),
                message: text::sort_merged().to_owned(),
            });
        }
    }

    pub fn open_sort_dialog(&mut self) {
        let (range, header) = self.sort_target();
        let col = self
            .sel
            .active
            .col()
            .clamp(range.start().col(), range.end().col());
        self.dialog = Some(Dialog::Sort {
            range,
            header,
            levels: vec![SortKey {
                col,
                descending: false,
            }],
        });
    }

    pub fn toggle_filter(&mut self) {
        let range = match self.sheet().layout().filter {
            Some(_) => self.selected(),
            None => self.sort_target().0,
        };
        self.filter_menu = None;
        self.doc.toggle_filter(range);
    }

    pub fn freeze(&mut self, rows: u32, cols: u32) {
        self.doc.set_layout(|layout| {
            layout.freeze_rows = rows;
            layout.freeze_cols = cols;
        });
        self.view.top = self.view.top.max(rows);
        self.view.left = self.view.left.max(cols);
    }
}

pub fn apply_highlight(style: &mut Style, highlight: &Highlight) {
    style.fill = highlight.fill.or(style.fill);
    style.color = highlight.color.or(style.color);
    style.bold |= highlight.bold;
    style.italic |= highlight.italic;
    style.strike |= highlight.strike;
    if let Some(border) = highlight.border {
        let borders = &mut style.borders;
        borders.top = Some(border);
        borders.bottom = Some(border);
        borders.left = Some(border);
        borders.right = Some(border);
    }
}
