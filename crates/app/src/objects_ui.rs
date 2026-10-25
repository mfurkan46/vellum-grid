use eframe::egui::{ComboBox, CornerRadius, DragValue, Grid, Popup, RichText, Sense, Ui, vec2};
use vellum_grid_core::drawing::{Shape, ShapeKind, TextAlign, TextAnchor};
use vellum_grid_core::{Color, round_decimal};

use crate::app::{Spreadsheet, locale};
use crate::format_cells::color_button;
use crate::icons::Icon;
use crate::objects::{self, Object};
use crate::ribbon::widgets::{big, chrome_text, group, points_text, tool};
use crate::{fluent, menu, text, theme};

const GALLERY: [(text::Label, &[ShapeKind]); 5] = [
    (
        text::shapes_lines,
        &[ShapeKind::Line, ShapeKind::Arrow, ShapeKind::DoubleArrow],
    ),
    (
        text::shapes_rectangles,
        &[ShapeKind::Rectangle, ShapeKind::RoundedRectangle],
    ),
    (
        text::shapes_basic,
        &[
            ShapeKind::TextBox,
            ShapeKind::Ellipse,
            ShapeKind::Triangle,
            ShapeKind::RightTriangle,
            ShapeKind::Parallelogram,
            ShapeKind::Trapezoid,
            ShapeKind::Diamond,
            ShapeKind::Pentagon,
            ShapeKind::Hexagon,
            ShapeKind::Octagon,
            ShapeKind::Plus,
        ],
    ),
    (
        text::shapes_arrows,
        &[
            ShapeKind::RightArrow,
            ShapeKind::LeftArrow,
            ShapeKind::UpArrow,
            ShapeKind::DownArrow,
            ShapeKind::LeftRightArrow,
            ShapeKind::Chevron,
        ],
    ),
    (
        text::shapes_stars,
        &[ShapeKind::Star4, ShapeKind::Star5, ShapeKind::Star6],
    ),
];

const LINE_WEIGHTS: [u16; 9] = [25, 50, 75, 100, 150, 225, 300, 450, 600];

pub fn shape_name(kind: ShapeKind) -> &'static str {
    match kind {
        ShapeKind::TextBox => text::text_box(),
        ShapeKind::Line => text::shape_line(),
        ShapeKind::Arrow => text::shape_arrow(),
        ShapeKind::DoubleArrow => text::shape_double_arrow(),
        ShapeKind::Rectangle => text::shape_rectangle(),
        ShapeKind::RoundedRectangle => text::shape_rounded_rectangle(),
        ShapeKind::Ellipse => text::shape_ellipse(),
        ShapeKind::Triangle => text::shape_triangle(),
        ShapeKind::RightTriangle => text::shape_right_triangle(),
        ShapeKind::Parallelogram => text::shape_parallelogram(),
        ShapeKind::Trapezoid => text::shape_trapezoid(),
        ShapeKind::Diamond => text::shape_diamond(),
        ShapeKind::Pentagon => text::shape_pentagon(),
        ShapeKind::Hexagon => text::shape_hexagon(),
        ShapeKind::Octagon => text::shape_octagon(),
        ShapeKind::Plus => text::shape_plus(),
        ShapeKind::RightArrow => text::shape_right_arrow(),
        ShapeKind::LeftArrow => text::shape_left_arrow(),
        ShapeKind::UpArrow => text::shape_up_arrow(),
        ShapeKind::DownArrow => text::shape_down_arrow(),
        ShapeKind::LeftRightArrow => text::shape_left_right_arrow(),
        ShapeKind::Chevron => text::shape_chevron(),
        ShapeKind::Star4 => text::shape_star4(),
        ShapeKind::Star5 => text::shape_star5(),
        ShapeKind::Star6 => text::shape_star6(),
    }
}

pub fn shape_gallery(ui: &mut Ui, lines: bool, boxes: bool) -> Option<ShapeKind> {
    let mut chosen = None;
    ui.set_max_width(theme::SHAPE_GALLERY_WIDTH);
    for (title, kinds) in GALLERY {
        let kinds: Vec<ShapeKind> = kinds
            .iter()
            .copied()
            .filter(|k| if k.is_line() { lines } else { boxes })
            .collect();
        if kinds.is_empty() {
            continue;
        }
        ui.label(RichText::new(title()).strong());
        ui.horizontal_wrapped(|ui| {
            for kind in kinds {
                let size = vec2(theme::SHAPE_SWATCH, theme::SHAPE_SWATCH);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, CornerRadius::ZERO, theme::tool_hover());
                }
                let mut sample = Shape::new(kind, crate::app::origin());
                sample.fill = None;
                sample.line = Some(Color(theme::SHAPE_SAMPLE_LINE));
                sample.line_width = 75;
                let inner = rect.shrink(theme::SHAPE_SWATCH_PAD);
                objects::paint_shape(ui.painter(), inner, &sample, (false, false), 1.0);
                if response.on_hover_text(shape_name(kind)).clicked() {
                    chosen = Some(kind);
                }
            }
        });
    }
    chosen
}

