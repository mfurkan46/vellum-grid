use eframe::egui::{
    self, Area, ComboBox, Context, FontFamily, Frame, Id, Margin, Order, Pos2, Rect, RichText,
    Sense, Shape, Stroke, TextEdit, Ui, vec2,
};
use vellum_grid_core::TotalFunction;
use vellum_grid_core::formula::{self, Expr};
use vellum_grid_core::validation::{
    AlertStyle, Allow, Compare, Validation, literal, literal_items,
};
use vellum_grid_core::{CellAddr, CellRange, CellValue, EN, numfmt};

use crate::app::{Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::icons::{self, Icon};
use crate::{assist, fluent, fonts, text, theme};

#[derive(Debug, Clone)]
pub struct ValidationForm {
    pub ranges: Vec<CellRange>,
    pub tab: usize,
    pub rule: Validation,
    pub first: String,
    pub second: String,
    pub error: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct ValidationAlert {
    pub rule: Validation,
}

fn allow_label(allow: Allow) -> &'static str {
    match allow {
        Allow::Any => text::allow_any(),
        Allow::Whole => text::allow_whole(),
        Allow::Decimal => text::allow_decimal(),
        Allow::List => text::allow_list(),
        Allow::Date => text::allow_date(),
        Allow::Time => text::allow_time(),
        Allow::TextLength => text::allow_length(),
        Allow::Custom => text::allow_custom(),
    }
}

fn compare_label(compare: Compare) -> &'static str {
    match compare {
        Compare::Between => text::cmp_between(),
        Compare::NotBetween => text::cmp_not_between(),
        Compare::Equal => text::cmp_equal(),
        Compare::NotEqual => text::cmp_not_equal(),
        Compare::Greater => text::cmp_greater(),
        Compare::Less => text::cmp_less(),
        Compare::GreaterOrEqual => text::cmp_greater_equal(),
        Compare::LessOrEqual => text::cmp_less_equal(),
    }
}

fn style_label(style: AlertStyle) -> &'static str {
    match style {
        AlertStyle::Stop => text::style_stop(),
        AlertStyle::Warning => text::style_warning(),
        AlertStyle::Information => text::style_information(),
    }
}

pub fn operand_text(formula: &str, allow: Allow) -> String {
    if formula.is_empty() {
        return String::new();
    }
    if allow == Allow::List
        && let Some(items) = literal_items(formula)
    {
        return items.join(&locale().list.to_string());
    }
    match formula::parse(formula, &EN) {
        Ok(Expr::Number(n)) => {
            let code = match allow {
                Allow::Date => numfmt::DATE,
                Allow::Time => numfmt::TIME,
                _ => "",
            };
            locale().format_value(&CellValue::Number(n), code)
        }
        Ok(expr) => format!("={}", expr.write(locale())),
        Err(_) => formula.to_owned(),
    }
}

pub fn operand_formula(input: &str, allow: Allow) -> Option<String> {
    let input = input.trim();
    if let Some(body) = input.strip_prefix('=') {
        return formula::parse(body, locale())
            .ok()
            .map(|expr| expr.write(&EN));
    }
    if allow == Allow::List {
        let items: Vec<String> = input
            .split(locale().list)
            .map(|item| item.trim().to_owned())
            .filter(|item| !item.is_empty())
            .collect();
        return (!items.is_empty()).then(|| literal(&items));
    }
    if allow == Allow::Custom {
        return formula::parse(input, locale())
            .ok()
            .map(|expr| expr.write(&EN));
    }
    locale()
        .parse_value(input)
        .map(|n| Expr::Number(n).write(&EN))
}

fn field(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(
        TextEdit::singleline(value)
            .desired_width(theme::NAME_INPUT_WIDTH)
            .background_color(theme::input_bg()),
    );
    ui.end_row();
}

