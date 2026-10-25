use eframe::egui::{self, ComboBox, DragValue, RichText, TextEdit, Ui};
use vellum_grid_core::page::{join_sections, parse_span, span_text, split_sections};
use vellum_grid_core::{CellAddr, Margins, PageSetup, Paper, Scale};

use crate::app::{Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::{text, theme};

pub const CM_PER_INCH: f64 = 2.54;
pub const FIT_PAGES: [u16; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
pub const SCALES: [u16; 8] = [25, 50, 75, 85, 100, 125, 150, 200];
const MIN_PERCENT: u16 = 10;
const MAX_PERCENT: u16 = 400;
const MAX_MARGIN_CM: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupTab {
    Page,
    Margins,
    HeaderFooter,
    Sheet,
}

impl SetupTab {
    const ALL: [Self; 4] = [Self::Page, Self::Margins, Self::HeaderFooter, Self::Sheet];

    fn label(self) -> &'static str {
        match self {
            Self::Page => text::tab_page(),
            Self::Margins => text::margins(),
            Self::HeaderFooter => text::header_footer_tab(),
            Self::Sheet => text::tab_sheet(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetupState {
    pub setup: PageSetup,
    pub tab: SetupTab,
    pub margins: [f64; 6],
    pub header: [String; 3],
    pub footer: [String; 3],
    pub area: String,
    pub rows: String,
    pub cols: String,
}

impl SetupState {
    pub fn new(setup: PageSetup, tab: SetupTab) -> Self {
        let m = setup.margins;
        let cm = |inches: f64| (inches * CM_PER_INCH * 100.0).round() / 100.0;
        Self {
            margins: [
                cm(m.top),
                cm(m.bottom),
                cm(m.left),
                cm(m.right),
                cm(m.header),
                cm(m.footer),
            ],
            header: split_sections(&setup.header),
            footer: split_sections(&setup.footer),
            area: setup.print_area.map(|a| a.to_string()).unwrap_or_default(),
            rows: setup
                .title_rows
                .map(|s| span_text(s, true))
                .unwrap_or_default(),
            cols: setup
                .title_cols
                .map(|s| span_text(s, false))
                .unwrap_or_default(),
            setup,
            tab,
        }
    }

    pub fn finish(mut self) -> Option<PageSetup> {
        let inches = |cm: f64| cm / CM_PER_INCH;
        let [top, bottom, left, right, header, footer] = self.margins.map(inches);
        self.setup.margins = Margins {
            left,
            right,
            top,
            bottom,
            header,
            footer,
        };
        self.setup.header = join_sections(&self.header);
        self.setup.footer = join_sections(&self.footer);
        let area = self.area.replace('$', "");
        let area = area
            .rsplit_once('!')
            .map_or(area.as_str(), |(_, a)| a)
            .trim();
        self.setup.print_area = if area.is_empty() {
            None
        } else {
            Some(area.to_uppercase().parse().ok()?)
        };
        self.setup.title_rows = optional_span(&self.rows, true)?;
        self.setup.title_cols = optional_span(&self.cols, false)?;
        Some(self.setup)
    }
}

fn optional_span(text: &str, rows: bool) -> Option<Option<(u32, u32)>> {
    if text.trim().is_empty() {
        Some(None)
    } else {
        parse_span(text, rows).map(Some)
    }
}

pub fn paper_label(paper: Paper) -> String {
    let (w, h) = paper.size();
    let cm =
        |points: f64| locale().format_number((points / 72.0 * CM_PER_INCH * 10.0).round() / 10.0);
    let name = match paper {
        Paper::A4 => "A4",
        Paper::A3 => "A3",
        Paper::A5 => "A5",
        Paper::Letter => "Letter",
        Paper::Legal => "Legal",
    };
    format!("{name}  {} cm x {} cm", cm(w), cm(h))
}

pub fn paper_parts(paper: Paper) -> (String, String) {
    let label = paper_label(paper);
    match label.split_once("  ") {
        Some((name, size)) => (name.to_owned(), size.to_owned()),
        None => (label, String::new()),
    }
}

pub fn margins_label(margins: Margins) -> String {
    let cm = |inches: f64| locale().format_number((inches * CM_PER_INCH * 100.0).round() / 100.0);
    format!(
        "{} {} cm  {} {} cm  {} {} cm  {} {} cm",
        text::top(),
        cm(margins.top),
        text::bottom(),
        cm(margins.bottom),
        text::left(),
        cm(margins.left),
        text::right(),
        cm(margins.right)
    )
}

pub fn fit_label(pages: u16) -> String {
    if pages == 0 {
        text::automatic_pages().to_owned()
    } else {
        format!("{pages} {}", text::page_unit())
    }
}

pub fn setup_ui(ui: &mut Ui, state: &mut SetupState) {
    ui.horizontal(|ui| {
        for tab in SetupTab::ALL {
            if ui.selectable_label(state.tab == tab, tab.label()).clicked() {
                state.tab = tab;
            }
        }
    });
    ui.separator();
    ui.set_min_height(theme::SETUP_HEIGHT);
    match state.tab {
        SetupTab::Page => page_tab(ui, &mut state.setup),
        SetupTab::Margins => margins_tab(ui, state),
        SetupTab::HeaderFooter => header_footer_tab(ui, state),
        SetupTab::Sheet => sheet_tab(ui, state),
    }
}

fn page_tab(ui: &mut Ui, setup: &mut PageSetup) {
    section(ui, text::orientation_page());
    ui.horizontal(|ui| {
        ui.radio_value(&mut setup.landscape, false, text::portrait());
        ui.radio_value(&mut setup.landscape, true, text::landscape());
    });
    section(ui, text::scaling());
    let mut fit = matches!(setup.scale, Scale::Fit { .. });
    ui.horizontal(|ui| {
        if ui.radio_value(&mut fit, false, text::adjust_to()).changed() {
            setup.scale = Scale::Percent(100);
        }
        if let Scale::Percent(percent) = &mut setup.scale {
            ui.add(
                DragValue::new(percent)
                    .range(MIN_PERCENT..=MAX_PERCENT)
                    .suffix(" %"),
            );
        }
    });
    ui.horizontal(|ui| {
        if ui.radio_value(&mut fit, true, text::fit_to()).changed() {
            setup.scale = Scale::Fit {
                width: 1,
                height: 1,
            };
        }
        if let Scale::Fit { width, height } = &mut setup.scale {
            ui.add(DragValue::new(width).range(0..=99));
            ui.label(text::pages_wide_by());
            ui.add(DragValue::new(height).range(0..=99));
            ui.label(text::pages_tall());
        }
    });
    section(ui, text::paper_size());
    ComboBox::from_id_salt("paper")
        .width(theme::PRESET_COMBO_WIDTH)
        .selected_text(paper_label(setup.paper))
        .show_ui(ui, |ui| {
            for paper in Paper::ALL {
                ui.selectable_value(&mut setup.paper, paper, paper_label(paper));
            }
        });
}

fn margins_tab(ui: &mut Ui, state: &mut SetupState) {
    let labels = [
        text::top(),
        text::bottom(),
        text::left(),
        text::right(),
        text::header(),
        text::footer(),
    ];
    egui::Grid::new("margins")
        .num_columns(4)
        .spacing([theme::DIALOG_SPACING * 2.0, theme::DIALOG_SPACING])
        .show(ui, |ui| {
            for (index, label) in labels.iter().enumerate() {
                ui.label(*label);
                ui.add(
                    DragValue::new(&mut state.margins[index])
                        .range(0.0..=MAX_MARGIN_CM)
                        .speed(0.05)
                        .max_decimals(2)
                        .suffix(" cm"),
                );
                if index % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    section(ui, text::center_on_page());
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.setup.center_horizontally, text::horizontally());
        ui.checkbox(&mut state.setup.center_vertically, text::vertically());
    });
}

fn header_footer_tab(ui: &mut Ui, state: &mut SetupState) {
    for (label, parts) in [
        (text::header(), &mut state.header),
        (text::footer(), &mut state.footer),
    ] {
        ui.horizontal(|ui| {
            section(ui, label);
            ComboBox::from_id_salt(label)
                .width(theme::PRESET_COMBO_WIDTH)
                .selected_text(text::presets())
                .show_ui(ui, |ui| {
                    for (name, code) in text::header_presets() {
                        if ui.selectable_label(false, *name).clicked() {
                            *parts = split_sections(code);
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            for (index, side) in [text::left(), text::center(), text::right()]
                .into_iter()
                .enumerate()
            {
                ui.vertical(|ui| {
                    ui.label(RichText::new(side).color(theme::chrome_text_muted()));
                    ui.add(
                        TextEdit::singleline(&mut parts[index])
                            .desired_width(theme::HEADER_FIELD_WIDTH)
                            .background_color(theme::input_bg()),
                    );
                });
            }
        });
    }
    ui.label(RichText::new(text::header_codes()).color(theme::chrome_text_muted()));
}

fn sheet_tab(ui: &mut Ui, state: &mut SetupState) {
    egui::Grid::new("sheet-print")
        .num_columns(2)
        .spacing([theme::DIALOG_SPACING, theme::DIALOG_SPACING])
        .show(ui, |ui| {
            for (label, value, hint) in [
                (text::print_area_field(), &mut state.area, "A1:F40"),
                (text::rows_to_repeat(), &mut state.rows, "$1:$1"),
                (text::cols_to_repeat(), &mut state.cols, "$A:$A"),
            ] {
                ui.label(label);
                ui.add(
                    TextEdit::singleline(value)
                        .hint_text(hint)
                        .desired_width(theme::FIND_INPUT_WIDTH)
                        .background_color(theme::input_bg()),
                );
                ui.end_row();
            }
        });
    section(ui, text::print_option());
    ui.checkbox(&mut state.setup.gridlines, text::gridlines());
    ui.checkbox(&mut state.setup.headings, text::row_col_headings());
}

fn section(ui: &mut Ui, label: &str) {
    ui.label(RichText::new(label).color(theme::chrome_text_muted()));
}

impl Spreadsheet {
    pub fn page_setup(&self) -> &PageSetup {
        &self.sheet().layout().page
    }

    pub fn set_page(&mut self, f: impl FnOnce(&mut PageSetup)) {
        self.doc.set_layout(|layout| f(&mut layout.page));
    }

    pub fn open_page_setup(&mut self, tab: SetupTab) {
        let state = SetupState::new(self.page_setup().clone(), tab);
        self.dialog = Some(Dialog::PageSetup(Box::new(state)));
    }

    pub fn finish_page_setup(&mut self, state: SetupState) {
        match state.clone().finish() {
            Some(setup) => self.set_page(|page| *page = setup),
            None => {
                self.dialog = Some(Dialog::PageSetup(Box::new(state)));
            }
        }
    }

    pub fn set_print_area(&mut self) {
        let range = self.selected();
        self.set_page(|page| page.print_area = Some(range));
    }

    pub fn insert_page_break(&mut self) {
        let at: CellAddr = self.sel.active;
        self.set_page(|page| {
            if at.row() > 0 {
                page.row_breaks.insert(at.row());
            }
            if at.col() > 0 {
                page.col_breaks.insert(at.col());
            }
        });
    }

    pub fn remove_page_break(&mut self) {
        let at = self.sel.active;
        self.set_page(|page| {
            page.row_breaks.remove(&at.row());
            page.col_breaks.remove(&at.col());
        });
    }
}
