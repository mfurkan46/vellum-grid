use eframe::egui::{
    self, ComboBox, CornerRadius, DragValue, FontFamily, FontId, Frame, Margin, Popup, Rect,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, pos2, vec2,
};
use vellum_grid_core::{
    Border, Borders, Cell, CellAddr, CellRange, CellValue, Color, HAlign, LineStyle, Pattern,
    PatternKind, STACKED, Script, Style, Underline, VAlign,
};

use crate::app::{Spreadsheet, locale};
use crate::format::{
    Category, DATE_CODES, FRACTION_CODES, NumberFormat, SPECIAL_CODES, TIME_CODES,
};
use crate::ribbon::widgets::palette_ui;
use crate::{fonts, icons, render, text, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatTab {
    Number,
    Alignment,
    Font,
    Border,
    Fill,
    Protection,
}

impl FormatTab {
    const ALL: [Self; 6] = [
        Self::Number,
        Self::Alignment,
        Self::Font,
        Self::Border,
        Self::Fill,
        Self::Protection,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Number => text::format_number(),
            Self::Alignment => text::alignment(),
            Self::Font => text::font(),
            Self::Border => text::border(),
            Self::Fill => text::fill_tab(),
            Self::Protection => text::protection(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    InnerH,
    Bottom,
    DiagUp,
    Left,
    InnerV,
    Right,
    DiagDown,
}

impl Edge {
    const ALL: [Self; 8] = [
        Self::Top,
        Self::InnerH,
        Self::Bottom,
        Self::DiagUp,
        Self::Left,
        Self::InnerV,
        Self::Right,
        Self::DiagDown,
    ];
}

#[derive(Debug, Clone)]
pub struct FormatForm {
    pub tab: FormatTab,
    pub number: NumberFormat,
    value: CellValue,
    original: Style,
    pub style: Style,
    edges: [Option<Option<Border>>; 8],
    line: Option<LineStyle>,
    line_color: Color,
    pattern_color: Color,
    rows: bool,
    cols: bool,
    merged: bool,
    original_merged: bool,
}

impl FormatForm {
    fn shown(&self, edge: Edge) -> Option<Border> {
        if let Some(choice) = self.edges[edge as usize] {
            return choice;
        }
        let b = &self.original.borders;
        match edge {
            Edge::Top => b.top,
            Edge::Bottom => b.bottom,
            Edge::Left => b.left,
            Edge::Right => b.right,
            Edge::DiagUp => b.diagonal.filter(|_| b.diagonal_up),
            Edge::DiagDown => b.diagonal.filter(|_| b.diagonal_down),
            Edge::InnerH | Edge::InnerV => None,
        }
    }

    fn current_line(&self) -> Option<Border> {
        self.line.map(|style| Border {
            style,
            color: self.line_color,
        })
    }

    fn toggle(&mut self, edge: Edge) {
        let next = match (self.shown(edge), self.current_line()) {
            (Some(shown), Some(line)) if shown == line => None,
            (_, line) => line,
        };
        self.edges[edge as usize] = Some(next);
    }

    fn preset(&mut self, edges: &[Edge], border: Option<Border>) {
        for edge in edges {
            self.edges[*edge as usize] = Some(border);
        }
    }

    fn apply_borders(&self, range: CellRange, addr: CellAddr, borders: &mut Borders) {
        let pick = |outer: Edge, inner: Edge, at_edge: bool| {
            self.edges[if at_edge { outer } else { inner } as usize]
        };
        let sides = [
            (
                pick(Edge::Top, Edge::InnerH, addr.row() == range.start().row()),
                &mut borders.top,
            ),
            (
                pick(Edge::Bottom, Edge::InnerH, addr.row() == range.end().row()),
                &mut borders.bottom,
            ),
            (
                pick(Edge::Left, Edge::InnerV, addr.col() == range.start().col()),
                &mut borders.left,
            ),
            (
                pick(Edge::Right, Edge::InnerV, addr.col() == range.end().col()),
                &mut borders.right,
            ),
        ];
        for (choice, side) in sides {
            if let Some(choice) = choice {
                *side = choice;
            }
        }
        let up = self.edges[Edge::DiagUp as usize];
        let down = self.edges[Edge::DiagDown as usize];
        if up.is_some() || down.is_some() {
            let up = up.unwrap_or(self.shown(Edge::DiagUp));
            let down = down.unwrap_or(self.shown(Edge::DiagDown));
            borders.diagonal = up.or(down);
            borders.diagonal_up = up.is_some();
            borders.diagonal_down = down.is_some();
        }
    }
}

macro_rules! copy_changed {
    ($from:expr, $to:expr, $target:expr, $($field:ident),+) => {
        $(if $from.$field != $to.$field {
            $target.$field = $to.$field.clone();
        })+
    };
}

fn merge_changes(original: &Style, edited: &Style, target: &mut Style) {
    copy_changed!(
        original, edited, target, font, size, bold, italic, underline, strike, script, color, fill,
        pattern, h_align, v_align, wrap, shrink, indent, rotation, locked, hidden
    );
}

impl Spreadsheet {
    pub fn open_format_cells(&mut self) {
        self.open_format_tab(FormatTab::Number);
    }

    pub fn open_format_tab(&mut self, tab: FormatTab) {
        let style = self.active_style();
        let range = self.selected();
        let layout = self.sheet().layout();
        let merged = layout.merge_at(self.sel.active).is_some();
        let pattern_color = style.pattern.map_or(Color::BLACK, |p| p.color);
        let form = FormatForm {
            tab,
            number: NumberFormat::new(&style.number_format),
            value: self.sheet().value(self.sel.active).clone(),
            original: style.clone(),
            style,
            edges: [None; 8],
            line: Some(LineStyle::Thin),
            line_color: Color::BLACK,
            pattern_color,
            rows: range.rows() > 1 && !merged,
            cols: range.cols() > 1 && !merged,
            merged,
            original_merged: merged,
        };
        self.dialog = Some(crate::dialogs::Dialog::FormatCells(Box::new(form)));
    }

    pub fn apply_format_form(&mut self, form: &FormatForm) {
        let code = form.number.code();
        let number_changed = code != form.original.number_format;
        let ranges = self.ranges();
        let range = self.selected();
        self.doc.batch(|doc| {
            doc.style_ranges(&ranges, |addr, style| {
                merge_changes(&form.original, &form.style, style);
                if number_changed {
                    style.number_format = code.clone();
                }
                let within = ranges
                    .iter()
                    .copied()
                    .find(|r| r.contains(addr))
                    .unwrap_or(range);
                form.apply_borders(within, addr, &mut style.borders);
            });
            if form.merged != form.original_merged {
                if form.merged {
                    doc.merge(range, false);
                } else {
                    doc.unmerge(range);
                }
            }
        });
        let (old, new) = (&form.original, &form.style);
        let taller = old.rotation != new.rotation
            || old.wrap != new.wrap
            || old.size != new.size
            || old.font != new.font;
        if taller {
            self.queue_autofit(range);
        }
    }
}

pub fn dialog_ui(ui: &mut Ui, form: &mut FormatForm) {
    ui.set_width(theme::FORMAT_DIALOG_WIDTH);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for tab in FormatTab::ALL {
            let label = RichText::new(tab.label());
            if ui.selectable_label(form.tab == tab, label).clicked() {
                form.tab = tab;
            }
        }
    });
    ui.separator();
    ui.allocate_ui(
        vec2(theme::FORMAT_DIALOG_WIDTH, theme::FORMAT_DIALOG_HEIGHT),
        |ui| {
            ui.set_min_height(theme::FORMAT_DIALOG_HEIGHT);
            match form.tab {
                FormatTab::Number => number_tab(ui, form),
                FormatTab::Alignment => alignment_tab(ui, form),
                FormatTab::Font => font_tab(ui, form),
                FormatTab::Border => border_tab(ui, form),
                FormatTab::Fill => fill_tab(ui, form),
                FormatTab::Protection => protection_tab(ui, form),
            }
        },
    );
}

fn boxed() -> Frame {
    Frame::NONE
        .stroke(Stroke::new(1.0, theme::chrome_line()))
        .fill(theme::input_bg())
        .inner_margin(Margin::same(4))
}

fn heading(ui: &mut Ui, label: &str) {
    ui.label(RichText::new(label).family(FontFamily::Name(fonts::CHROME_BOLD.into())));
}

fn list<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: &str,
    width: f32,
    height: f32,
    items: &[(T, String)],
    selected: &mut T,
) {
    boxed().show(ui, |ui| {
        ui.set_width(width);
        ScrollArea::vertical()
            .id_salt(id)
            .max_height(height)
            .min_scrolled_height(height)
            .show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 0.0;
                for (value, label) in items {
                    let item = ui.selectable_label(*selected == *value, label.as_str());
                    if item.clicked() {
                        *selected = *value;
                    }
                }
            });
    });
}

