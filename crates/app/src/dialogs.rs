use eframe::egui::{
    self, Align, ComboBox, Context, DragValue, FontFamily, Frame, Id, Key, Layout, Margin, Modal,
    RichText, TextEdit, Ui,
};
use vellum_grid_core::{CellAddr, CellRange, FormulaError, SortKey, column_name};

use crate::app::{Pending, Spreadsheet, locale};
use crate::cells::ShiftChoice;
use crate::format::{self, RuleKind};
use crate::goto::SpecialChoice;
use crate::insert_function::{FunctionPicker, picker_ui};
use crate::names::{self, ManagerAction, ManagerState, NameForm, NameRow};
use crate::page_setup::SetupState;
use crate::paste::{PasteOp, PasteOptions, PasteWhat};
use crate::recovery::Recovered;
use crate::{fonts, text, theme};
use vellum_grid_core::Kinds;
pub use vellum_ui::dialog::{accent_button, list_box, plain_button, roomy};
use vellum_ui::dialog::{section, title};

#[derive(Debug)]
pub enum Dialog {
    SavePrompt(Pending),
    Options(vellum_ui::settings::Prefs),
    Error {
        title: &'static str,
        message: String,
    },
    Formula(FormulaError),
    FormatCells(Box<crate::format_cells::FormatForm>),
    Size {
        columns: bool,
        value: String,
    },
    DeleteSheet(usize),
    Shortcuts,
    About,
    Zoom {
        percent: u32,
    },
    MoveCopy {
        index: usize,
        before: Option<usize>,
        copy: bool,
    },
    Unhide {
        selected: usize,
    },
    Shift {
        insert: bool,
        choice: ShiftChoice,
    },
    PasteSpecial(PasteOptions),
    PageSetup(Box<SetupState>),
    GoTo {
        reference: String,
    },
    InsertFunction(FunctionPicker),
    Spelling(Box<crate::spelling::SpellState>),
    Table(crate::tables_ui::TableForm),
    Sparkline(crate::sparklines_ui::SparkForm),
    ConditionalRule(Box<crate::conditional_ui::RuleForm>),
    ManageRules(Box<crate::conditional_ui::ManageState>),
    ConvertTable(usize),
    Protect(crate::protection_ui::ProtectForm),
    Unprotect(crate::protection_ui::UnprotectForm),
    Link(crate::links_ui::LinkForm),
    FollowFile(String),
    CustomFilter(crate::filter_ui::CustomFilterState),
    ImportText(crate::data_tools::ImportState),
    Subtotal(crate::data_tools::SubtotalState),
    GroupAxis {
        group: bool,
        rows: bool,
    },
    AdvancedFilter(crate::filter_ui::AdvancedState),
    Top10(crate::filter_ui::TopState),
    Validation(crate::validation_ui::ValidationForm),
    ValidationAlert(crate::validation_ui::ValidationAlert),
    TextToColumns(crate::data_tools::SplitState),
    RemoveDuplicates(crate::data_tools::DuplicateState),
    ExpandSelection {
        range: CellRange,
        region: CellRange,
        expand: bool,
    },
    ErrorCheck {
        at: Option<CellAddr>,
    },
    DefineName(NameForm),
    NameManager(ManagerState),
    CreateNames {
        range: CellRange,
        edges: [bool; 4],
    },
    GoToSpecial {
        choice: SpecialChoice,
        kinds: Kinds,
    },
    Recover {
        files: Vec<Recovered>,
        selected: usize,
    },
    ReplaceData {
        from: CellRange,
        to: CellAddr,
        copy: bool,
    },
    Sort {
        range: CellRange,
        header: bool,
        levels: Vec<SortKey>,
    },
    ChartTitle {
        index: usize,
        title: String,
    },
    ChartAxisTitle {
        index: usize,
        x: bool,
        title: String,
    },
    Rule {
        kind: RuleKind,
        range: CellRange,
        first: String,
        second: String,
        preset: usize,
        period: usize,
    },
}

#[derive(PartialEq, Eq)]
enum Outcome {
    Open,
    Dismiss,
    Confirm,
}

