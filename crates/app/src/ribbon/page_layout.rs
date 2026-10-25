use eframe::egui::{Popup, Ui};
use vellum_grid_core::{Margins, Paper, Scale};

use super::widgets::{big, chrome_text, group, launcher_group, small_menu, soon};
use crate::app::Spreadsheet;
use crate::icons::Icon;
use crate::page_setup::{self, FIT_PAGES, SCALES, SetupTab};
use crate::{fluent, menu, text, theme};

impl Spreadsheet {
    pub(super) fn page_layout(&mut self, ui: &mut Ui) {
        group(ui, text::themes(), |ui| {
            soon(ui, |ui| {
                big(
                    ui,
                    Icon::Tinted(fluent::COLOR, theme::tint_orange()),
                    text::themes(),
                    text::themes(),
                    true,
                );
            });
        });
        let launch = launcher_group(ui, text::page_setup(), text::page_setup(), |ui| {
            ui.horizontal_top(|ui| self.page_setup_tools(ui));
        });
        if launch {
            self.open_page_setup(SetupTab::Page);
        }
        let launch = launcher_group(ui, text::scale_to_fit(), text::page_setup(), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            self.scale_tools(ui);
        });
        if launch {
            self.open_page_setup(SetupTab::Page);
        }
        group(ui, text::sheet_options(), |ui| {
            ui.add_space(theme::RIBBON_ROW_GAP);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0 * theme::CHROME_SPACING;
                ui.vertical(|ui| {
                    ui.label(chrome_text(text::gridlines()));
                    self.gridlines_checkbox(ui, text::view_option());
                    let mut print = self.page_setup().gridlines;
                    if ui
                        .checkbox(&mut print, chrome_text(text::print_option()))
                        .changed()
                    {
                        self.set_page(|page| page.gridlines = print);
                    }
                });
                ui.vertical(|ui| {
                    ui.label(chrome_text(text::headings()));
                    self.headings_checkbox(ui, text::view_option());
                    let mut print = self.page_setup().headings;
                    if ui
                        .checkbox(&mut print, chrome_text(text::print_option()))
                        .changed()
                    {
                        self.set_page(|page| page.headings = print);
                    }
                });
            });
        });
    }

    fn page_setup_tools(&mut self, ui: &mut Ui) {
        let margins = big(
            ui,
            Icon::Plain(fluent::DOCUMENT_MARGINS),
            text::margins(),
            text::margins(),
            true,
        );
        Popup::menu(&margins)
            .width(theme::WIDE_MENU_WIDTH)
            .show(|ui| {
                let current = self.page_setup().margins;
                for (name, preset) in [
                    (text::margins_normal(), Margins::NORMAL),
                    (text::margins_wide(), Margins::WIDE),
                    (text::margins_narrow(), Margins::NARROW),
                ] {
                    let icon = (current == preset).then_some(Icon::Plain(fluent::CHECKMARK));
                    let detail = page_setup::margins_label(preset);
                    if menu::detail_item(ui, icon, name, &detail) {
                        self.set_page(|page| page.margins = preset);
                    }
                }
                menu::separator(ui);
                if menu::item(ui, text::custom_margins()) {
                    self.open_page_setup(SetupTab::Margins);
                }
            });
        let orientation = big(
            ui,
            Icon::Plain(fluent::ORIENTATION),
            text::orientation_page(),
            text::orientation_page(),
            true,
        );
        Popup::menu(&orientation)
            .width(theme::MENU_WIDTH)
            .show(|ui| {
                let landscape = self.page_setup().landscape;
                for (value, label) in [(false, text::portrait()), (true, text::landscape())] {
                    let icon = (landscape == value).then_some(Icon::Plain(fluent::CHECKMARK));
                    if menu::icon_or_blank(ui, icon, label) {
                        self.set_page(|page| page.landscape = value);
                    }
                }
            });
        let size = big(
            ui,
            Icon::Plain(fluent::DOCUMENT),
            text::paper_size(),
            text::paper_size(),
            true,
        );
        Popup::menu(&size).width(theme::WIDE_MENU_WIDTH).show(|ui| {
            let paper = self.page_setup().paper;
            for option in Paper::ALL {
                let icon = (paper == option).then_some(Icon::Plain(fluent::CHECKMARK));
                if menu::icon_or_blank(ui, icon, &page_setup::paper_label(option)) {
                    self.set_page(|page| page.paper = option);
                }
            }
            menu::separator(ui);
            if menu::item(ui, text::more_paper()) {
                self.open_page_setup(SetupTab::Page);
            }
        });
        let area = big(
            ui,
            Icon::Plain(fluent::PRINT),
            text::print_area(),
            text::print_area(),
            true,
        );
        Popup::menu(&area).width(theme::MENU_WIDTH).show(|ui| {
            if menu::item(ui, text::set_print_area()) {
                self.set_print_area();
            }
            let has = self.page_setup().print_area.is_some();
            if menu::enabled_item(ui, has, None, text::clear_print_area()) {
                self.set_page(|page| page.print_area = None);
            }
        });
        let breaks = big(
            ui,
            Icon::Plain(fluent::DOCUMENT_PAGE_BREAK),
            text::breaks(),
            text::breaks(),
            true,
        );
        Popup::menu(&breaks).width(theme::MENU_WIDTH).show(|ui| {
            if menu::item(ui, text::insert_break()) {
                self.insert_page_break();
            }
            if menu::item(ui, text::remove_break()) {
                self.remove_page_break();
            }
            if menu::item(ui, text::reset_breaks()) {
                self.set_page(|page| {
                    page.row_breaks.clear();
                    page.col_breaks.clear();
                });
            }
        });
        soon(ui, |ui| {
            big(
                ui,
                Icon::Plain(fluent::IMAGE),
                text::background(),
                text::background(),
                false,
            );
        });
        let titles = big(
            ui,
            Icon::Plain(fluent::DOCUMENT_TABLE),
            text::print_titles(),
            text::print_titles(),
            false,
        );
        if titles.clicked() {
            self.open_page_setup(SetupTab::Sheet);
        }
    }

    fn scale_tools(&mut self, ui: &mut Ui) {
        let (width, height, percent) = match self.page_setup().scale {
            Scale::Fit { width, height } => (width, height, None),
            Scale::Percent(percent) => (0, 0, Some(percent)),
        };
        for (glyph, label, current, wide) in [
            (fluent::TABLE_RESIZE_COLUMN, text::fit_width(), width, true),
            (fluent::TABLE_RESIZE_ROW, text::fit_height(), height, false),
        ] {
            let caption = format!("{label}: {}", page_setup::fit_label(current));
            let button = small_menu(ui, Icon::Plain(glyph), &caption);
            Popup::menu(&button).width(theme::MENU_WIDTH).show(|ui| {
                for pages in FIT_PAGES {
                    let icon = (pages == current).then_some(Icon::Plain(fluent::CHECKMARK));
                    if menu::icon_or_blank(ui, icon, &page_setup::fit_label(pages)) {
                        self.set_page(|page| {
                            let (mut w, mut h) = match page.scale {
                                Scale::Fit { width, height } => (width, height),
                                Scale::Percent(_) => (0, 0),
                            };
                            if wide {
                                w = pages;
                            } else {
                                h = pages;
                            }
                            page.scale = if w == 0 && h == 0 {
                                Scale::Percent(100)
                            } else {
                                Scale::Fit {
                                    width: w,
                                    height: h,
                                }
                            };
                        });
                    }
                }
            });
        }
        let caption = match percent {
            Some(p) => format!("{}: %{p}", text::scale()),
            None => format!("{}: —", text::scale()),
        };
        let button = small_menu(ui, Icon::Plain(fluent::PAGE_FIT), &caption);
        Popup::menu(&button).width(theme::MENU_WIDTH).show(|ui| {
            for value in SCALES {
                let icon = (percent == Some(value)).then_some(Icon::Plain(fluent::CHECKMARK));
                if menu::icon_or_blank(ui, icon, &text::percent_of(value)) {
                    self.set_page(|page| page.scale = Scale::Percent(value));
                }
            }
            menu::separator(ui);
            if menu::item(ui, text::custom_scale()) {
                self.open_page_setup(SetupTab::Page);
            }
        });
    }
}