fn number_tab(ui: &mut Ui, form: &mut FormatForm) {
    let format = &mut form.number;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(text::category());
            let mut category = format.category;
            let items: Vec<(Category, String)> = Category::ALL
                .iter()
                .map(|c| (*c, c.label().to_owned()))
                .collect();
            list(
                ui,
                "format-category",
                theme::CATEGORY_LIST_WIDTH,
                theme::CATEGORY_LIST_HEIGHT,
                &items,
                &mut category,
            );
            if category != format.category {
                format.select(category);
            }
        });
        ui.vertical(|ui| {
            ui.label(text::sample());
            let code = format.code();
            let sample = match &form.value {
                CellValue::Empty => String::new(),
                value => locale().render(value, &code).text,
            };
            boxed().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(sample).font(theme::bar_font()));
            });
            if format.category.has_decimals() {
                ui.horizontal(|ui| {
                    ui.label(text::decimal_places());
                    ui.add(DragValue::new(&mut format.decimals).range(0..=theme::MAX_DECIMALS));
                });
            }
            let serial = match form.value {
                CellValue::Number(n) => n,
                _ => theme::SAMPLE_SERIAL,
            };
            let example = |code: &str| locale().render(&CellValue::Number(serial), code).text;
            match format.category {
                Category::Number => {
                    ui.checkbox(&mut format.separator, text::use_separator());
                }
                Category::Currency | Category::Accounting => {
                    ui.horizontal(|ui| {
                        ui.label(text::symbol());
                        let shown = if format.symbol {
                            "₺"
                        } else {
                            text::no_symbol()
                        };
                        ComboBox::from_id_salt("currency-symbol")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut format.symbol, true, "₺");
                                ui.selectable_value(&mut format.symbol, false, text::no_symbol());
                            });
                    });
                }
                Category::Date | Category::Time => {
                    ui.label(text::type_text());
                    let (codes, chosen): (&[&str], &mut usize) =
                        if format.category == Category::Date {
                            (&DATE_CODES, &mut format.date)
                        } else {
                            (&TIME_CODES, &mut format.time)
                        };
                    let items: Vec<(usize, String)> = codes
                        .iter()
                        .enumerate()
                        .map(|(i, code)| (i, example(code)))
                        .collect();
                    let width = ui.available_width() - theme::LIST_INSET;
                    list(
                        ui,
                        "format-type",
                        width,
                        theme::TYPE_LIST_HEIGHT,
                        &items,
                        chosen,
                    );
                }
                Category::Fraction | Category::Special => {
                    ui.label(text::type_text());
                    let (codes, chosen): (&[(&str, text::Label)], &mut usize) =
                        if format.category == Category::Fraction {
                            (&FRACTION_CODES, &mut format.fraction)
                        } else {
                            (&SPECIAL_CODES, &mut format.special)
                        };
                    let items: Vec<(usize, String)> = codes
                        .iter()
                        .enumerate()
                        .map(|(i, (_, label))| (i, label().to_owned()))
                        .collect();
                    let width = ui.available_width() - theme::LIST_INSET;
                    list(
                        ui,
                        "format-type",
                        width,
                        theme::TYPE_LIST_HEIGHT,
                        &items,
                        chosen,
                    );
                }
                Category::Custom => {
                    ui.label(text::type_text());
                    ui.add(
                        TextEdit::singleline(&mut format.custom)
                            .desired_width(f32::INFINITY)
                            .background_color(theme::input_bg()),
                    );
                }
                _ => {}
            }
        });
    });
}