pub fn form_ui(ui: &mut Ui, form: &mut ValidationForm) {
    ui.set_width(theme::VALIDATION_WIDTH);
    ui.horizontal(|ui| {
        for (index, label) in [text::tab_settings(), text::tab_input(), text::tab_error()]
            .into_iter()
            .enumerate()
        {
            if ui.selectable_label(form.tab == index, label).clicked() {
                form.tab = index;
            }
        }
    });
    ui.separator();
    let rule = &mut form.rule;
    match form.tab {
        0 => {
            ui.label(
                RichText::new(text::validation_criteria())
                    .family(FontFamily::Name(fonts::CHROME_BOLD.into())),
            );
            egui::Grid::new("validation_settings")
                .num_columns(2)
                .spacing(vec2(theme::DIALOG_SPACING, theme::DIALOG_SPACING))
                .show(ui, |ui| {
                    ui.label(text::allow());
                    ComboBox::from_id_salt("validation_allow")
                        .selected_text(allow_label(rule.allow))
                        .width(theme::NAME_INPUT_WIDTH)
                        .show_ui(ui, |ui| {
                            for allow in Allow::ALL {
                                ui.selectable_value(&mut rule.allow, allow, allow_label(allow));
                            }
                        });
                    ui.end_row();
                    if rule.allow.compares() {
                        ui.label(text::data_compare());
                        ComboBox::from_id_salt("validation_compare")
                            .selected_text(compare_label(rule.compare))
                            .width(theme::NAME_INPUT_WIDTH)
                            .show_ui(ui, |ui| {
                                for compare in Compare::ALL {
                                    ui.selectable_value(
                                        &mut rule.compare,
                                        compare,
                                        compare_label(compare),
                                    );
                                }
                            });
                        ui.end_row();
                        if rule.compare.two_operands() {
                            field(ui, text::validation_min(), &mut form.first);
                            field(ui, text::validation_max(), &mut form.second);
                        } else {
                            field(ui, text::value_label(), &mut form.first);
                        }
                    }
                    match rule.allow {
                        Allow::List => field(ui, text::source(), &mut form.first),
                        Allow::Custom => field(ui, text::formula_label(), &mut form.first),
                        _ => {}
                    }
                });
            if rule.allow != Allow::Any {
                ui.checkbox(&mut rule.ignore_blank, text::ignore_blank());
            }
            if rule.allow == Allow::List {
                ui.checkbox(&mut rule.dropdown, text::in_cell_dropdown());
            }
        }
        1 => {
            ui.checkbox(&mut rule.show_input, text::show_input());
            ui.label(text::when_selected());
            ui.label(text::title_label());
            ui.add(
                TextEdit::singleline(&mut rule.input_title)
                    .desired_width(f32::INFINITY)
                    .background_color(theme::input_bg()),
            );
            ui.label(text::input_message());
            ui.add(
                TextEdit::multiline(&mut rule.input_message)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .background_color(theme::input_bg()),
            );
        }
        _ => {
            ui.checkbox(&mut rule.show_error, text::show_error());
            ui.horizontal(|ui| {
                ui.label(text::style_label());
                ComboBox::from_id_salt("validation_style")
                    .selected_text(style_label(rule.style))
                    .show_ui(ui, |ui| {
                        for style in [
                            AlertStyle::Stop,
                            AlertStyle::Warning,
                            AlertStyle::Information,
                        ] {
                            ui.selectable_value(&mut rule.style, style, style_label(style));
                        }
                    });
            });
            ui.label(text::title_label());
            ui.add(
                TextEdit::singleline(&mut rule.error_title)
                    .desired_width(f32::INFINITY)
                    .background_color(theme::input_bg()),
            );
            ui.label(text::error_message_label());
            ui.add(
                TextEdit::multiline(&mut rule.error_message)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .background_color(theme::input_bg()),
            );
        }
    }
    if let Some(error) = form.error {
        ui.label(RichText::new(error).color(theme::error_text()));
    }
}

