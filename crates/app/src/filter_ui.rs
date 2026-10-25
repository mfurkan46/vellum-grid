use eframe::egui::{self, ComboBox, RichText, TextEdit, Ui, vec2};
use vellum_grid_core::filter::{ColumnFilter, Condition, Dynamic, Operator};
use vellum_grid_core::numfmt::{self, Kind};
use vellum_grid_core::{CellAddr, CellRange, CellValue, Color, EN, column_name, current_region};

use crate::app::{Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::{text, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Number,
    Date,
    Text,
}

impl FilterKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Number => text::number_filters(),
            Self::Date => text::date_filters(),
            Self::Text => text::text_filters(),
        }
    }

    pub fn presets(self) -> &'static [Preset] {
        match self {
            Self::Number => &[
                Preset::Op(Operator::Equal),
                Preset::Op(Operator::NotEqual),
                Preset::Op(Operator::Greater),
                Preset::Op(Operator::GreaterOrEqual),
                Preset::Op(Operator::Less),
                Preset::Op(Operator::LessOrEqual),
                Preset::Between,
                Preset::Top,
                Preset::Dynamic(Dynamic::AboveAverage),
                Preset::Dynamic(Dynamic::BelowAverage),
                Preset::Custom,
            ],
            Self::Date => &[
                Preset::Op(Operator::Equal),
                Preset::Op(Operator::Less),
                Preset::Op(Operator::Greater),
                Preset::Between,
                Preset::Dynamic(Dynamic::Tomorrow),
                Preset::Dynamic(Dynamic::Today),
                Preset::Dynamic(Dynamic::Yesterday),
                Preset::Dynamic(Dynamic::NextWeek),
                Preset::Dynamic(Dynamic::ThisWeek),
                Preset::Dynamic(Dynamic::LastWeek),
                Preset::Dynamic(Dynamic::NextMonth),
                Preset::Dynamic(Dynamic::ThisMonth),
                Preset::Dynamic(Dynamic::LastMonth),
                Preset::Dynamic(Dynamic::NextQuarter),
                Preset::Dynamic(Dynamic::ThisQuarter),
                Preset::Dynamic(Dynamic::LastQuarter),
                Preset::Dynamic(Dynamic::NextYear),
                Preset::Dynamic(Dynamic::ThisYear),
                Preset::Dynamic(Dynamic::LastYear),
                Preset::Dynamic(Dynamic::YearToDate),
                Preset::Custom,
            ],
            Self::Text => &[
                Preset::Op(Operator::Equal),
                Preset::Op(Operator::NotEqual),
                Preset::Op(Operator::BeginsWith),
                Preset::Op(Operator::EndsWith),
                Preset::Op(Operator::Contains),
                Preset::Op(Operator::NotContains),
                Preset::Custom,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Op(Operator),
    Between,
    Top,
    Dynamic(Dynamic),
    Custom,
}

pub fn operator_label(op: Operator, kind: FilterKind) -> &'static str {
    match (op, kind) {
        (Operator::Less, FilterKind::Date) => text::op_before(),
        (Operator::Greater, FilterKind::Date) => text::op_after(),
        (Operator::Equal, _) => text::op_equal(),
        (Operator::NotEqual, _) => text::op_not_equal(),
        (Operator::Greater, _) => text::op_greater(),
        (Operator::GreaterOrEqual, _) => text::op_greater_equal(),
        (Operator::Less, _) => text::op_less(),
        (Operator::LessOrEqual, _) => text::op_less_equal(),
        (Operator::BeginsWith, _) => text::op_begins(),
        (Operator::NotBeginsWith, _) => text::op_not_begins(),
        (Operator::EndsWith, _) => text::op_ends(),
        (Operator::NotEndsWith, _) => text::op_not_ends(),
        (Operator::Contains, _) => text::op_contains(),
        (Operator::NotContains, _) => text::op_not_contains(),
    }
}

pub fn dynamic_label(dynamic: Dynamic) -> &'static str {
    match dynamic {
        Dynamic::AboveAverage => text::above_average(),
        Dynamic::BelowAverage => text::below_average(),
        Dynamic::Tomorrow => text::dyn_tomorrow(),
        Dynamic::Today => text::dyn_today(),
        Dynamic::Yesterday => text::dyn_yesterday(),
        Dynamic::NextWeek => text::dyn_next_week(),
        Dynamic::ThisWeek => text::dyn_this_week(),
        Dynamic::LastWeek => text::dyn_last_week(),
        Dynamic::NextMonth => text::dyn_next_month(),
        Dynamic::ThisMonth => text::dyn_this_month(),
        Dynamic::LastMonth => text::dyn_last_month(),
        Dynamic::NextQuarter => text::dyn_next_quarter(),
        Dynamic::ThisQuarter => text::dyn_this_quarter(),
        Dynamic::LastQuarter => text::dyn_last_quarter(),
        Dynamic::NextYear => text::dyn_next_year(),
        Dynamic::ThisYear => text::dyn_this_year(),
        Dynamic::LastYear => text::dyn_last_year(),
        Dynamic::YearToDate => text::dyn_year_to_date(),
    }
}