fn h_align_label(align: HAlign) -> &'static str {
    match align {
        HAlign::General => text::align_general(),
        HAlign::Left => text::align_left_indent(),
        HAlign::Center => text::align_middle_item(),
        HAlign::Right => text::align_right_indent(),
        HAlign::Fill => text::align_fill(),
        HAlign::Justify => text::align_justify(),
        HAlign::CenterAcross => text::align_center_across(),
        HAlign::Distributed => text::align_distributed_indent(),
    }
}

fn v_align_label(align: VAlign) -> &'static str {
    match align {
        VAlign::Top => text::align_top_item(),
        VAlign::Center => text::align_middle_item(),
        VAlign::Bottom => text::align_bottom_item(),
        VAlign::Justify => text::align_justify(),
        VAlign::Distributed => text::align_distributed(),
    }
}

fn alignment_tab(ui: &mut Ui, form: &mut FormatForm) {
    let style = &mut form.style;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(theme::ALIGN_COLUMN_WIDTH);
            heading(ui, text::text_alignment());
            ui.label(text::horizontal());
            ComboBox::from_id_salt("h-align")
                .width(theme::ALIGN_COMBO_WIDTH)
                .selected_text(h_align_label(style.h_align))
                .show_ui(ui, |ui| {
                    for align in HAlign::ALL {
                        ui.selectable_value(&mut style.h_align, align, h_align_label(align));
                    }
                });
            ui.label(text::vertical());
            ComboBox::from_id_salt("v-align")
                .width(theme::ALIGN_COMBO_WIDTH)
                .selected_text(v_align_label(style.v_align))
                .show_ui(ui, |ui| {
                    for align in VAlign::ALL {
                        ui.selectable_value(&mut style.v_align, align, v_align_label(align));
                    }
                });
            ui.horizontal(|ui| {
                ui.label(text::indent());
                ui.add_enabled(
                    style.h_align.indents(),
                    DragValue::new(&mut style.indent).range(0..=theme::MAX_INDENT),
                );
            });
            heading(ui, text::text_control());
            if ui
                .checkbox(&mut style.wrap, text::wrap_text_check())
                .changed()
                && style.wrap
            {
                style.shrink = false;
            }
            if ui
                .checkbox(&mut style.shrink, text::shrink_to_fit())
                .changed()
                && style.shrink
            {
                style.wrap = false;
            }
            ui.checkbox(&mut form.merged, text::merge_cells_check());
        });
        ui.vertical(|ui| {
            heading(ui, text::orientation());
            let mut stacked = style.is_stacked();
            if ui.checkbox(&mut stacked, text::vertical_text()).changed() {
                style.rotation = if stacked { STACKED } else { 0 };
            }
            orientation_dial(ui, style);
            ui.horizontal(|ui| {
                let mut degrees = style.angle();
                let changed = ui
                    .add_enabled(
                        !style.is_stacked(),
                        DragValue::new(&mut degrees).range(-90..=90),
                    )
                    .changed();
                if changed {
                    style.rotation = degrees;
                }
                ui.label(text::degrees());
            });
        });
    });
}