pub fn alert_ui(ui: &mut Ui, alert: &ValidationAlert) {
    ui.set_max_width(theme::DIALOG_WIDTH);
    let message = if alert.rule.error_message.is_empty() {
        text::invalid_entry()
    } else {
        alert.rule.error_message.as_str()
    };
    ui.horizontal(|ui| {
        let glyph = match alert.rule.style {
            AlertStyle::Stop => fluent::ERROR_CIRCLE,
            AlertStyle::Warning => fluent::WARNING,
            AlertStyle::Information => fluent::INFO,
        };
        let (rect, _) =
            ui.allocate_exact_size(vec2(theme::ALERT_ICON, theme::ALERT_ICON), Sense::hover());
        let color = match alert.rule.style {
            AlertStyle::Stop => theme::TRACE_ERROR,
            AlertStyle::Warning => theme::ISSUE_ICON,
            AlertStyle::Information => theme::TRACE,
        };
        icons::paint(ui.painter(), rect, Icon::Plain(glyph), color, None);
        ui.label(message);
    });
    if alert.rule.style == AlertStyle::Warning {
        ui.label(text::continue_question());
    }
}

impl Spreadsheet {
    pub fn open_validation(&mut self) {
        let ranges = self.ranges();
        let existing = self.doc.validation_at(self.sel.active).cloned();
        let rule = existing.unwrap_or_else(|| Validation::new(ranges.clone()));
        self.dialog = Some(Dialog::Validation(ValidationForm {
            first: operand_text(&rule.formula1, rule.allow),
            second: operand_text(&rule.formula2, rule.allow),
            ranges,
            tab: 0,
            rule,
            error: None,
        }));
    }

    pub fn apply_validation(&mut self, mut form: ValidationForm) {
        let mut rule = form.rule.clone();
        let needs_first = rule.allow != Allow::Any;
        let needs_second = rule.allow.compares() && rule.compare.two_operands();
        rule.formula1 = String::new();
        rule.formula2 = String::new();
        if needs_first {
            match operand_formula(&form.first, rule.allow) {
                Some(formula) => rule.formula1 = formula,
                None => {
                    form.error = Some(text::bad_operand());
                    self.dialog = Some(Dialog::Validation(form));
                    return;
                }
            }
        }
        if needs_second {
            match operand_formula(&form.second, rule.allow) {
                Some(formula) => rule.formula2 = formula,
                None => {
                    form.error = Some(text::bad_operand());
                    self.dialog = Some(Dialog::Validation(form));
                    return;
                }
            }
        }
        let empty = rule.allow == Allow::Any
            && rule.input_title.is_empty()
            && rule.input_message.is_empty();
        self.doc
            .set_validation(&form.ranges, (!empty).then_some(rule));
    }

    pub fn check_entry(&mut self, addr: CellAddr, input: &str) -> Option<Validation> {
        let cell = self.doc.parse(input).ok()?;
        self.doc
            .violated(addr, &cell)
            .filter(|rule| rule.show_error)
    }