pub fn preset_label(preset: Preset, kind: FilterKind) -> String {
    let ellipsis = |label: &str| format!("{label}…");
    match preset {
        Preset::Op(op) => ellipsis(operator_label(op, kind)),
        Preset::Between => ellipsis(text::op_between()),
        Preset::Top => ellipsis(text::top_ten()),
        Preset::Dynamic(dynamic) => dynamic_label(dynamic).to_owned(),
        Preset::Custom => ellipsis(text::custom_filter()),
    }
}

#[derive(Debug, Clone)]
pub struct CustomFilterState {
    pub col: u32,
    pub heading: String,
    pub kind: FilterKind,
    pub first: Operator,
    pub first_value: String,
    pub second: Option<Operator>,
    pub second_value: String,
    pub and: bool,
    pub values: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TopState {
    pub col: u32,
    pub bottom: bool,
    pub count: String,
    pub percent: bool,
}

fn shown_value(value: &str, kind: FilterKind) -> String {
    match EN.parse_number(value) {
        Some(n) if kind == FilterKind::Date => {
            locale().format_value(&CellValue::Number(n), numfmt::DATE)
        }
        Some(n) => locale().format_number(n),
        None => value.to_owned(),
    }
}

fn stored_value(input: &str) -> String {
    let input = input.trim();
    match locale().parse_value(input) {
        Some(n) => vellum_grid_core::formula::Expr::Number(n).write(&EN),
        None => input.to_owned(),
    }
}

fn operator_combo(
    ui: &mut Ui,
    id: &str,
    op: &mut Option<Operator>,
    kind: FilterKind,
    optional: bool,
) {
    let label = op.map_or("", |op| operator_label(op, kind));
    ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(theme::FILTER_OP_WIDTH)
        .show_ui(ui, |ui| {
            if optional {
                ui.selectable_value(op, None, " ");
            }
            for option in Operator::ALL {
                ui.selectable_value(op, Some(option), operator_label(option, kind));
            }
        });
}

fn value_combo(ui: &mut Ui, id: &str, value: &mut String, values: &[String]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add(
            TextEdit::singleline(value)
                .desired_width(theme::FILTER_VALUE_WIDTH)
                .background_color(theme::input_bg()),
        );
        ComboBox::from_id_salt(id)
            .selected_text("")
            .width(theme::LIST_BUTTON)
            .show_ui(ui, |ui| {
                for option in values {
                    if ui.selectable_label(false, option).clicked() {
                        value.clone_from(option);
                    }
                }
            });
    });
}

pub fn custom_ui(ui: &mut Ui, state: &mut CustomFilterState) {
    ui.set_width(theme::CUSTOM_FILTER_WIDTH);
    ui.label(text::show_rows_where());
    ui.label(RichText::new(&state.heading).strong());
    let kind = state.kind;
    ui.horizontal(|ui| {
        let mut first = Some(state.first);
        operator_combo(ui, "filter_first_op", &mut first, kind, false);
        state.first = first.unwrap_or(Operator::Equal);
        value_combo(
            ui,
            "filter_first_value",
            &mut state.first_value,
            &state.values,
        );
    });
    ui.horizontal(|ui| {
        ui.radio_value(&mut state.and, true, text::filter_and());
        ui.radio_value(&mut state.and, false, text::filter_or());
    });
    ui.horizontal(|ui| {
        operator_combo(ui, "filter_second_op", &mut state.second, kind, true);
        value_combo(
            ui,
            "filter_second_value",
            &mut state.second_value,
            &state.values,
        );
    });
    ui.label(RichText::new(text::wildcard_hint()).color(theme::chrome_text_muted()));
}