fn orientation_dial(ui: &mut Ui, style: &mut Style) {
    let size = vec2(theme::ORIENTATION_SIZE, theme::ORIENTATION_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, theme::input_bg());
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
    let center = pos2(rect.left() + theme::CELL_PADDING * 3.0, rect.center().y);
    let radius = rect.height() / 2.0 - theme::CELL_PADDING * 2.0;
    for step in -6..=6 {
        let angle = (step as f32 * 15.0).to_radians();
        let dot = center + vec2(angle.cos(), -angle.sin()) * radius;
        painter.circle_filled(dot, 1.5, theme::chrome_text_muted());
    }
    if !style.is_stacked() {
        let angle = f32::from(style.angle()).to_radians();
        let tip = center + vec2(angle.cos(), -angle.sin()) * radius;
        painter.line_segment([center, tip], Stroke::new(1.5, theme::accent()));
        painter.circle_filled(tip, 3.0, theme::accent());
    }
    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let delta = pointer - center;
        let degrees = (-delta.y).atan2(delta.x.max(0.0)).to_degrees().round() as i16;
        style.rotation = degrees.clamp(-90, 90);
    }
}

pub fn color_button(
    ui: &mut Ui,
    id: &str,
    color: Option<Color>,
    none: &str,
) -> Option<Option<Color>> {
    let size = vec2(theme::COLOR_BUTTON_WIDTH, theme::INPUT_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, theme::input_bg());
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
    let swatch = rect.shrink(theme::CELL_PADDING);
    icons::swatch(painter, swatch, color.map(render::color));
    let mut choice = None;
    Popup::menu(&response).id(egui::Id::unique(id)).show(|ui| {
        choice = palette_ui(ui, none);
    });
    choice
}

