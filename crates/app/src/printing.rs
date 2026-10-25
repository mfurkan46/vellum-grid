use std::path::{Path, PathBuf};

use eframe::egui::{
    Align, Align2, CornerRadius, DragValue, FontId, Frame, Layout, Margin, Popup, Pos2, Rect,
    Response, RichText, Sense, Shadow, Stroke, StrokeKind, Ui, pos2, vec2,
};
use vellum_grid_core::{Margins, Paper, Scale};
use vellum_grid_io::pdf::write_pdf;
use vellum_grid_io::print::{FontBook, Item, Job, PrintPage, layout};

use crate::app::{Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::fonts::{SystemFonts, family_for};
use crate::icons::{self, Icon};
use crate::page_setup::{self, SetupTab};
use crate::ribbon::widgets::tool;
use crate::{files, fluent, menu, printers, render, text, theme};

static SYSTEM_FONTS: SystemFonts = SystemFonts;
const MAX_COPIES: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    Active,
    Workbook,
    Selection,
}

impl Scope {
    const ALL: [Self; 3] = [Self::Active, Self::Workbook, Self::Selection];

    fn label(self) -> &'static str {
        match self {
            Self::Active => text::print_active(),
            Self::Workbook => text::print_workbook(),
            Self::Selection => text::print_selection(),
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Self::Active => text::print_active_detail(),
            Self::Workbook => text::print_workbook_detail(),
            Self::Selection => text::print_selection_detail(),
        }
    }
}

pub struct Preview {
    key: (u64, usize, Scope),
    pub pages: Vec<PrintPage>,
    pub fonts: FontBook<'static>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fit {
    None,
    Sheet,
    Columns,
    Rows,
}

impl Fit {
    const ALL: [Self; 4] = [Self::None, Self::Sheet, Self::Columns, Self::Rows];

    fn label(self) -> &'static str {
        match self {
            Self::None => text::no_scaling(),
            Self::Sheet => text::fit_sheet(),
            Self::Columns => text::fit_columns(),
            Self::Rows => text::fit_rows(),
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Self::None => text::no_scaling_detail(),
            Self::Sheet => text::fit_sheet_detail(),
            Self::Columns => text::fit_columns_detail(),
            Self::Rows => text::fit_rows_detail(),
        }
    }

    fn scale(self) -> Scale {
        match self {
            Self::None => Scale::Percent(100),
            Self::Sheet => Scale::Fit {
                width: 1,
                height: 1,
            },
            Self::Columns => Scale::Fit {
                width: 1,
                height: 0,
            },
            Self::Rows => Scale::Fit {
                width: 0,
                height: 1,
            },
        }
    }

    fn of(scale: Scale) -> Option<Self> {
        Self::ALL.into_iter().find(|fit| fit.scale() == scale)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintOptions {
    pub scope: Scope,
    pub copies: u32,
    pub from: u32,
    pub to: u32,
    pub printer: Option<String>,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            scope: Scope::Active,
            copies: 1,
            from: 0,
            to: 0,
            printer: None,
        }
    }
}

fn margin_name(margins: Margins) -> &'static str {
    [
        (text::margins_normal_long(), Margins::NORMAL),
        (text::margins_wide_long(), Margins::WIDE),
        (text::margins_narrow_long(), Margins::NARROW),
    ]
    .into_iter()
    .find(|(_, preset)| *preset == margins)
    .map_or(text::custom_margins_long(), |(name, _)| name)
}

impl Spreadsheet {
    fn print_job(&self, fonts: &mut FontBook) -> Vec<PrintPage> {
        let sheets: Vec<usize> = match self.print.scope {
            Scope::Workbook => self.doc.visible_sheets(),
            _ => vec![self.doc.active()],
        };
        let selection = (self.print.scope == Scope::Selection).then(|| self.selected());
        let today = crate::clock::today();
        let date = locale().format_value(&vellum_grid_core::CellValue::Number(today), "dd.mm.yyyy");
        let job = Job {
            book: &self.book,
            date: &date,
            time: "",
            locale: locale(),
            today,
        };
        layout(self.doc.book(), &sheets, selection, &job, fonts)
    }