pub fn top_ui(ui: &mut Ui, state: &mut TopState) {
    ui.set_width(theme::CUSTOM_FILTER_WIDTH);
    ui.label(text::show());
    ui.horizontal(|ui| {
        ComboBox::from_id_salt("top_side")
            .selected_text(if state.bottom {
                text::bottom()
            } else {
                text::top()
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.bottom, false, text::top());
                ui.selectable_value(&mut state.bottom, true, text::bottom());
            });
        ui.add(
            TextEdit::singleline(&mut state.count)
                .desired_width(theme::NUMBER_INPUT_WIDTH)
                .background_color(theme::input_bg()),
        );
        ComboBox::from_id_salt("top_unit")
            .selected_text(if state.percent {
                text::percent()
            } else {
                text::items()
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.percent, false, text::items());
                ui.selectable_value(&mut state.percent, true, text::percent());
            });
    });
}

impl Spreadsheet {
    pub fn filter_rows_of(&self) -> Vec<u32> {
        self.sheet()
            .layout()
            .filter
            .as_ref()
            .map(|filter| (filter.range.start().row() + 1..=filter.range.end().row()).collect())
            .unwrap_or_default()
    }

    pub fn column_kind(&self, col: u32) -> FilterKind {
        let (mut numbers, mut dates, mut texts) = (0, 0, 0);
        for row in self.filter_rows_of() {
            let Ok(addr) = CellAddr::new(row, col) else {
                continue;
            };
            match self.sheet().get(addr) {
                Some(cell) if matches!(cell.value, CellValue::Number(_)) => {
                    if numfmt::kind(cell.number_format()) == Kind::Date {
                        dates += 1;
                    } else {
                        numbers += 1;
                    }
                }
                Some(cell) if matches!(cell.value, CellValue::Text(_)) => texts += 1,
                _ => {}
            }
        }
        if texts >= numbers + dates {
            FilterKind::Text
        } else if dates > numbers {
            FilterKind::Date
        } else {
            FilterKind::Number
        }
    }

    pub fn column_colors(&self, col: u32, fill: bool) -> Vec<Option<Color>> {
        let mut found = Vec::new();
        for row in self.filter_rows_of() {
            let Ok(addr) = CellAddr::new(row, col) else {
                continue;
            };
            let style = self.sheet().get(addr).map(|cell| cell.style().clone());
            let color = style.and_then(|style| if fill { style.fill } else { style.color });
            if !found.contains(&color) {
                found.push(color);
            }
        }
        found
    }

    pub fn apply_preset(&mut self, col: u32, kind: FilterKind, preset: Preset) {
        match preset {
            Preset::Dynamic(dynamic) => {
                self.doc
                    .set_filter_column(col, Some(ColumnFilter::Dynamic(dynamic)));
            }
            Preset::Top => {
                self.dialog = Some(Dialog::Top10(TopState {
                    col,
                    bottom: false,
                    count: "10".to_owned(),
                    percent: false,
                }));
            }
            Preset::Op(op) => self.open_custom_filter(col, kind, Some((op, None))),
            Preset::Between => self.open_custom_filter(
                col,
                kind,
                Some((Operator::GreaterOrEqual, Some(Operator::LessOrEqual))),
            ),
            Preset::Custom => self.open_custom_filter(col, kind, None),
        }
    }

    fn open_custom_filter(
        &mut self,
        col: u32,
        kind: FilterKind,
        preset: Option<(Operator, Option<Operator>)>,
    ) {
        let existing = self
            .sheet()
            .layout()
            .filter
            .as_ref()
            .and_then(|filter| filter.criteria.get(&col).cloned());
        let (first, first_value, second, second_value, and) = match (preset, existing) {
            (Some((first, second)), _) => (first, String::new(), second, String::new(), true),
            (None, Some(ColumnFilter::Custom { first, second, and })) => (
                first.op,
                shown_value(&first.value, kind),
                second.as_ref().map(|c| c.op),
                second.map_or_else(String::new, |c| shown_value(&c.value, kind)),
                and,
            ),
            _ => (Operator::Equal, String::new(), None, String::new(), true),
        };
        let header = self.filter_header(col);
        self.dialog = Some(Dialog::CustomFilter(CustomFilterState {
            col,
            heading: self.doc.filter_text(header),
            kind,
            first,
            first_value,
            second,
            second_value,
            and,
            values: self
                .doc
                .filter_values(col)
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect(),
        }));
    }

    pub fn apply_custom_filter(&mut self, state: CustomFilterState) {
        let first = Condition {
            op: state.first,
            value: stored_value(&state.first_value),
        };
        let second = state.second.map(|op| Condition {
            op,
            value: stored_value(&state.second_value),
        });
        self.doc.set_filter_column(
            state.col,
            Some(ColumnFilter::Custom {
                first,
                second,
                and: state.and,
            }),
        );
    }