fn font_tab(ui: &mut Ui, form: &mut FormatForm) {
    let style = &mut form.style;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(text::font_name());
            ui.add(
                TextEdit::singleline(&mut style.font)
                    .desired_width(theme::FONT_LIST_WIDTH)
                    .background_color(theme::input_bg()),
            );
            let names: Vec<(usize, String)> = fonts::available()
                .into_iter()
                .enumerate()
                .map(|(i, name)| (i, name.to_owned()))
                .collect();
            let mut chosen = names
                .iter()
                .position(|(_, name)| *name == style.font)
                .unwrap_or(usize::MAX);
            let before = chosen;
            list(
                ui,
                "font-names",
                theme::FONT_LIST_WIDTH,
                theme::TYPE_LIST_HEIGHT,
                &names,
                &mut chosen,
            );
            if chosen != before
                && let Some((_, name)) = names.get(chosen)
            {
                style.font = name.clone();
            }
        });
        ui.vertical(|ui| {
            ui.label(text::font_style());
            let styles = [
                ((false, false), text::font_regular().to_owned()),
                ((false, true), text::italic().to_owned()),
                ((true, false), text::bold().to_owned()),
                ((true, true), text::bold_italic().to_owned()),
            ];
            let mut chosen = (style.bold, style.italic);
            list(
                ui,
                "font-styles",
                theme::FONT_STYLE_WIDTH,
                theme::TYPE_LIST_HEIGHT,
                &styles,
                &mut chosen,
            );
            (style.bold, style.italic) = chosen;
        });
        ui.vertical(|ui| {
            ui.label(text::size_label());
            let sizes: Vec<(u16, String)> = theme::FONT_SIZES
                .iter()
                .map(|p| (p * 10, p.to_string()))
                .collect();
            list(
                ui,
                "font-sizes",
                theme::FONT_SIZE_WIDTH,
                theme::TYPE_LIST_HEIGHT,
                &sizes,
                &mut style.size,
            );
        });
    });
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(text::underline_kind());
            ComboBox::from_id_salt("underline-kind")
                .width(theme::ALIGN_COMBO_WIDTH)
                .selected_text(underline_label(style.underline))
                .show_ui(ui, |ui| {
                    for kind in Underline::ALL {
                        ui.selectable_value(&mut style.underline, kind, underline_label(kind));
                    }
                });
            heading(ui, text::effects());
            ui.checkbox(&mut style.strike, text::strikethrough());
            let mut superscript = style.script == Script::Super;
            if ui.checkbox(&mut superscript, text::superscript()).changed() {
                style.script = if superscript {
                    Script::Super
                } else {
                    Script::None
                };
            }
            let mut subscript = style.script == Script::Sub;
            if ui.checkbox(&mut subscript, text::subscript()).changed() {
                style.script = if subscript { Script::Sub } else { Script::None };
            }
        });
        ui.vertical(|ui| {
            ui.label(text::color());
            if let Some(color) = color_button(ui, "font-color", style.color, text::automatic()) {
                style.color = color;
            }
            ui.label(text::preview());
            preview(ui, style, text::font_sample());
        });
    });
}

fn underline_label(kind: Underline) -> &'static str {
    match kind {
        Underline::None => text::underline_none(),
        Underline::Single => text::underline_single(),
        Underline::Double => text::underline_double(),
        Underline::SingleAccounting => text::underline_single_accounting(),
        Underline::DoubleAccounting => text::underline_double_accounting(),
    }
}

fn preview(ui: &mut Ui, style: &Style, sample: &str) {
    let size = vec2(theme::FONT_PREVIEW_WIDTH, theme::FONT_PREVIEW_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        CornerRadius::ZERO,
        style.fill.map_or(theme::cell_bg(), render::cell_color),
    );
    if let Some(pattern) = style.pattern {
        render::paint_pattern(&painter, rect, pattern);
    }
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
    if sample.is_empty() {
        return;
    }
    let shown = Style {
        h_align: HAlign::Center,
        v_align: VAlign::Center,
        rotation: 0,
        wrap: false,
        shrink: false,
        indent: 0,
        ..style.clone()
    };
    let cell = Cell::from(CellValue::Text(sample.to_owned()));
    if let Some(visual) = render::visual(&painter, &cell, &shown, rect.width(), false, 1.0) {
        let pos = render::position(&visual, rect);
        render::paint_text(&painter.with_clip_rect(rect.shrink(1.0)), &visual, pos);
    }
}