fn weight_text(width: u16) -> String {
    format!(
        "{} {}",
        locale().format_number(f64::from(width) / 100.0),
        text::points()
    )
}

fn cm(pixels: u32) -> f64 {
    f64::from(pixels) / 96.0 * 2.54
}

fn pixels(cm: f64) -> u32 {
    (cm / 2.54 * 96.0).round().max(1.0) as u32
}

fn size_fields(ui: &mut Ui, size: (u32, u32), aspect: bool) -> Option<(u32, u32)> {
    let mut out = None;
    Grid::new("object-size").show(ui, |ui| {
        for (label, height) in [(text::height(), true), (text::width(), false)] {
            ui.label(chrome_text(label));
            let current = if height { size.1 } else { size.0 };
            let mut value = cm(current);
            let response = ui.add_sized(
                vec2(theme::SIZE_FIELD_WIDTH, theme::INPUT_HEIGHT),
                DragValue::new(&mut value)
                    .speed(0.0)
                    .range(0.01..=500.0)
                    .custom_formatter(|v, _| {
                        format!("{} cm", locale().format_number(round_decimal(v, 2)))
                    })
                    .custom_parser(|text| {
                        locale().parse_number(text.trim().trim_end_matches("cm").trim())
                    }),
            );
            ui.end_row();
            if response.changed() {
                let new = pixels(value);
                let ratio = f64::from(new) / f64::from(current.max(1));
                let scaled = |other: u32| ((f64::from(other) * ratio).round() as u32).max(1);
                out = Some(match (height, aspect) {
                    (true, true) => (scaled(size.0), new),
                    (true, false) => (size.0, new),
                    (false, true) => (new, scaled(size.1)),
                    (false, false) => (new, size.1),
                });
            }
        }
    });
    out
}

impl Spreadsheet {
    pub fn shape_button(&mut self, ui: &mut Ui) {
        let button = big(
            ui,
            Icon::Plain(fluent::SHAPES),
            text::shapes(),
            text::shapes(),
            true,
        );
        Popup::menu(&button).show(|ui| {
            if let Some(kind) = shape_gallery(ui, true, true) {
                self.start_shape_tool(kind);
                ui.close();
            }
        });
    }

    fn arrange_group(&mut self, ui: &mut Ui) {
        group(ui, text::arrange(), |ui| {
            ui.horizontal_top(|ui| {
                let forward = Icon::Plain(fluent::SQUARE_MULTIPLE);
                if big(
                    ui,
                    forward,
                    text::bring_forward(),
                    text::bring_forward(),
                    false,
                )
                .clicked()
                {
                    self.reorder_object(true);
                }
                let backward = Icon::Plain(fluent::SQUARE);
                if big(
                    ui,
                    backward,
                    text::send_backward(),
                    text::send_backward(),
                    false,
                )
                .clicked()
                {
                    self.reorder_object(false);
                }
            });
        });
    }

    pub fn shape_format_tab(&mut self, ui: &mut Ui) {
        let Some((index, shape)) = self.selected_shape() else {
            return;
        };
        let shape = shape.clone();
        let line = shape.kind.is_line();
        group(ui, text::insert_shapes(), |ui| {
            ui.horizontal_top(|ui| {
                self.shape_button(ui);
                let icon = Icon::Plain(fluent::TABLE_EDIT);
                let button = big(ui, icon, text::change_shape(), text::change_shape(), true);
                Popup::menu(&button).show(|ui| {
                    if let Some(kind) = shape_gallery(ui, line, !line) {
                        self.update_shape(index, |s| s.kind = kind);
                        ui.close();
                    }
                });
            });
        });
        group(ui, text::shape_styles(), |ui| {
            ui.horizontal_top(|ui| {
                Grid::new("shape-colors").show(ui, |ui| {
                    if !line {
                        ui.label(chrome_text(text::shape_fill()));
                        if let Some(fill) =
                            color_button(ui, "shape-fill", shape.fill, text::no_fill())
                        {
                            self.update_shape(index, |s| s.fill = fill);
                        }
                        ui.end_row();
                    }
                    ui.label(chrome_text(text::shape_outline()));
                    if let Some(color) =
                        color_button(ui, "shape-line", shape.line, text::no_outline())
                    {
                        self.update_shape(index, |s| s.line = color);
                    }
                    ui.end_row();
                });
                ui.vertical(|ui| {
                    ui.label(chrome_text(text::outline_weight()));
                    let mut weight = None;
                    ComboBox::from_id_salt("shape-weight")
                        .width(theme::WEIGHT_COMBO_WIDTH)
                        .selected_text(weight_text(shape.line_width))
                        .show_ui(ui, |ui| {
                            for width in LINE_WEIGHTS {
                                let label = weight_text(width);
                                if ui
                                    .selectable_label(shape.line_width == width, label)
                                    .clicked()
                                {
                                    weight = Some(width);
                                }
                            }
                        });
                    if let Some(width) = weight {
                        self.update_shape(index, |s| s.line_width = width);
                    }
                });
            });
        });
        if !line {
            self.shape_text_group(ui, index, &shape);
        }
        self.arrange_group(ui);
        group(ui, text::size(), |ui| {
            ui.vertical(|ui| {
                if let Some(size) = size_fields(ui, shape.size, false) {
                    self.update_shape(index, |s| s.size = size);
                }
            });
        });
    }