    pub fn apply_top(&mut self, state: TopState) {
        let Some(count) = locale()
            .parse_number(state.count.trim())
            .filter(|n| *n > 0.0)
        else {
            self.dialog = Some(Dialog::Top10(state));
            return;
        };
        self.doc.set_filter_column(
            state.col,
            Some(ColumnFilter::Top {
                count,
                percent: state.percent,
                bottom: state.bottom,
            }),
        );
    }
}

#[derive(Debug, Clone)]
pub struct AdvancedState {
    pub copy: bool,
    pub list: String,
    pub criteria: String,
    pub dest: String,
    pub unique: bool,
    pub error: Option<&'static str>,
}

fn reference_field(ui: &mut Ui, label: &str, value: &mut String, enabled: bool) {
    ui.label(label);
    ui.add_enabled(
        enabled,
        TextEdit::singleline(value)
            .desired_width(theme::NAME_INPUT_WIDTH)
            .background_color(theme::input_bg()),
    );
    ui.end_row();
}

pub fn advanced_ui(ui: &mut Ui, state: &mut AdvancedState) {
    ui.set_width(theme::CUSTOM_FILTER_WIDTH);
    ui.label(text::action());
    ui.radio_value(&mut state.copy, false, text::filter_in_place());
    ui.radio_value(&mut state.copy, true, text::copy_elsewhere());
    egui::Grid::new("advanced_filter")
        .num_columns(2)
        .spacing(vec2(theme::DIALOG_SPACING, theme::DIALOG_SPACING))
        .show(ui, |ui| {
            reference_field(ui, text::list_range(), &mut state.list, true);
            reference_field(ui, text::criteria_range(), &mut state.criteria, true);
            reference_field(ui, text::copy_to(), &mut state.dest, state.copy);
        });
    ui.checkbox(&mut state.unique, text::unique_only());
    if let Some(error) = state.error {
        ui.label(RichText::new(error).color(theme::error_text()));
    }
}

fn absolute(range: CellRange) -> String {
    let cell = |addr: CellAddr| format!("${}${}", column_name(addr.col()), addr.row() + 1);
    if range.is_single() {
        cell(range.start())
    } else {
        format!("{}:{}", cell(range.start()), cell(range.end()))
    }
}

impl Spreadsheet {
    pub fn open_advanced_filter(&mut self) {
        let list = match self.sheet().layout().filter.as_ref() {
            Some(filter) => filter.range,
            None => current_region(self.sheet(), self.sel.active),
        };
        self.dialog = Some(Dialog::AdvancedFilter(AdvancedState {
            copy: false,
            list: absolute(list),
            criteria: String::new(),
            dest: String::new(),
            unique: false,
            error: None,
        }));
    }

    pub fn apply_advanced_filter(&mut self, mut state: AdvancedState) {
        let parse = |text: &str| crate::goto::parse_reference(text).map(|(_, range)| range);
        let Some(list) = parse(&state.list).filter(|range| range.rows() > 1) else {
            state.error = Some(text::bad_list_range());
            self.dialog = Some(Dialog::AdvancedFilter(state));
            return;
        };
        let criteria = match state.criteria.trim() {
            "" => None,
            text => match parse(text) {
                Some(range) => Some(range),
                None => {
                    state.error = Some(text::bad_criteria_range());
                    self.dialog = Some(Dialog::AdvancedFilter(state));
                    return;
                }
            },
        };
        let dest = if state.copy {
            match parse(&state.dest) {
                Some(range) => Some(range.start()),
                None => {
                    state.error = Some(text::bad_destination());
                    self.dialog = Some(Dialog::AdvancedFilter(state));
                    return;
                }
            }
        } else {
            None
        };
        self.doc.advanced_filter(list, criteria, state.unique, dest);
        if dest.is_none() {
            self.advanced_list = Some((self.doc.active(), list));
        }
    }

    pub fn clear_any_filter(&mut self) {
        if self.sheet().layout().filter.is_some() {
            self.doc.clear_filters();
        } else if let Some((sheet, list)) = self.advanced_list.take()
            && sheet == self.doc.active()
        {
            self.doc.show_rows(list);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_between_locale_and_storage() {
        assert_eq!(stored_value("1,5"), "1.5");
        assert_eq!(stored_value("abc*"), "abc*");
        assert_eq!(shown_value("1.5", FilterKind::Number), "1,5");
        assert_eq!(shown_value("45366", FilterKind::Date), "15.03.2024");
        assert_eq!(stored_value("15.03.2024"), "45366");
    }
}