fn line_sample(ui: &mut Ui, line: Option<LineStyle>, selected: bool) -> egui::Response {
    let size = vec2(theme::LINE_SAMPLE_WIDTH, theme::LINE_SAMPLE_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, theme::accent()),
            StrokeKind::Inside,
        );
    }
    let a = pos2((rect.left() + 6.0).round(), rect.center().y.round() + 0.5);
    let b = pos2((rect.right() - 6.0).round(), a.y);
    match line {
        Some(style) => render::paint_line(
            painter,
            a,
            b,
            vec2(0.0, 1.0),
            Border {
                style,
                color: Color::BLACK,
            },
        ),
        None => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text::none(),
                FontId::proportional(theme::DIALOG_FONT_SIZE),
                theme::chrome_text(),
            );
        }
    }
    response
}

fn edge_button(ui: &mut Ui, edge: Edge, on: bool) -> egui::Response {
    let size = vec2(theme::EDGE_BUTTON, theme::EDGE_BUTTON);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let fill = if on {
        theme::selection_fill()
    } else if response.hovered() {
        theme::hover_fill()
    } else {
        theme::input_bg()
    };
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(5.0);
    let faint = Stroke::new(1.0, theme::chrome_text_muted().gamma_multiply(0.4));
    painter.rect_stroke(inner, CornerRadius::ZERO, faint, StrokeKind::Middle);
    let strong = Stroke::new(2.0, theme::chrome_text());
    let c = inner.center();
    let segment = match edge {
        Edge::Top => [inner.left_top(), inner.right_top()],
        Edge::Bottom => [inner.left_bottom(), inner.right_bottom()],
        Edge::Left => [inner.left_top(), inner.left_bottom()],
        Edge::Right => [inner.right_top(), inner.right_bottom()],
        Edge::InnerH => [pos2(inner.left(), c.y), pos2(inner.right(), c.y)],
        Edge::InnerV => [pos2(c.x, inner.top()), pos2(c.x, inner.bottom())],
        Edge::DiagUp => [inner.left_bottom(), inner.right_top()],
        Edge::DiagDown => [inner.left_top(), inner.right_bottom()],
    };
    painter.line_segment(segment, strong);
    response
}

fn border_preview(ui: &mut Ui, form: &FormatForm) -> Rect {
    let size = vec2(theme::BORDER_PREVIEW_WIDTH, theme::BORDER_PREVIEW_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect.expand(2.0));
    painter.rect_filled(rect, CornerRadius::ZERO, theme::cell_bg());
    let area = rect.shrink(theme::BORDER_PREVIEW_MARGIN);
    let corner = Stroke::new(1.0, theme::chrome_line());
    for p in [
        area.left_top(),
        area.right_top(),
        area.left_bottom(),
        area.right_bottom(),
    ] {
        painter.line_segment([p - vec2(6.0, 0.0), p + vec2(6.0, 0.0)], corner);
        painter.line_segment([p - vec2(0.0, 6.0), p + vec2(0.0, 6.0)], corner);
    }
    let font = FontId::proportional(theme::DIALOG_FONT_SIZE);
    let cols = if form.cols { 2 } else { 1 };
    let rows = if form.rows { 2 } else { 1 };
    let cell = vec2(area.width() / cols as f32, area.height() / rows as f32);
    for r in 0..rows {
        for c in 0..cols {
            let center = area.min + vec2((c as f32 + 0.5) * cell.x, (r as f32 + 0.5) * cell.y);
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                text::border_sample(),
                font.clone(),
                theme::chrome_text_muted(),
            );
        }
    }
    let mid = area.center();
    let lines = [
        (Edge::Top, area.left_top(), area.right_top(), vec2(0.0, 1.0)),
        (
            Edge::Bottom,
            area.left_bottom(),
            area.right_bottom(),
            vec2(0.0, -1.0),
        ),
        (
            Edge::Left,
            area.left_top(),
            area.left_bottom(),
            vec2(1.0, 0.0),
        ),
        (
            Edge::Right,
            area.right_top(),
            area.right_bottom(),
            vec2(-1.0, 0.0),
        ),
        (
            Edge::InnerH,
            pos2(area.left(), mid.y),
            pos2(area.right(), mid.y),
            vec2(0.0, 1.0),
        ),
        (
            Edge::InnerV,
            pos2(mid.x, area.top()),
            pos2(mid.x, area.bottom()),
            vec2(1.0, 0.0),
        ),
    ];
    for (edge, a, b, inward) in lines {
        let shown = (edge != Edge::InnerH || form.rows) && (edge != Edge::InnerV || form.cols);
        if let (true, Some(border)) = (shown, form.shown(edge)) {
            render::paint_line(&painter, a, b, inward, border);
        }
    }
    for edge in [Edge::DiagUp, Edge::DiagDown] {
        if let Some(border) = form.shown(edge) {
            for r in 0..rows {
                for c in 0..cols {
                    let min = area.min + vec2(c as f32 * cell.x, r as f32 * cell.y);
                    let box_ = Rect::from_min_size(min, cell);
                    let (a, b) = if edge == Edge::DiagUp {
                        (box_.left_bottom(), box_.right_top())
                    } else {
                        (box_.left_top(), box_.right_bottom())
                    };
                    let normal = (b - a).normalized().rot90();
                    render::paint_line(&painter, a, b, normal, border);
                }
            }
        }
    }
    rect
}