    fn preview(&mut self) -> &Preview {
        let key = (self.doc.revision(), self.doc.active(), self.print.scope);
        if self.print_preview.as_ref().is_none_or(|p| p.key != key) {
            let mut fonts = FontBook::new(&SYSTEM_FONTS);
            let pages = self.print_job(&mut fonts);
            self.print_preview = Some(Preview { key, pages, fonts });
            self.print_page = 0;
        }
        self.print_preview.as_ref().expect("preview was just built")
    }

    fn page_span(&mut self) -> (usize, usize) {
        let count = self.preview().pages.len();
        let from = (self.print.from.max(1) as usize).min(count.max(1)) - 1;
        let to = if self.print.to == 0 {
            count
        } else {
            (self.print.to as usize).clamp(from + 1, count.max(from + 1))
        };
        (from, to.min(count))
    }

    pub fn pdf_bytes(&mut self) -> Option<Vec<u8>> {
        let (from, to) = self.page_span();
        let copies = self.print.copies.max(1) as usize;
        let preview = self.preview();
        if from >= to {
            return None;
        }
        let pages: Vec<PrintPage> = (0..copies)
            .flat_map(|_| preview.pages[from..to].iter().cloned())
            .collect();
        Some(write_pdf(&pages, &preview.fonts))
    }

    pub fn export_pdf(&mut self, path: &Path) -> bool {
        let Some(bytes) = self.pdf_bytes() else {
            self.nothing_to_print();
            return false;
        };
        match files::store(path, &bytes) {
            Ok(()) => true,
            Err(message) => {
                self.dialog = Some(Dialog::Error {
                    title: text::save_failed(),
                    message,
                });
                false
            }
        }
    }

    pub fn print_now(&mut self) {
        let Some(bytes) = self.pdf_bytes() else {
            self.nothing_to_print();
            return;
        };
        let Some(path) = print_file(&self.book) else {
            return;
        };
        if files::store(&path, &bytes).is_err() {
            return;
        }
        match &self.print.printer {
            Some(printer) if printers::print(&path, printer) => {}
            _ => printers::open(&path),
        }
    }

    fn nothing_to_print(&mut self) {
        self.dialog = Some(Dialog::Error {
            title: text::app_name(),
            message: text::nothing_to_print().to_owned(),
        });
    }