    pub fn input_message(&self, ctx: &Context, cell: Rect) {
        if self.modal_open() || self.list_popup.is_some() {
            return;
        }
        let Some(rule) = self.doc.validation_at(self.sel.active) else {
            return;
        };
        if !rule.show_input || (rule.input_title.is_empty() && rule.input_message.is_empty()) {
            return;
        }
        Area::new(Id::unique("validation-input"))
            .order(Order::Tooltip)
            .interactable(false)
            .fixed_pos(cell.right_bottom() + vec2(theme::ISSUE_GAP * 4.0, theme::ISSUE_GAP * 4.0))
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .fill(theme::tip_bg())
                    .inner_margin(Margin::same(theme::MENU_MARGIN))
                    .show(ui, |ui| {
                        ui.set_max_width(theme::ASSIST_HELP_WIDTH);
                        if !rule.input_title.is_empty() {
                            ui.label(
                                RichText::new(&rule.input_title)
                                    .family(FontFamily::Name(fonts::CHROME_BOLD.into()))
                                    .color(theme::chrome_text()),
                            );
                        }
                        if !rule.input_message.is_empty() {
                            ui.label(
                                RichText::new(&rule.input_message).color(theme::chrome_text()),
                            );
                        }
                    });
            });
    }

    pub fn list_button(&mut self, ui: &mut Ui, cell: Rect) {
        let addr = self.sel.active;
        let total = self.total_source();
        let show = total.is_some()
            || self
                .doc
                .validation_at(addr)
                .is_some_and(|rule| rule.allow == Allow::List && rule.dropdown);
        if !show {
            self.list_popup = None;
            return;
        }
        let rect = Rect::from_min_size(
            cell.right_top() + vec2(1.0, 0.0),
            vec2(theme::LIST_BUTTON, cell.height()),
        );
        self.list_button_rect = Some(rect);
        let response = ui.interact(rect, ui.scope_id().with("validation-list"), Sense::click());
        let fill = if response.hovered() {
            theme::tool_hover()
        } else {
            theme::header_bg()
        };
        ui.painter().rect_filled(rect, 0.0, fill);
        ui.painter().rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, theme::chrome_line()),
            egui::StrokeKind::Inside,
        );
        icons::paint(
            ui.painter(),
            rect.shrink(3.0),
            Icon::Plain(fluent::CHEVRON_DOWN),
            theme::icon(),
            None,
        );
        if response.clicked() {
            self.list_popup = match self.list_popup {
                Some(_) => None,
                None => Some(addr),
            };
        }
        if self.list_popup == Some(addr) {
            self.list_popup_ui(ui.ctx(), cell, rect, total);
        }
    }

    fn list_popup_ui(
        &mut self,
        ctx: &Context,
        cell: Rect,
        button: Rect,
        total: Option<(usize, usize)>,
    ) {
        let items: Vec<String> = match total {
            Some(_) => TotalFunction::MENU
                .iter()
                .map(|f| crate::tables_ui::total_label(*f).to_owned())
                .collect(),
            None => self.doc.list_items(self.sel.active),
        };
        let mut chosen = None;
        let area = Area::new(Id::unique("validation-list-popup"))
            .order(Order::Foreground)
            .fixed_pos(Pos2::new(cell.left(), cell.bottom() + 1.0))
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .inner_margin(Margin::same(theme::MENU_MARGIN / 2))
                    .show(ui, |ui| {
                        ui.set_width((button.right() - cell.left()).max(theme::LIST_MIN_WIDTH));
                        egui::ScrollArea::vertical()
                            .max_height(theme::ASSIST_HEIGHT)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                for (index, item) in items.iter().enumerate() {
                                    if assist::list_row(ui, None, item, false).clicked() {
                                        chosen = Some(index);
                                    }
                                }
                            });
                    });
            });
        if let Some(index) = chosen {
            self.list_popup = None;
            let addr = self.sel.active;
            self.doc.tick(crate::clock::now());
            match total {
                Some((table, col)) => {
                    self.doc
                        .set_table_total(table, col, TotalFunction::MENU[index]);
                }
                None => {
                    self.doc.enter(addr, &items[index]).ok();
                }
            }
            return;
        }
        let pressed_outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|pos| !area.response.rect.contains(pos) && !button.contains(pos))
        });
        if pressed_outside {
            self.list_popup = None;
        }
    }

    pub fn toggle_list(&mut self) {
        let addr = self.sel.active;
        let listed = self.total_source().is_some()
            || self
                .doc
                .validation_at(addr)
                .is_some_and(|rule| rule.allow == Allow::List);
        self.list_popup = listed.then_some(addr).filter(|_| self.list_popup.is_none());
    }

    pub fn refresh_circles(&mut self) {
        if !self.circles {
            self.circle_cells.clear();
            return;
        }
        let key = (self.doc.revision(), self.doc.active());
        if self.circle_key != Some(key) {
            self.circle_cells = self.doc.invalid_cells();
            self.circle_key = Some(key);
        }
    }
}

pub fn paint_circle(painter: &egui::Painter, rect: Rect) {
    let radius = vec2(
        rect.width() / 2.0 + theme::CIRCLE_PAD,
        rect.height() / 2.0 + theme::CIRCLE_PAD,
    );
    painter.add(Shape::ellipse_stroke(
        rect.center(),
        radius,
        Stroke::new(theme::CIRCLE_WIDTH, theme::TRACE_ERROR),
    ));
}
