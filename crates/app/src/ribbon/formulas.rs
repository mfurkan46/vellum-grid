use eframe::egui::{Popup, ScrollArea, Ui};

use super::widgets::{big, group, small, small_drop, small_menu, soon, tip};
use crate::app::{Spreadsheet, locale};
use crate::function_help::Category;
use crate::icons::Icon;
use crate::insert_function::sorted;
use crate::{fluent, menu, text, theme};

const MENUS: [(text::Label, &[Category]); 6] = [
    (text::logical, &[Category::Logical]),
    (text::text_functions, &[Category::Text]),
    (text::date_time, &[Category::DateTime]),
    (text::lookup, &[Category::Lookup]),
    (text::math, &[Category::Math]),
    (
        text::more_functions,
        &[Category::Statistical, Category::Information],
    ),
];

fn function_list(ui: &mut Ui, category: Category) -> Option<&'static str> {
    let mut chosen = None;
    ScrollArea::vertical()
        .max_height(theme::FUNCTION_MENU_HEIGHT)
        .show(ui, |ui| {
            for help in sorted(Some(category)) {
                if menu::hint_item(ui, locale().function_name(help.name), help.description()) {
                    chosen = Some(help.name);
                }
            }
        });
    chosen
}

impl Spreadsheet {
    pub(super) fn formulas(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        group(ui, text::function_library(), |ui| {
            ui.horizontal_top(|ui| {
                let insert = tip(text::insert_function(), "Shift+F3");
                if big(ui, Icon::Function, text::insert_function(), &insert, false).clicked() {
                    self.open_insert_function();
                }
                let autosum = big(
                    ui,
                    Icon::AutoSum,
                    text::autosum(),
                    &tip(text::autosum(), "Alt+="),
                    true,
                );
                if autosum.clicked() {
                    self.auto_sum(&ctx, "SUM");
                }
                for column in MENUS.chunks(3) {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (label, categories) in column {
                            let button = small_drop(ui, label());
                            Popup::menu(&button).show(|ui| {
                                let chosen = match categories {
                                    [category] => function_list(ui, *category),
                                    _ => {
                                        let mut chosen = None;
                                        for category in *categories {
                                            menu::sub(ui, None, category.label(), |ui| {
                                                chosen = chosen.or(function_list(ui, *category));
                                            });
                                        }
                                        chosen
                                    }
                                };
                                if let Some(name) = chosen {
                                    self.insert_function(&ctx, name);
                                }
                                menu::separator(ui);
                                let more = format!("{}…", text::insert_function());
                                if menu::shortcut_item(ui, Some(Icon::Function), &more, "Shift+F3")
                                {
                                    self.open_insert_function();
                                }
                            });
                        }
                    });
                }
            });
        });
        group(ui, text::defined_names(), |ui| {
            ui.horizontal_top(|ui| {
                let manager = tip(text::name_manager(), "Ctrl+F3");
                let tag = Icon::Plain(fluent::TAG);
                if big(ui, tag, text::name_manager(), &manager, false).clicked() {
                    self.open_name_manager();
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if small(ui, tag, text::define_name(), text::define_name(), false).clicked() {
                        self.open_define_name(false);
                    }
                    let names: Vec<String> = self
                        .doc
                        .book()
                        .names()
                        .iter()
                        .filter(|defined| {
                            defined.scope.is_none_or(|scope| scope == self.doc.active())
                        })
                        .map(|defined| defined.name.clone())
                        .collect();
                    let button = small_drop(ui, text::use_in_formula());
                    Popup::menu(&button).show(|ui| {
                        if names.is_empty() {
                            menu::soon(ui, None, text::no_names());
                        }
                        for name in &names {
                            if menu::icon_item(ui, tag, name) {
                                self.insert_name(&ctx, name);
                            }
                        }
                    });
                    let create = tip(text::create_from_selection(), "Ctrl+Shift+F3");
                    let table = Icon::Plain(fluent::TABLE_EDIT);
                    if small(ui, table, text::create_from_selection(), &create, false).clicked() {
                        self.open_create_names();
                    }
                });
            });
        });
        group(ui, text::formula_auditing(), |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let precedents = Icon::Plain(fluent::ARROW_EXPORT);
                    let label = text::trace_precedents();
                    if small(ui, precedents, label, label, false).clicked() {
                        self.trace(true);
                    }
                    let dependents = Icon::Plain(fluent::ARROW_IMPORT);
                    let label = text::trace_dependents();
                    if small(ui, dependents, label, label, false).clicked() {
                        self.trace(false);
                    }
                    let remove =
                        small_menu(ui, Icon::Plain(fluent::DISMISS), text::remove_arrows());
                    Popup::menu(&remove).show(|ui| {
                        if menu::item(ui, text::remove_arrows()) {
                            self.remove_arrows(None);
                        }
                        if menu::item(ui, text::remove_precedent_arrows()) {
                            self.remove_arrows(Some(true));
                        }
                        if menu::item(ui, text::remove_dependent_arrows()) {
                            self.remove_arrows(Some(false));
                        }
                    });
                });
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let label = text::show_formulas();
                    let hint = tip(label, "Ctrl+`");
                    if small(ui, Icon::Function, label, &hint, self.show_formulas).clicked() {
                        self.show_formulas = !self.show_formulas;
                    }
                    let check =
                        small_menu(ui, Icon::Plain(fluent::WARNING), text::error_checking());
                    Popup::menu(&check).show(|ui| {
                        if menu::item(ui, text::error_check_menu()) {
                            self.open_error_check();
                        }
                        if menu::item(ui, text::trace_error()) {
                            self.arrows.clear();
                            self.trace(true);
                        }
                    });
                    soon(ui, |ui| {
                        small(
                            ui,
                            Icon::Plain(fluent::MATH_FORMULA),
                            text::evaluate_formula(),
                            text::evaluate_formula(),
                            false,
                        );
                    });
                });
            });
        });
        group(ui, text::calculation(), |ui| {
            ui.horizontal_top(|ui| {
                let options = big(
                    ui,
                    Icon::Plain(fluent::CALCULATOR),
                    text::calculation_options(),
                    text::calculation_options(),
                    true,
                );
                Popup::menu(&options).show(|ui| {
                    let manual = self.doc.is_manual();
                    let mark = |on: bool| on.then_some(Icon::Plain(fluent::CHECKMARK));
                    if menu::icon_or_blank(ui, mark(!manual), text::calc_automatic()) {
                        self.doc.set_manual(false);
                    }
                    if menu::icon_or_blank(ui, mark(manual), text::calc_manual()) {
                        self.doc.set_manual(true);
                    }
                });
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let now = Icon::Plain(fluent::CALCULATOR);
                    if small(
                        ui,
                        now,
                        text::calculate_now(),
                        &tip(text::calculate_now(), "F9"),
                        false,
                    )
                    .clicked()
                    {
                        self.calculate();
                    }
                    let sheet = Icon::Plain(fluent::DOCUMENT_TABLE);
                    let hint = tip(text::calculate_sheet(), "Shift+F9");
                    if small(ui, sheet, text::calculate_sheet(), &hint, false).clicked() {
                        self.calculate();
                    }
                });
            });
        });
    }
}