    pub fn print_page_ui(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.vertical(|ui| {
                ui.set_width(theme::PRINT_PANEL_WIDTH);
                self.print_settings(ui);
            });
            ui.add_space(theme::BACKSTAGE_GAP);
            let line = ui.cursor().left();
            ui.painter()
                .vline(line, full.y_range(), Stroke::new(1.0, theme::ribbon_line()));
            ui.add_space(theme::BACKSTAGE_GAP);
            Frame::NONE
                .fill(theme::preview_bg())
                .corner_radius(CornerRadius::same(theme::MENU_RADIUS))
                .inner_margin(Margin::same(theme::PREVIEW_MARGIN))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    self.preview_ui(ui);
                });
        });
    }

    fn print_settings(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = theme::DIALOG_SPACING;
            if square_button(ui, Icon::Plain(fluent::PRINT), text::print()).clicked() {
                self.print_now();
            }
            if square_button(ui, Icon::Plain(fluent::DOCUMENT_PDF), text::pdf_short()).clicked() {
                self.export("pdf");
            }
            ui.vertical(|ui| {
                ui.add_space(theme::DIALOG_SPACING);
                ui.label(text::copies());
                ui.add(
                    DragValue::new(&mut self.print.copies)
                        .range(1..=MAX_COPIES)
                        .speed(0.1),
                );
            });
        });
        ui.add_space(theme::BACKSTAGE_GAP);
        heading(ui, text::printer());
        let printers = self.printers.names();
        if printers.is_none() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(300));
        }
        let (title, detail) = match &self.print.printer {
            Some(name) => (name.as_str(), text::printer_ready()),
            None => (text::default_viewer(), text::default_viewer_detail()),
        };
        let response = picker(ui, Icon::Plain(fluent::PRINT), title, detail);
        Popup::menu(&response)
            .width(theme::PRINT_PANEL_WIDTH)
            .show(|ui| {
                let current = self.print.printer.clone();
                let check = |selected: bool| selected.then_some(Icon::Plain(fluent::CHECKMARK));
                if menu::detail_item(
                    ui,
                    check(current.is_none()),
                    text::default_viewer(),
                    text::default_viewer_detail(),
                ) {
                    self.print.printer = None;
                }
                for name in printers.unwrap_or_default() {
                    let selected = current.as_deref() == Some(name.as_str());
                    if menu::icon_or_blank(ui, check(selected), &name) {
                        self.print.printer = Some(name);
                    }
                }
            });
        ui.add_space(theme::BACKSTAGE_GAP);
        heading(ui, text::settings());
        let scope = self.print.scope;
        let response = picker(
            ui,
            Icon::Plain(fluent::TABLE),
            scope.label(),
            scope.detail(),
        );
        Popup::menu(&response)
            .width(theme::PRINT_PANEL_WIDTH)
            .show(|ui| {
                for option in Scope::ALL {
                    let icon = (option == scope).then_some(Icon::Plain(fluent::CHECKMARK));
                    if menu::detail_item(ui, icon, option.label(), option.detail()) {
                        self.print.scope = option;
                    }
                }
            });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme::DIALOG_SPACING / 2.0;
            ui.label(text::pages());
            ui.add(
                DragValue::new(&mut self.print.from)
                    .range(0..=9999)
                    .speed(0.1),
            );
            ui.label("–");
            ui.add(
                DragValue::new(&mut self.print.to)
                    .range(0..=9999)
                    .speed(0.1),
            );
        });
        ui.add_space(theme::DIALOG_SPACING / 2.0);
        let setup = self.page_setup().clone();
        let (glyph, label) = if setup.landscape {
            (fluent::ORIENTATION, text::landscape_orientation())
        } else {
            (fluent::DOCUMENT, text::portrait_orientation())
        };
        let response = picker(ui, Icon::Plain(glyph), label, "");
        Popup::menu(&response)
            .width(theme::PRINT_PANEL_WIDTH)
            .show(|ui| {
                for (value, label) in [
                    (false, text::portrait_orientation()),
                    (true, text::landscape_orientation()),
                ] {
                    let icon = (setup.landscape == value).then_some(Icon::Plain(fluent::CHECKMARK));
                    if menu::icon_or_blank(ui, icon, label) {
                        self.set_page(|page| page.landscape = value);
                    }
                }
            });
        let (paper_name, paper_size) = page_setup::paper_parts(setup.paper);
        let response = picker(
            ui,
            Icon::Plain(fluent::DOCUMENT_ONE_PAGE),
            &paper_name,
            &paper_size,
        );
        Popup::menu(&response)
            .width(theme::PRINT_PANEL_WIDTH)
            .show(|ui| {
                for option in Paper::ALL {
                    let (name, size) = page_setup::paper_parts(option);
                    let icon = (setup.paper == option).then_some(Icon::Plain(fluent::CHECKMARK));
                    if menu::detail_item(ui, icon, &name, &size) {
                        self.set_page(|page| page.paper = option);
                    }
                }
            });
        let response = picker(
            ui,
            Icon::Plain(fluent::DOCUMENT_MARGINS),
            margin_name(setup.margins),
            &page_setup::margins_label(setup.margins),
        );
        Popup::menu(&response)
            .width(theme::WIDE_MENU_WIDTH)
            .show(|ui| {
                for preset in [Margins::NORMAL, Margins::WIDE, Margins::NARROW] {
                    let icon = (setup.margins == preset).then_some(Icon::Plain(fluent::CHECKMARK));
                    let detail = page_setup::margins_label(preset);
                    if menu::detail_item(ui, icon, margin_name(preset), &detail) {
                        self.set_page(|page| page.margins = preset);
                    }
                }
                menu::separator(ui);
                if menu::item(ui, text::custom_margins()) {
                    self.open_page_setup(SetupTab::Margins);
                }
            });
        let fit = Fit::of(setup.scale);
        let response = picker(
            ui,
            Icon::Plain(fluent::PAGE_FIT),
            fit.map_or(text::custom_scaling(), Fit::label),
            fit.map_or(text::custom_scaling_detail(), Fit::detail),
        );
        Popup::menu(&response)
            .width(theme::WIDE_MENU_WIDTH)
            .show(|ui| {
                for option in Fit::ALL {
                    let icon = (fit == Some(option)).then_some(Icon::Plain(fluent::CHECKMARK));
                    if menu::detail_item(ui, icon, option.label(), option.detail()) {
                        self.set_page(|page| page.scale = option.scale());
                    }
                }
                menu::separator(ui);
                if menu::item(ui, text::custom_scaling_options()) {
                    self.open_page_setup(SetupTab::Page);
                }
            });
        ui.add_space(theme::DIALOG_SPACING / 2.0);
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            ui.set_width(theme::PRINT_PANEL_WIDTH);
            if ui.link(text::page_setup()).clicked() {
                self.open_page_setup(SetupTab::Page);
            }
        });
    }

    fn preview_ui(&mut self, ui: &mut Ui) {
        let (from, to) = self.page_span();
        let count = to.saturating_sub(from);
        let available = ui.available_size() - vec2(0.0, theme::PREVIEW_NAV_HEIGHT);
        if count == 0 {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(text::nothing_to_print()).color(theme::chrome_text_muted()));
            });
            return;
        }
        let index = self.print_page.min(count - 1);
        let preview = self.print_preview.as_ref().expect("preview exists");
        let page = &preview.pages[from + index];
        let zoom = (available.x / page.width).min(available.y / page.height);
        let size = vec2(page.width, page.height) * zoom;
        let (rect, _) = ui.allocate_exact_size(available, Sense::hover());
        let paper = Rect::from_center_size(rect.center(), size);
        paint_page(ui, paper, zoom, page, &preview.fonts);
        ui.horizontal(|ui| {
            ui.add_space((available.x - theme::PREVIEW_NAV_WIDTH).max(0.0) / 2.0);
            let previous = ui.add_enabled_ui(index > 0, |ui| {
                tool(
                    ui,
                    Icon::Plain(fluent::CHEVRON_LEFT),
                    text::previous_page(),
                    false,
                )
            });
            if previous.inner.clicked() {
                self.print_page = index - 1;
            }
            ui.label(format!("{} / {count}", index + 1));
            let next = ui.add_enabled_ui(index + 1 < count, |ui| {
                tool(
                    ui,
                    Icon::Plain(fluent::CHEVRON_RIGHT),
                    text::next_page(),
                    false,
                )
            });
            if next.inner.clicked() {
                self.print_page = index + 1;
            }
        });
    }
}