impl Spreadsheet {
    pub fn dialogs(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let mut outcome = Outcome::Open;
        let mut save = None;
        let mut special = false;
        let mut discard = false;
        let book = self.book.clone();
        let sheet_names: Vec<String> = self
            .doc
            .book()
            .sheets()
            .iter()
            .map(|sheet| sheet.name().to_owned())
            .collect();
        let visible = self.doc.visible_sheets();
        let columns = match &dialog {
            Dialog::Sort { range, .. } => self.column_labels(*range),
            _ => Vec::new(),
        };
        let name_rows: Vec<NameRow> = match &dialog {
            Dialog::NameManager(_) | Dialog::GoTo { .. } => self.name_rows(),
            _ => Vec::new(),
        };
        let mut manager = None;
        let mut check = None;
        let mut remove_link = None;
        let mut spell = None;
        let mut rules_action = None;
        let mut remove_subtotals = false;
        let mut alert_choice: Option<Option<bool>> = None;
        let current_issue = match &dialog {
            Dialog::ErrorCheck { at } => self.current_issue(*at),
            _ => None,
        };
        let frame =
            Frame::popup(&ctx.global_style()).inner_margin(Margin::same(theme::DIALOG_MARGIN));
        Modal::new(Id::unique("dialog"))
            .frame(frame)
            .show(ctx, |ui| {
                roomy(ui);
                match &mut dialog {
                    Dialog::SavePrompt(_) => {
                        title(ui, text::app_name());
                        ui.set_max_width(theme::DIALOG_WIDTH);
                        ui.label(format!("'{book}' - {}", text::save_prompt()));
                        ui.horizontal(|ui| {
                            if ui.add(accent_button(text::save())).clicked() {
                                save = Some(true);
                            }
                            if ui.add(plain_button(text::dont_save())).clicked() {
                                save = Some(false);
                            }
                            if ui.add(plain_button(text::cancel())).clicked() {
                                outcome = Outcome::Dismiss;
                            }
                        });
                    }
                    Dialog::Options(prefs) => {
                        title(ui, vellum_ui::text::options());
                        ui.set_max_width(theme::DIALOG_WIDTH);
                        let system_dark = ui.ctx().system_theme() == Some(egui::Theme::Dark);
                        vellum_ui::options::form(
                            ui,
                            prefs,
                            theme::OPTIONS_COMBO_WIDTH,
                            system_dark,
                        );
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Error {
                        title: heading,
                        message,
                    } => {
                        title(ui, heading);
                        ui.set_max_width(theme::DIALOG_WIDTH);
                        ui.label(message.as_str());
                        if ok_button(ui) {
                            outcome = Outcome::Dismiss;
                        }
                    }
                    Dialog::Formula(error) => {
                        title(ui, text::app_name());
                        ui.set_max_width(theme::DIALOG_WIDTH);
                        ui.label(formula_message(error));
                        if ok_button(ui) {
                            outcome = Outcome::Dismiss;
                        }
                    }
                    Dialog::FormatCells(form) => {
                        title(ui, text::format_cells_title());
                        crate::format_cells::dialog_ui(ui, form);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Size { columns, value } => {
                        let (heading, label) = if *columns {
                            (text::column_width_title(), text::column_width_label())
                        } else {
                            (text::row_height_title(), text::row_height_label())
                        };
                        title(ui, heading);
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        ui.horizontal(|ui| {
                            ui.label(label);
                            let input = ui.add(
                                TextEdit::singleline(value)
                                    .desired_width(theme::NUMBER_INPUT_WIDTH)
                                    .background_color(theme::input_bg()),
                            );
                            if ui.memory(|m| m.focused().is_none()) {
                                input.request_focus();
                            }
                        });
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Sort { header, levels, .. } => {
                        title(ui, text::sort());
                        sort_levels(ui, header, levels, &columns);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Rule {
                        kind,
                        first,
                        second,
                        preset,
                        period,
                        ..
                    } => {
                        title(ui, kind.label());
                        ui.set_width(theme::DIALOG_WIDTH);
                        ui.label(kind.prompt());
                        ui.horizontal(|ui| {
                            if kind.inputs() > 0 {
                                let input = ui.add(
                                    TextEdit::singleline(first)
                                        .desired_width(theme::RULE_INPUT_WIDTH),
                                );
                                if ui.memory(|m| m.focused().is_none()) {
                                    input.request_focus();
                                }
                            }
                            if kind.inputs() > 1 {
                                ui.label(text::and());
                                ui.add(
                                    TextEdit::singleline(second)
                                        .desired_width(theme::RULE_INPUT_WIDTH),
                                );
                            }
                            if *kind == RuleKind::Date {
                                ComboBox::from_id_salt("rule-period")
                                    .width(theme::RULE_INPUT_WIDTH)
                                    .selected_text(crate::conditional_ui::period_label(
                                        vellum_grid_core::DatePeriod::ALL[*period],
                                    ))
                                    .show_ui(ui, |ui| {
                                        for (index, p) in
                                            vellum_grid_core::DatePeriod::ALL.iter().enumerate()
                                        {
                                            let label = crate::conditional_ui::period_label(*p);
                                            ui.selectable_value(period, index, label);
                                        }
                                    });
                            }
                            ui.label(text::with());
                            ComboBox::from_id_salt("rule-preset")
                                .width(theme::PRESET_COMBO_WIDTH)
                                .selected_text(format::PRESETS[*preset].0())
                                .show_ui(ui, |ui| {
                                    for (index, (label, _)) in format::PRESETS.iter().enumerate() {
                                        ui.selectable_value(preset, index, label());
                                    }
                                });
                        });
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::ChartTitle { title: name, .. } => {
                        title(ui, text::chart_title());
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        ui.label(text::chart_title_label());
                        let input = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY));
                        if ui.memory(|m| m.focused().is_none()) {
                            input.request_focus();
                        }
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::ChartAxisTitle { title: name, .. } => {
                        title(ui, text::axis_title());
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        ui.label(text::chart_title_label());
                        let input = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY));
                        if ui.memory(|m| m.focused().is_none()) {
                            input.request_focus();
                        }
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::MoveCopy { before, copy, .. } => {
                        title(ui, text::move_copy_title());
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        ui.label(text::move_before());
                        list_box(ui, |ui| {
                            for &index in &visible {
                                ui.selectable_value(before, Some(index), &sheet_names[index]);
                            }
                            ui.selectable_value(before, None, text::move_to_end());
                        });
                        ui.checkbox(copy, text::make_copy());
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Unhide { selected } => {
                        title(ui, text::unhide_title());
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        ui.label(text::unhide_label());
                        list_box(ui, |ui| {
                            for (index, name) in sheet_names.iter().enumerate() {
                                if !visible.contains(&index) {
                                    ui.selectable_value(selected, index, name);
                                }
                            }
                        });
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Zoom { percent } => {
                        title(ui, text::zoom());
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        for preset in text::zoom_presets() {
                            ui.radio_value(percent, preset, text::percent_of(preset));
                        }
                        ui.horizontal(|ui| {
                            ui.label(text::zoom_custom());
                            ui.add(DragValue::new(percent).range(10..=400).prefix("%"));
                        });
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Shortcuts => {
                        title(ui, text::shortcuts());
                        ui.set_width(theme::DIALOG_WIDTH);
                        egui::Grid::new("shortcuts")
                            .num_columns(2)
                            .striped(true)
                            .spacing([theme::CHROME_SPACING * 2.0, theme::CHROME_SPACING / 2.0])
                            .show(ui, |ui| {
                                for (keys, action) in text::shortcut_list() {
                                    ui.label(RichText::new(keys).strong());
                                    ui.label(action);
                                    ui.end_row();
                                }
                            });
                        if ok_button(ui) {
                            outcome = Outcome::Dismiss;
                        }
                    }
                    Dialog::About => {
                        ui.set_width(theme::DIALOG_WIDTH);
                        ui.horizontal(|ui| {
                            if let Some(logo) = crate::logo::texture(ui.ctx()) {
                                ui.image((
                                    logo.id(),
                                    egui::vec2(theme::ABOUT_LOGO, theme::ABOUT_LOGO),
                                ));
                            }
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(text::app_name())
                                        .size(theme::ABOUT_TITLE)
                                        .family(FontFamily::Name(fonts::CHROME_BOLD.into())),
                                );
                                ui.label(format!(
                                    "{} {}",
                                    text::version(),
                                    env!("CARGO_PKG_VERSION")
                                ));
                            });
                        });
                        ui.label(text::about_text());
                        ui.label(
                            RichText::new(text::about_license()).color(theme::chrome_text_muted()),
                        );
                        if ok_button(ui) {
                            outcome = Outcome::Dismiss;
                        }
                    }
                    Dialog::Shift { insert, choice } => {
                        let heading = if *insert {
                            text::insert_title()
                        } else {
                            text::delete_title()
                        };
                        title(ui, heading);
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        ui.spacing_mut().item_spacing.y = theme::DIALOG_SPACING / 2.0;
                        for option in ShiftChoice::ALL {
                            ui.radio_value(choice, option, option.label(*insert));
                        }
                        ui.add_space(theme::DIALOG_SPACING / 2.0);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::PasteSpecial(options) => {
                        title(ui, text::paste_special_title());
                        ui.set_width(theme::DIALOG_WIDTH);
                        ui.spacing_mut().item_spacing.y = theme::DIALOG_SPACING / 2.0;
                        section(ui, text::paste_section());
                        egui::Grid::new("paste-what").num_columns(2).show(ui, |ui| {
                            for (index, what) in PasteWhat::ALL.into_iter().enumerate() {
                                ui.radio_value(&mut options.what, what, what.label());
                                if index % 2 == 1 {
                                    ui.end_row();
                                }
                            }
                        });
                        ui.add_space(theme::DIALOG_SPACING / 2.0);
                        section(ui, text::operation_section());
                        egui::Grid::new("paste-op").num_columns(2).show(ui, |ui| {
                            for (index, op) in PasteOp::ALL.into_iter().enumerate() {
                                ui.radio_value(&mut options.op, op, op.label());
                                if index % 2 == 1 {
                                    ui.end_row();
                                }
                            }
                        });
                        ui.add_space(theme::DIALOG_SPACING / 2.0);
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut options.skip_blanks, text::skip_blanks());
                            ui.checkbox(&mut options.transpose, text::transpose());
                        });
                        ui.add_space(theme::DIALOG_SPACING / 2.0);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::ReplaceData { .. } => {
                        title(ui, text::app_name());
                        ui.set_max_width(theme::DIALOG_WIDTH);
                        ui.label(text::replace_data());
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::GoTo { reference } => {
                        title(ui, text::go_to_title());
                        ui.set_width(theme::GO_TO_WIDTH);
                        ui.label(text::reference());
                        let input = ui.add(
                            TextEdit::singleline(reference)
                                .desired_width(f32::INFINITY)
                                .background_color(theme::input_bg()),
                        );
                        if !input.has_focus() && reference.is_empty() {
                            input.request_focus();
                        }
                        if !name_rows.is_empty() {
                            section(ui, text::names_in_book());
                            list_box(ui, |ui| {
                                for row in &name_rows {
                                    let chosen = *reference == row.name;
                                    let item = ui.selectable_label(chosen, &row.name);
                                    if item.clicked() {
                                        reference.clone_from(&row.name);
                                    }
                                    if item.double_clicked() {
                                        outcome = Outcome::Confirm;
                                    }
                                }
                            });
                        }
                        ui.horizontal(|ui| {
                            if ui.add(plain_button(text::special_button())).clicked() {
                                special = true;
                            }
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                if ui.add(plain_button(text::cancel())).clicked() {
                                    outcome = Outcome::Dismiss;
                                }
                                if ui.add(accent_button(text::ok())).clicked() {
                                    outcome = Outcome::Confirm;
                                }
                            });
                        });
                    }
                    Dialog::DefineName(form) => {
                        let heading = if form.replacing.is_some() {
                            text::edit_name_title()
                        } else {
                            text::new_name_title()
                        };
                        title(ui, heading);
                        names::form_ui(ui, form, &sheet_names);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::ConditionalRule(form) => {
                        title(ui, form.heading());
                        crate::conditional_ui::rule_ui(ui, form);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::ManageRules(state) => {
                        title(ui, text::manage_rules_title());
                        rules_action = crate::conditional_ui::manage_ui(ui, state);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Sparkline(form) => {
                        title(ui, text::spark_create());
                        crate::sparklines_ui::form_ui(ui, form);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Table(form) => {
                        let heading = if form.resize.is_some() {
                            text::resize_table()
                        } else {
                            text::create_table()
                        };
                        title(ui, heading);
                        crate::tables_ui::form_ui(ui, form);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::ConvertTable(_) => {
                        title(ui, text::app_name());
                        ui.set_max_width(theme::DIALOG_WIDTH);
                        ui.label(text::convert_prompt());
                        outcome = buttons(ui, text::yes());
                    }
                    Dialog::Spelling(state) => {
                        title(ui, text::spelling_title());
                        spell = crate::spelling::dialog_ui(ui, state);
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if ui.add(plain_button(text::cancel())).clicked() {
                                outcome = Outcome::Dismiss;
                            }
                        });
                    }
                    Dialog::ErrorCheck { .. } => {
                        title(ui, text::error_checking());
                        check = crate::error_check::dialog_ui(ui, current_issue);
                        if current_issue.is_some() {
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                if ui.add(plain_button(text::close())).clicked() {
                                    outcome = Outcome::Dismiss;
                                }
                            });
                        }
                    }
                    Dialog::TextToColumns(state) => {
                        title(ui, text::text_to_columns());
                        crate::data_tools::split_ui(ui, state);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::ExpandSelection { expand, .. } => {
                        title(ui, text::remove_duplicates_warning());
                        ui.set_width(theme::GO_TO_WIDTH);
                        ui.label(text::expand_prompt());
                        ui.radio_value(expand, true, text::expand_selection());
                        ui.radio_value(expand, false, text::continue_selection());
                        outcome = buttons(ui, text::remove_duplicates_button());
                    }
                    Dialog::RemoveDuplicates(state) => {
                        title(ui, text::remove_duplicates());
                        crate::data_tools::duplicates_ui(ui, state);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Validation(form) => {
                        title(ui, text::data_validation());
                        crate::validation_ui::form_ui(ui, form);
                        ui.add_space(theme::DIALOG_SPACING / 2.0);
                        ui.horizontal(|ui| {
                            if ui.add(plain_button(text::clear_all_validation())).clicked() {
                                form.rule = vellum_grid_core::validation::Validation::new(
                                    form.ranges.clone(),
                                );
                                form.first.clear();
                                form.second.clear();
                                form.error = None;
                            }
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                if ui.add(plain_button(text::cancel())).clicked() {
                                    outcome = Outcome::Dismiss;
                                }
                                if ui.add(accent_button(text::ok())).clicked() {
                                    outcome = Outcome::Confirm;
                                }
                            });
                        });
                    }
                    Dialog::ValidationAlert(alert) => {
                        let heading = if alert.rule.error_title.is_empty() {
                            text::app_name()
                        } else {
                            alert.rule.error_title.as_str()
                        };
                        title(ui, heading);
                        crate::validation_ui::alert_ui(ui, alert);
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            use vellum_grid_core::validation::AlertStyle;
                            if ui.add(plain_button(text::cancel())).clicked() {
                                alert_choice = Some(None);
                            }
                            match alert.rule.style {
                                AlertStyle::Stop => {
                                    if ui.add(accent_button(text::retry())).clicked() {
                                        alert_choice = Some(Some(false));
                                    }
                                }
                                AlertStyle::Warning => {
                                    if ui.add(plain_button(text::no())).clicked() {
                                        alert_choice = Some(Some(false));
                                    }
                                    if ui.add(accent_button(text::yes())).clicked() {
                                        alert_choice = Some(Some(true));
                                    }
                                }
                                AlertStyle::Information => {
                                    if ui.add(accent_button(text::ok())).clicked() {
                                        alert_choice = Some(Some(true));
                                    }
                                }
                            }
                        });
                    }
                    Dialog::CustomFilter(state) => {
                        title(ui, text::custom_autofilter());
                        crate::filter_ui::custom_ui(ui, state);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::AdvancedFilter(state) => {
                        title(ui, text::advanced_filter());
                        crate::filter_ui::advanced_ui(ui, state);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::GroupAxis { group, rows } => {
                        title(
                            ui,
                            if *group {
                                text::group()
                            } else {
                                text::ungroup()
                            },
                        );
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        ui.radio_value(rows, true, text::group_rows());
                        ui.radio_value(rows, false, text::group_columns());
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Subtotal(state) => {
                        title(ui, text::subtotal());
                        if crate::data_tools::subtotal_ui(ui, state) {
                            remove_subtotals = true;
                        }
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::ImportText(state) => {
                        title(ui, text::text_import());
                        crate::data_tools::import_ui(ui, state);
                        outcome = buttons(ui, text::load());
                    }
                    Dialog::Top10(state) => {
                        title(ui, text::top_ten_title());
                        crate::filter_ui::top_ui(ui, state);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Link(form) => {
                        let heading = if form.existing {
                            text::edit_link()
                        } else {
                            text::insert_link()
                        };
                        title(ui, heading);
                        if crate::links_ui::form_ui(ui, form, &sheet_names) {
                            remove_link = Some(form.addr);
                        }
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::FollowFile(target) => {
                        title(ui, text::app_name());
                        ui.set_max_width(theme::DIALOG_WIDTH);
                        ui.label(text::follow_file_warning());
                        ui.label(RichText::new(target.as_str()).strong());
                        outcome = buttons(ui, text::open_link());
                    }
                    Dialog::Protect(form) => {
                        let heading = if form.book {
                            text::protect_workbook()
                        } else {
                            text::protect_sheet()
                        };
                        title(ui, heading);
                        crate::protection_ui::protect_ui(ui, form);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Unprotect(form) => {
                        let heading = if form.book {
                            text::unprotect_workbook()
                        } else {
                            text::unprotect_sheet()
                        };
                        title(ui, heading);
                        crate::protection_ui::unprotect_ui(ui, form);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::NameManager(state) => {
                        title(ui, text::name_manager());
                        ui.set_width(theme::NAME_MANAGER_WIDTH);
                        manager = names::manager_ui(ui, state, &name_rows);
                    }
                    Dialog::CreateNames { edges, .. } => {
                        title(ui, text::create_names_title());
                        ui.set_width(theme::GO_TO_WIDTH);
                        ui.spacing_mut().item_spacing.y = theme::DIALOG_SPACING / 2.0;
                        names::create_ui(ui, edges);
                        ui.add_space(theme::DIALOG_SPACING / 2.0);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::InsertFunction(picker) => {
                        title(ui, text::insert_function());
                        ui.set_width(theme::FUNCTION_DIALOG_WIDTH);
                        if picker_ui(ui, picker) {
                            outcome = Outcome::Confirm;
                        }
                        let pressed = buttons(ui, text::ok());
                        if pressed != Outcome::Open {
                            outcome = pressed;
                        }
                    }
                    Dialog::GoToSpecial { choice, kinds } => {
                        title(ui, text::go_to_special_title());
                        ui.set_width(theme::SMALL_DIALOG_WIDTH);
                        ui.spacing_mut().item_spacing.y = theme::DIALOG_SPACING / 2.0;
                        for option in SpecialChoice::ALL {
                            ui.radio_value(choice, option, option.label());
                            if option == SpecialChoice::Formulas && choice.has_kinds() {
                                ui.horizontal(|ui| {
                                    ui.add_space(theme::DIALOG_INDENT);
                                    ui.vertical(|ui| {
                                        ui.checkbox(&mut kinds.numbers, text::kind_numbers());
                                        ui.checkbox(&mut kinds.text, text::kind_text());
                                        ui.checkbox(&mut kinds.logicals, text::kind_logicals());
                                        ui.checkbox(&mut kinds.errors, text::kind_errors());
                                    });
                                });
                            }
                        }
                        ui.add_space(theme::DIALOG_SPACING / 2.0);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::Recover { files, selected } => {
                        title(ui, text::recover_title());
                        ui.set_width(theme::DIALOG_WIDTH);
                        ui.label(text::recover_message());
                        list_box(ui, |ui| {
                            for (index, file) in files.iter().enumerate() {
                                let label = format!("{} — {}", file.name, file.saved);
                                if ui.selectable_label(*selected == index, label).clicked() {
                                    *selected = index;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.add(plain_button(text::discard_all())).clicked() {
                                discard = true;
                            }
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                if ui.add(plain_button(text::later())).clicked() {
                                    outcome = Outcome::Dismiss;
                                }
                                if ui.add(accent_button(text::open_recovered())).clicked() {
                                    outcome = Outcome::Confirm;
                                }
                            });
                        });
                    }
                    Dialog::PageSetup(state) => {
                        title(ui, text::page_setup());
                        ui.set_width(theme::SETUP_WIDTH);
                        crate::page_setup::setup_ui(ui, state);
                        ui.add_space(theme::DIALOG_SPACING);
                        outcome = buttons(ui, text::ok());
                    }
                    Dialog::DeleteSheet(_) => {
                        title(ui, text::app_name());
                        ui.set_max_width(theme::DIALOG_WIDTH);
                        ui.label(text::delete_sheet_prompt());
                        outcome = buttons(ui, text::delete_sheet());
                    }
                }
            });

        if special {
            self.open_go_to_special();
            return;
        }
        if let (Some(action), Dialog::ManageRules(_)) = (rules_action, &dialog) {
            if let Dialog::ManageRules(state) = dialog {
                self.manage_action(*state, action);
            }
            return;
        }
        if let (Some(action), Dialog::Spelling(_)) = (spell, &dialog) {
            if let Dialog::Spelling(state) = dialog {
                self.spell_action(*state, action);
            }
            return;
        }
        if let Some(addr) = remove_link {
            self.remove_link(addr);
            return;
        }
        if let (true, Dialog::Subtotal(state)) = (remove_subtotals, &dialog) {
            self.remove_all_subtotals(state);
            return;
        }
        if let (Dialog::ValidationAlert(alert), Outcome::Confirm) = (&dialog, &outcome) {
            use vellum_grid_core::validation::AlertStyle;
            alert_choice = Some(Some(alert.rule.style != AlertStyle::Stop));
        }
        if let Some(choice) = alert_choice {
            match choice {
                Some(true) => {
                    self.skip_validation = true;
                    if self.commit_edit(ctx) {
                        self.move_active(1, 0);
                    }
                    self.skip_validation = false;
                }
                Some(false) => {
                    if let Some(edit) = self.edit.as_mut() {
                        edit.focus_cell = true;
                    }
                }
                None => self.cancel_edit(ctx),
            }
            return;
        }
        if let (Some(action), Dialog::ErrorCheck { at }) = (check, &dialog) {
            if current_issue.is_some() {
                self.error_check_action(*at, action);
            }
            return;
        }
        if let Some(action) = manager {
            match action {
                ManagerAction::Close => {}
                other => self.manager_action(other),
            }
            return;
        }
        if discard {
            if let Dialog::Recover { files, .. } = &dialog {
                for file in files {
                    crate::recovery::remove(&file.path);
                }
            }
            return;
        }
        let prompt = matches!(dialog, Dialog::SavePrompt(_) | Dialog::Spelling(_));
        if outcome == Outcome::Open {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                outcome = Outcome::Dismiss;
            } else if !prompt && ctx.input(|i| i.key_pressed(Key::Enter)) {
                outcome = Outcome::Confirm;
            }
        }
        if let (Dialog::SavePrompt(action), Some(save)) = (&dialog, save) {
            if !save || self.save(false) {
                self.run(ctx, *action);
            }
            return;
        }
        if let (Outcome::Confirm | Outcome::Dismiss, Dialog::DefineName(form)) = (&outcome, &dialog)
        {
            match outcome {
                Outcome::Confirm => self.submit_name(form.clone()),
                _ if form.back => {
                    self.dialog = Some(Dialog::NameManager(ManagerState {
                        selected: form.replacing,
                    }));
                }
                _ => {}
            }
            return;
        }
        if let (Outcome::Confirm, Dialog::InsertFunction(picker)) = (&outcome, &dialog) {
            match picker.selected {
                Some(name) => self.insert_function(ctx, name),
                None => self.dialog = Some(dialog),
            }
            return;
        }
        match (outcome, dialog) {
            (Outcome::Open, dialog) => self.dialog = Some(dialog),
            (Outcome::Dismiss, Dialog::ConditionalRule(form)) => self.close_rule_form(*form),
            (Outcome::Dismiss, _) => {}
            (Outcome::Confirm, dialog) => self.confirm(dialog),
        }
    }

    fn confirm(&mut self, dialog: Dialog) {
        match dialog {
            Dialog::Options(prefs) => {
                prefs.save();
                self.prefs = prefs;
            }
            Dialog::FormatCells(form) => self.apply_format_form(&form),
            Dialog::Table(form) => self.apply_table_form(form),
            Dialog::Sparkline(form) => self.apply_sparklines(form),
            Dialog::ConditionalRule(form) => self.apply_rule_form(*form),
            Dialog::ManageRules(state) => self.apply_manage(*state),
            Dialog::ConvertTable(index) => {
                self.doc.convert_table(index);
            }
            Dialog::Size { columns, value } => {
                let Some(number) = locale().parse_number(value.trim()) else {
                    self.dialog = Some(Dialog::Size { columns, value });
                    return;
                };
                if columns {
                    let width = format::width_from_chars(
                        number.clamp(0.0, theme::MAX_COLUMN_CHARS),
                        self.digit,
                    );
                    let cols = self.selected_cols();
                    self.doc.set_layout(|layout| {
                        for col in cols {
                            layout.col_widths.insert(col, width);
                        }
                    });
                } else {
                    let points = number.clamp(0.0, theme::MAX_ROW_POINTS);
                    let rows = self.selected_rows();
                    self.doc.set_layout(|layout| {
                        for row in rows {
                            layout.row_heights.insert(row, points);
                        }
                    });
                }
            }
            Dialog::DeleteSheet(index) => self.delete_sheet_now(index),
            Dialog::Zoom { percent } => self.set_zoom(percent as f32 / 100.0),
            Dialog::MoveCopy {
                index,
                before,
                copy,
            } => self.move_or_copy(index, before, copy),
            Dialog::Unhide { selected } => self.unhide_sheet(selected),
            Dialog::Shift { insert, choice } => self.shift_selection(choice, insert),
            Dialog::PasteSpecial(options) => self.paste_special(options),
            Dialog::PageSetup(state) => self.finish_page_setup(*state),
            Dialog::ReplaceData { from, to, copy } => self.move_now(from, to, copy),
            Dialog::GoTo { reference } => {
                if !self.go_to(&reference) {
                    self.dialog = Some(Dialog::Error {
                        title: text::app_name(),
                        message: text::invalid_go_to().to_owned(),
                    });
                }
            }
            Dialog::GoToSpecial { choice, kinds } => self.go_to_special(choice, kinds),
            Dialog::CreateNames { range, edges } => self.create_names(range, edges),
            Dialog::Protect(form) => self.apply_protect(form),
            Dialog::Unprotect(form) => self.apply_unprotect(form),
            Dialog::Link(form) => self.apply_link(form),
            Dialog::FollowFile(target) => crate::links_ui::open_external(&target),
            Dialog::CustomFilter(state) => self.apply_custom_filter(state),
            Dialog::Top10(state) => self.apply_top(state),
            Dialog::ImportText(state) => self.apply_text_import(state),
            Dialog::Subtotal(state) => self.apply_subtotal(state),
            Dialog::GroupAxis { group, rows } => self.group_selection(group, Some(rows)),
            Dialog::AdvancedFilter(state) => self.apply_advanced_filter(state),
            Dialog::Validation(form) => self.apply_validation(form),
            Dialog::TextToColumns(state) => self.apply_text_to_columns(state),
            Dialog::RemoveDuplicates(state) => self.apply_remove_duplicates(state),
            Dialog::ExpandSelection {
                range,
                region,
                expand,
            } => self.remove_duplicates_in(if expand { region } else { range }),
            Dialog::Recover { files, selected } => {
                if let Some(file) = files.get(selected) {
                    self.open_recovered(file);
                }
            }
            Dialog::ChartAxisTitle { index, x, title } => {
                let title = (!title.trim().is_empty()).then(|| title.trim().to_owned());
                self.doc.set_layout(|layout| {
                    if let Some(chart) = layout.charts.get_mut(index) {
                        if x {
                            chart.x_title = title;
                        } else {
                            chart.y_title = title;
                        }
                    }
                });
            }
            Dialog::ChartTitle { index, title } => {
                let title = (!title.trim().is_empty()).then(|| title.trim().to_owned());
                self.doc.set_layout(|layout| {
                    if let Some(chart) = layout.charts.get_mut(index) {
                        chart.title = title;
                    }
                });
            }
            Dialog::Rule {
                kind,
                range,
                first,
                second,
                preset,
                period,
            } => match kind.rule(&first, &second, period, locale()) {
                Some(rule) => {
                    let format = vellum_grid_core::ConditionalFormat::new(
                        range,
                        rule,
                        format::PRESETS[preset].1,
                    );
                    self.doc
                        .set_layout(|layout| layout.conditional.push(format));
                }
                None => {
                    self.dialog = Some(Dialog::Rule {
                        kind,
                        range,
                        first,
                        second,
                        preset,
                        period,
                    });
                }
            },
            Dialog::Sort {
                range,
                header,
                levels,
            } => self.apply_sort(range, &levels, header),
            _ => {}
        }
    }
}

impl Spreadsheet {
    pub fn column_labels(&self, range: CellRange) -> Vec<(u32, String, String)> {
        (range.start().col()..=range.end().col())
            .map(|col| {
                let plain = format!("{} {}", text::column(), column_name(col));
                let at = CellAddr::new(range.start().row(), col).expect("inside the range");
                let heading = self.doc.filter_text(at);
                let named = if heading.is_empty() {
                    format!("({plain})")
                } else {
                    heading
                };
                (col, plain, named)
            })
            .collect()
    }
}

fn sort_levels(
    ui: &mut Ui,
    header: &mut bool,
    levels: &mut Vec<SortKey>,
    columns: &[(u32, String, String)],
) {
    ui.set_width(theme::DIALOG_WIDTH);
    ui.horizontal(|ui| {
        if ui.button(format!("+ {}", text::add_level())).clicked()
            && let Some((col, ..)) = columns.first()
        {
            levels.push(SortKey {
                col: *col,
                descending: false,
            });
        }
        if ui
            .add_enabled(levels.len() > 1, egui::Button::new(text::delete_level()))
            .clicked()
        {
            levels.pop();
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.checkbox(header, text::has_header());
        });
    });
    let name = |col: u32| {
        columns
            .iter()
            .find(|(c, ..)| *c == col)
            .map_or_else(String::new, |(_, plain, named)| {
                if *header {
                    named.clone()
                } else {
                    plain.clone()
                }
            })
    };
    egui::Grid::new("sort-levels")
        .num_columns(3)
        .spacing([theme::CHROME_SPACING, theme::CHROME_SPACING])
        .show(ui, |ui| {
            ui.label("");
            ui.label(text::column());
            ui.label(text::order());
            ui.end_row();
            for (index, level) in levels.iter_mut().enumerate() {
                ui.label(if index == 0 {
                    text::sort_by()
                } else {
                    text::then_by()
                });
                ComboBox::from_id_salt(("sort-column", index))
                    .width(theme::SORT_COMBO_WIDTH)
                    .selected_text(name(level.col))
                    .show_ui(ui, |ui| {
                        for (col, ..) in columns {
                            ui.selectable_value(&mut level.col, *col, name(*col));
                        }
                    });
                let order = if level.descending {
                    text::z_to_a()
                } else {
                    text::a_to_z()
                };
                ComboBox::from_id_salt(("sort-order", index))
                    .width(theme::SORT_COMBO_WIDTH)
                    .selected_text(order)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut level.descending, false, text::a_to_z());
                        ui.selectable_value(&mut level.descending, true, text::z_to_a());
                    });
                ui.end_row();
            }
        });
}

fn ok_button(ui: &mut Ui) -> bool {
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        ui.add(accent_button(text::ok())).clicked()
    })
    .inner
}

fn buttons(ui: &mut Ui, confirm: &str) -> Outcome {
    let mut outcome = Outcome::Open;
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        if ui.add(plain_button(text::cancel())).clicked() {
            outcome = Outcome::Dismiss;
        }
        if ui.add(accent_button(confirm)).clicked() {
            outcome = Outcome::Confirm;
        }
    });
    outcome
}

fn formula_message(error: &FormulaError) -> String {
    match error {
        FormulaError::TooFewArguments(name) => format!("{name}: {}", text::too_few_arguments()),
        FormulaError::TooManyArguments(name) => format!("{name}: {}", text::too_many_arguments()),
        FormulaError::Syntax | FormulaError::UnterminatedText => text::formula_problem().to_owned(),
    }
}