    fn shape_text_group(&mut self, ui: &mut Ui, index: usize, shape: &Shape) {
        group(ui, text::shape_text(), |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let mut size = None;
                    ComboBox::from_id_salt("shape-font-size")
                        .width(theme::SIZE_COMBO_WIDTH)
                        .selected_text(points_text(shape.font_size))
                        .show_ui(ui, |ui| {
                            for points in theme::FONT_SIZES {
                                let tenths = points * 10;
                                if ui
                                    .selectable_label(shape.font_size == tenths, points.to_string())
                                    .clicked()
                                {
                                    size = Some(tenths);
                                }
                            }
                        })
                        .response
                        .on_hover_text(text::font_size());
                    if let Some(size) = size {
                        self.update_shape(index, |s| s.font_size = size);
                    }
                    if tool(ui, Icon::Bold, text::bold(), shape.bold).clicked() {
                        self.update_shape(index, |s| s.bold = !s.bold);
                    }
                    if tool(ui, Icon::Italic, text::italic(), shape.italic).clicked() {
                        self.update_shape(index, |s| s.italic = !s.italic);
                    }
                    if let Some(color) =
                        color_button(ui, "shape-text", Some(shape.color), text::automatic())
                    {
                        let color = color.unwrap_or(Color(0x000000));
                        self.update_shape(index, |s| s.color = color);
                    }
                });
                ui.horizontal(|ui| {
                    for (icon, tip, align) in [
                        (Icon::AlignLeft, text::align_left(), TextAlign::Left),
                        (Icon::AlignCenter, text::align_center(), TextAlign::Center),
                        (Icon::AlignRight, text::align_right(), TextAlign::Right),
                    ] {
                        if tool(ui, icon, tip, shape.align == align).clicked() {
                            self.update_shape(index, |s| s.align = align);
                        }
                    }
                    for (icon, tip, anchor) in [
                        (Icon::AlignTop, text::align_top(), TextAnchor::Top),
                        (Icon::AlignMiddle, text::align_middle(), TextAnchor::Middle),
                        (Icon::AlignBottom, text::align_bottom(), TextAnchor::Bottom),
                    ] {
                        if tool(ui, icon, tip, shape.text_anchor == anchor).clicked() {
                            self.update_shape(index, |s| s.text_anchor = anchor);
                        }
                    }
                });
            });
        });
    }

    pub fn picture_format_tab(&mut self, ui: &mut Ui) {
        let Some((index, picture)) = self.selected_picture() else {
            return;
        };
        let size = picture.size;
        group(ui, text::adjust(), |ui| {
            let icon = Icon::Plain(fluent::ARROW_SYNC);
            if big(
                ui,
                icon,
                text::reset_picture(),
                text::reset_picture(),
                false,
            )
            .clicked()
            {
                self.reset_picture(index);
            }
        });
        self.arrange_group(ui);
        group(ui, text::size(), |ui| {
            ui.vertical(|ui| {
                if let Some(size) = size_fields(ui, size, true) {
                    self.update_picture(index, |p| p.size = size);
                }
            });
        });
    }

    pub fn object_menu(&mut self, ui: &mut Ui, object: Object) {
        self.object = Some(object);
        let editable = match object {
            Object::Shape(index) => self
                .sheet()
                .layout()
                .shapes
                .get(index)
                .is_some_and(|s| !s.kind.is_line()),
            _ => false,
        };
        if let Object::Shape(index) = object
            && editable
            && menu::icon_item(ui, Icon::Plain(fluent::TEXTBOX), text::edit_text())
        {
            self.begin_shape_text(index);
        }
        if let Object::Picture(index) = object
            && menu::icon_item(ui, Icon::Plain(fluent::ARROW_SYNC), text::reset_picture())
        {
            self.reset_picture(index);
        }
        let forward = Icon::Plain(fluent::SQUARE_MULTIPLE);
        if menu::icon_item(ui, forward, text::bring_forward()) {
            self.reorder_object(true);
        }
        if menu::icon_item(ui, Icon::Plain(fluent::SQUARE), text::send_backward()) {
            self.reorder_object(false);
        }
        menu::separator(ui);
        if menu::icon_item(ui, Icon::Plain(fluent::DELETE), text::delete_object()) {
            self.delete_object();
        }
    }
}