fn heading(ui: &mut Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .size(theme::BACKSTAGE_HEADING)
            .color(theme::chrome_text()),
    );
    ui.add_space(theme::DIALOG_SPACING / 2.0);
}

fn square_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let size = vec2(theme::PRINT_BUTTON, theme::PRINT_BUTTON);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let fill = if response.hovered() {
        theme::tool_hover()
    } else {
        theme::ribbon_bg()
    };
    painter.rect(
        rect,
        CornerRadius::same(theme::MENU_RADIUS),
        fill,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
    let area = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + rect.height() * 0.4),
        vec2(theme::BIG_ICON_SIZE, theme::BIG_ICON_SIZE),
    );
    icons::paint(painter, area, icon, theme::icon(), None);
    painter.text(
        pos2(rect.center().x, rect.bottom() - theme::DIALOG_SPACING),
        Align2::CENTER_BOTTOM,
        label,
        theme::menu_font(),
        theme::chrome_text(),
    );
    response
}

fn picker(ui: &mut Ui, icon: Icon, title: &str, detail: &str) -> Response {
    let size = vec2(theme::PRINT_PANEL_WIDTH, theme::PICKER_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let fill = if response.hovered() {
        theme::tool_hover()
    } else {
        theme::surface()
    };
    painter.rect(
        rect,
        CornerRadius::same(theme::MENU_RADIUS),
        fill,
        Stroke::new(1.0, theme::chrome_line()),
        StrokeKind::Inside,
    );
    let area = Rect::from_center_size(
        pos2(
            rect.left() + theme::RECENT_ICON_COLUMN / 2.0,
            rect.center().y,
        ),
        vec2(theme::PICKER_ICON, theme::PICKER_ICON),
    );
    icons::paint(painter, area, icon, theme::icon(), None);
    let x = rect.left() + theme::RECENT_ICON_COLUMN;
    let width = rect.right() - x - theme::PICKER_ARROW;
    let title_galley = painter.layout(
        title.to_owned(),
        theme::menu_font(),
        theme::chrome_text(),
        width,
    );
    if detail.is_empty() {
        let y = rect.center().y - title_galley.size().y / 2.0;
        painter.galley(pos2(x, y), title_galley, theme::chrome_text());
    } else {
        let note = painter.layout_no_wrap(
            detail.to_owned(),
            theme::small_font(),
            theme::chrome_text_muted(),
        );
        let top = rect.center().y - (title_galley.size().y + note.size().y) / 2.0;
        let below = top + title_galley.size().y;
        painter.galley(pos2(x, top), title_galley, theme::chrome_text());
        painter
            .with_clip_rect(Rect::from_min_max(
                pos2(x, rect.top()),
                pos2(x + width, rect.bottom()),
            ))
            .galley(pos2(x, below), note, theme::chrome_text_muted());
    }
    icons::chevron(
        painter,
        pos2(rect.right() - theme::PICKER_ARROW / 2.0, rect.center().y),
        theme::icon(),
    );
    ui.add_space(theme::DIALOG_SPACING / 2.0);
    response
}

fn paint_page(ui: &Ui, paper: Rect, zoom: f32, page: &PrintPage, fonts: &FontBook) {
    let painter = ui.painter().with_clip_rect(paper.intersect(ui.clip_rect()));
    let shadow = Shadow {
        offset: [0, 2],
        blur: theme::POPUP_SHADOW_BLUR,
        spread: 0,
        color: theme::popup_shadow(),
    };
    ui.painter().add(shadow.as_shape(paper, CornerRadius::ZERO));
    painter.rect_filled(paper, CornerRadius::ZERO, theme::PAPER);
    let at = |x: f32, y: f32| -> Pos2 { paper.min + vec2(x, y) * zoom };
    for item in &page.items {
        match item {
            Item::Fill { area, color } => {
                let rect = Rect::from_min_size(at(area.x, area.y), vec2(area.w, area.h) * zoom);
                painter.rect_filled(rect, CornerRadius::ZERO, render::color(*color));
            }
            Item::Line {
                from,
                to,
                width,
                color,
                ..
            } => {
                painter.line_segment(
                    [at(from.0, from.1), at(to.0, to.1)],
                    Stroke::new((width * zoom).max(0.5), render::color(*color)),
                );
            }
            Item::Text {
                x,
                baseline,
                text,
                font,
                size,
                color,
                clip,
                ..
            } => {
                let face = fonts.face(*font);
                let key = &face.key;
                let font_id =
                    FontId::new(size * zoom, family_for(&key.family, key.bold, key.italic));
                let clip_rect = clip.map_or(paper, |c| {
                    Rect::from_min_size(at(c.x, c.y), vec2(c.w, c.h) * zoom).intersect(paper)
                });
                let ascent = face.ascent() * size * zoom;
                painter.with_clip_rect(clip_rect).text(
                    at(*x, *baseline) - vec2(0.0, ascent),
                    Align2::LEFT_TOP,
                    text,
                    font_id,
                    render::color(*color),
                );
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn print_file(book: &str) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(text::app_name());
    std::fs::create_dir_all(&dir).ok()?;
    let safe: String = book
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Some(dir.join(format!("{safe}.pdf")))
}

#[cfg(target_arch = "wasm32")]
fn print_file(book: &str) -> Option<PathBuf> {
    Some(PathBuf::from(format!("{book}.pdf")))
}