fn border_tab(ui: &mut Ui, form: &mut FormatForm) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            heading(ui, text::line());
            ui.label(text::line_style());
            boxed().show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if line_sample(ui, None, form.line.is_none()).clicked() {
                    form.line = None;
                }
                for style in LineStyle::ALL {
                    if line_sample(ui, Some(style), form.line == Some(style)).clicked() {
                        form.line = Some(style);
                    }
                }
            });
            ui.label(text::color());
            if let Some(color) =
                color_button(ui, "border-color", Some(form.line_color), text::automatic())
            {
                form.line_color = color.unwrap_or(Color::BLACK);
            }
        });
        ui.vertical(|ui| {
            heading(ui, text::border_presets());
            ui.horizontal(|ui| {
                let line = form.current_line();
                if ui.button(text::none()).clicked() {
                    form.preset(&Edge::ALL, None);
                }
                if ui.button(text::border_outline()).clicked() {
                    form.preset(&[Edge::Top, Edge::Bottom, Edge::Left, Edge::Right], line);
                }
                let inside = form.rows || form.cols;
                if ui
                    .add_enabled(inside, egui::Button::new(text::inside()))
                    .clicked()
                {
                    let mut edges = Vec::new();
                    if form.rows {
                        edges.push(Edge::InnerH);
                    }
                    if form.cols {
                        edges.push(Edge::InnerV);
                    }
                    form.preset(&edges, line);
                }
            });
            heading(ui, text::border());
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    for edge in [Edge::Top, Edge::InnerH, Edge::Bottom, Edge::DiagUp] {
                        let enabled = edge != Edge::InnerH || form.rows;
                        let on = form.shown(edge).is_some();
                        if ui
                            .add_enabled_ui(enabled, |ui| edge_button(ui, edge, on))
                            .inner
                            .clicked()
                        {
                            form.toggle(edge);
                        }
                    }
                });
                ui.vertical(|ui| {
                    border_preview(ui, form);
                    ui.horizontal(|ui| {
                        for edge in [Edge::Left, Edge::InnerV, Edge::Right, Edge::DiagDown] {
                            let enabled = edge != Edge::InnerV || form.cols;
                            let on = form.shown(edge).is_some();
                            if ui
                                .add_enabled_ui(enabled, |ui| edge_button(ui, edge, on))
                                .inner
                                .clicked()
                            {
                                form.toggle(edge);
                            }
                        }
                    });
                });
            });
            ui.label(RichText::new(text::border_hint()).color(theme::chrome_text_muted()));
        });
    });
}

fn pattern_label(kind: Option<PatternKind>) -> &'static str {
    let Some(kind) = kind else {
        return text::pattern_solid();
    };
    match kind {
        PatternKind::Gray75 => text::pattern_gray_75(),
        PatternKind::Gray50 => text::pattern_gray_50(),
        PatternKind::Gray25 => text::pattern_gray_25(),
        PatternKind::Gray125 => text::pattern_gray_125(),
        PatternKind::Gray0625 => text::pattern_gray_0625(),
        PatternKind::DarkHorizontal => text::pattern_dark_horizontal(),
        PatternKind::DarkVertical => text::pattern_dark_vertical(),
        PatternKind::DarkDown => text::pattern_dark_down(),
        PatternKind::DarkUp => text::pattern_dark_up(),
        PatternKind::DarkGrid => text::pattern_dark_grid(),
        PatternKind::DarkTrellis => text::pattern_dark_trellis(),
        PatternKind::LightHorizontal => text::pattern_light_horizontal(),
        PatternKind::LightVertical => text::pattern_light_vertical(),
        PatternKind::LightDown => text::pattern_light_down(),
        PatternKind::LightUp => text::pattern_light_up(),
        PatternKind::LightGrid => text::pattern_light_grid(),
        PatternKind::LightTrellis => text::pattern_light_trellis(),
    }
}

fn fill_tab(ui: &mut Ui, form: &mut FormatForm) {
    let style = &mut form.style;
    let pattern_color = &mut form.pattern_color;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(text::background_color());
            ui.set_width(theme::FILL_PALETTE_WIDTH);
            ui.spacing_mut().item_spacing.y = theme::SWATCH_GAP * 2.0;
            if let Some(choice) = palette_ui(ui, text::no_color()) {
                style.fill = choice;
            }
        });
        ui.vertical(|ui| {
            ui.label(text::pattern_color());
            if let Some(color) =
                color_button(ui, "pattern-color", Some(*pattern_color), text::automatic())
            {
                *pattern_color = color.unwrap_or(Color::BLACK);
                if let Some(pattern) = style.pattern.as_mut() {
                    pattern.color = *pattern_color;
                }
            }
            ui.label(text::pattern_style());
            let current = style.pattern.map(|p| p.kind);
            let color = *pattern_color;
            ComboBox::from_id_salt("pattern-style")
                .width(theme::ALIGN_COMBO_WIDTH)
                .selected_text(pattern_label(current))
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(current.is_none(), pattern_label(None))
                        .clicked()
                    {
                        style.pattern = None;
                    }
                    for kind in PatternKind::ALL {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(
                                vec2(theme::SWATCH_SIZE * 2.0, theme::SWATCH_SIZE),
                                Sense::hover(),
                            );
                            render::paint_pattern(ui.painter(), rect, Pattern { kind, color });
                            if ui
                                .selectable_label(current == Some(kind), pattern_label(Some(kind)))
                                .clicked()
                            {
                                style.pattern = Some(Pattern { kind, color });
                            }
                        });
                    }
                });
            ui.label(text::sample());
            preview(ui, style, "");
        });
    });
}

fn protection_tab(ui: &mut Ui, form: &mut FormatForm) {
    ui.checkbox(&mut form.style.locked, text::locked());
    ui.checkbox(&mut form.style.hidden, text::hidden_formula());
    ui.add_space(theme::DIALOG_SPACING);
    ui.label(text::protection_hint());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> FormatForm {
        FormatForm {
            tab: FormatTab::Border,
            number: NumberFormat::new("General"),
            value: CellValue::Empty,
            original: Style::default(),
            style: Style::default(),
            edges: [None; 8],
            line: Some(LineStyle::Medium),
            line_color: Color(0xFF0000),
            pattern_color: Color::BLACK,
            rows: true,
            cols: true,
            merged: false,
            original_merged: false,
        }
    }

    #[test]
    fn border_edges_follow_cell_position() {
        let mut form = form();
        form.preset(
            &[Edge::Top, Edge::Bottom, Edge::Left, Edge::Right],
            form.current_line(),
        );
        form.preset(&[Edge::InnerH], None);
        let range: CellRange = "B2:C3".parse().unwrap();
        let mut corner = Borders::default();
        form.apply_borders(range, "B2".parse().unwrap(), &mut corner);
        assert_eq!(corner.top.unwrap().style, LineStyle::Medium);
        assert_eq!(corner.left.unwrap().color, Color(0xFF0000));
        assert!(corner.bottom.is_none() && corner.right.is_none());
        let mut inner = Borders {
            right: Some(Border::thin()),
            ..Borders::default()
        };
        form.apply_borders(range, "B2".parse().unwrap(), &mut inner);
        assert!(inner.right.is_some());
    }

    #[test]
    fn toggling_an_edge_clears_the_same_line() {
        let mut form = form();
        form.toggle(Edge::DiagDown);
        assert!(form.shown(Edge::DiagDown).is_some());
        form.toggle(Edge::DiagDown);
        assert!(form.shown(Edge::DiagDown).is_none());
        form.toggle(Edge::DiagUp);
        let mut borders = Borders::default();
        form.apply_borders("A1".parse().unwrap(), "A1".parse().unwrap(), &mut borders);
        assert!(borders.diagonal_up && !borders.diagonal_down);
    }

    #[test]
    fn only_changed_fields_are_copied() {
        let original = Style::default();
        let edited = Style {
            bold: true,
            indent: 2,
            ..Style::default()
        };
        let mut target = Style {
            italic: true,
            ..Style::default()
        };
        merge_changes(&original, &edited, &mut target);
        assert!(target.bold && target.italic);
        assert_eq!(target.indent, 2);
    }
}
