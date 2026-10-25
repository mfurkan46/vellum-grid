use eframe::egui::{
    self, Align2, CornerRadius, Frame, Id, Margin, Rect, RichText, Sense, Stroke, StrokeKind, Ui,
    pos2, vec2,
};

use crate::app::{Pending, Spreadsheet};
use crate::icons::Icon;
use crate::{files, fluent, settings, text, theme};
pub use vellum_ui::backstage::wide_button;
use vellum_ui::backstage::{nav_row, recent_row, separator, sized_button, wide_width};
use vellum_ui::motion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Home,
    New,
    Open,
    Info,
    Print,
    Export,
}

impl Page {
    const ALL: [Self; 6] = [
        Self::Home,
        Self::New,
        Self::Open,
        Self::Info,
        Self::Print,
        Self::Export,
    ];
}

enum Nav {
    Page(Page),
    Save,
    SaveAs,
    Close,
    Options,
}

impl Spreadsheet {
    pub fn backstage(&mut self, ui: &mut Ui, page: Page, shown: f32) {
        egui::Panel::left("backstage-nav")
            .exact_size(theme::BACKSTAGE_NAV_WIDTH)
            .resizable(false)
            .frame(Frame::NONE.fill(theme::surface()))
            .show(ui, |ui| {
                let slide = vec2(-theme::BACKSTAGE_NAV_WIDTH, 0.0);
                motion::revealed(ui, shown, slide, |ui| {
                    ui.painter().rect_filled(
                        ui.max_rect(),
                        CornerRadius::ZERO,
                        theme::backstage_nav(),
                    );
                    self.backstage_nav(ui, page);
                });
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            Frame::NONE
                .fill(theme::surface())
                .inner_margin(Margin::symmetric(
                    theme::BACKSTAGE_PADDING_X,
                    theme::BACKSTAGE_PADDING_Y,
                ))
                .show(ui, |ui| {
                    let mut current = shown;
                    for other in Page::ALL {
                        let t = motion::reveal(
                            ui.ctx(),
                            Id::unique(("backstage-page", other)),
                            other == page,
                            theme::BACKSTAGE_ANIMATION,
                        );
                        if other == page {
                            current = current.min(t);
                        }
                    }
                    let slide = vec2(theme::BACKSTAGE_SLIDE, 0.0);
                    motion::revealed(ui, current, slide, |ui| self.backstage_page(ui, page));
                });
        });
    }

    fn backstage_page(&mut self, ui: &mut Ui, page: Page) {
        ui.set_min_size(ui.available_size());
        let title = match page {
            Page::Home => text::greeting(),
            Page::New => text::new(),
            Page::Open => text::open(),
            Page::Info => text::info(),
            Page::Print => text::print(),
            Page::Export => text::export(),
        };
        ui.label(
            RichText::new(title)
                .size(theme::BACKSTAGE_TITLE)
                .color(theme::chrome_text()),
        );
        ui.add_space(theme::BACKSTAGE_GAP);
        if page == Page::Print {
            self.print_page_ui(ui);
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match page {
                Page::Home => {
                    self.new_tiles(ui);
                    ui.add_space(theme::BACKSTAGE_GAP);
                    self.recent_list(ui);
                }
                Page::New => self.new_tiles(ui),
                Page::Open => {
                    if wide_button(ui, Icon::Plain(fluent::FOLDER_OPEN), text::browse()).clicked() {
                        let ctx = ui.ctx().clone();
                        self.request(&ctx, Pending::Open);
                    }
                    ui.add_space(theme::BACKSTAGE_GAP);
                    self.recent_list(ui);
                }
                Page::Info => self.info_page(ui),
                Page::Print => {}
                Page::Export => self.export_page(ui),
            });
    }

    fn backstage_nav(&mut self, ui: &mut Ui, page: Page) {
        ui.add_space(theme::BACKSTAGE_NAV_TOP);
        let back = nav_row(ui, Some(fluent::ARROW_LEFT), "", false);
        if back.on_hover_text(text::back()).clicked() {
            self.backstage = None;
        }
        ui.add_space(theme::BACKSTAGE_NAV_TOP);
        let items = [
            (Some(fluent::HOME), text::home_page(), Nav::Page(Page::Home)),
            (
                Some(fluent::DOCUMENT_ADD),
                text::new(),
                Nav::Page(Page::New),
            ),
            (
                Some(fluent::FOLDER_OPEN),
                text::open(),
                Nav::Page(Page::Open),
            ),
        ];
        let lower = [
            (Some(fluent::INFO), text::info(), Nav::Page(Page::Info)),
            (Some(fluent::SAVE), text::save(), Nav::Save),
            (Some(fluent::SAVE_EDIT), text::save_as(), Nav::SaveAs),
            (Some(fluent::PRINT), text::print(), Nav::Page(Page::Print)),
            (
                Some(fluent::ARROW_EXPORT),
                text::export(),
                Nav::Page(Page::Export),
            ),
            (Some(fluent::DISMISS), text::close(), Nav::Close),
            (
                Some(fluent::SETTINGS),
                vellum_ui::text::options(),
                Nav::Options,
            ),
        ];
        let mut chosen = None;
        for (glyph, label, nav) in items {
            let selected = matches!(nav, Nav::Page(p) if p == page);
            if nav_row(ui, glyph, label, selected).clicked() {
                chosen = Some(nav);
            }
        }
        separator(ui);
        for (glyph, label, nav) in lower {
            let selected = matches!(nav, Nav::Page(p) if p == page);
            if nav_row(ui, glyph, label, selected).clicked() {
                chosen = Some(nav);
            }
        }
        let ctx = ui.ctx().clone();
        match chosen {
            Some(Nav::Page(page)) => self.backstage = Some(page),
            Some(Nav::Save) => {
                if self.save(false) {
                    self.backstage = None;
                }
            }
            Some(Nav::SaveAs) => {
                if self.save(true) {
                    self.backstage = None;
                }
            }
            Some(Nav::Close) => {
                self.backstage = None;
                self.request(&ctx, Pending::Close);
            }
            Some(Nav::Options) => {
                self.dialog = Some(crate::dialogs::Dialog::Options(self.prefs));
            }
            None => {}
        }
    }

    fn new_tiles(&mut self, ui: &mut Ui) {
        let size = vec2(theme::NEW_TILE_WIDTH, theme::NEW_TILE_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter();
        let page = Rect::from_min_size(
            rect.min,
            vec2(rect.width(), rect.height() - theme::NEW_TILE_LABEL),
        );
        let line = if response.hovered() {
            theme::accent()
        } else {
            theme::chrome_line()
        };
        painter.rect(
            page,
            CornerRadius::same(theme::BUTTON_RADIUS as u8),
            theme::surface(),
            Stroke::new(1.0, line),
            StrokeKind::Inside,
        );
        let grid = page.shrink(theme::NEW_TILE_INSET);
        let stroke = Stroke::new(1.0, theme::chrome_line());
        for i in 0..=theme::NEW_TILE_LINES {
            let t = i as f32 / theme::NEW_TILE_LINES as f32;
            painter.hline(grid.x_range(), grid.top() + t * grid.height(), stroke);
            painter.vline(grid.left() + t * grid.width(), grid.y_range(), stroke);
        }
        painter.text(
            pos2(rect.left(), page.bottom() + theme::NEW_TILE_LABEL / 2.0),
            Align2::LEFT_CENTER,
            text::blank_workbook(),
            theme::menu_font(),
            theme::chrome_text(),
        );
        if response.clicked() {
            let ctx = ui.ctx().clone();
            self.backstage = None;
            self.request(&ctx, Pending::New);
        }
    }

    fn recent_list(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(text::recent())
                .size(theme::BACKSTAGE_HEADING)
                .color(theme::chrome_text()),
        );
        ui.add_space(theme::BACKSTAGE_GAP / 2.0);
        let recent = settings::recent();
        if recent.is_empty() {
            ui.label(RichText::new(text::no_recent()).color(theme::chrome_text_muted()));
            return;
        }
        let mut open = None;
        let mut forget = None;
        for path in recent {
            let response = recent_row(ui, &path, Icon::Tinted(fluent::TABLE, theme::tint_green()));
            if response.clicked() {
                open = Some(path.clone());
            }
            response.context_menu(|ui| {
                if crate::menu::item(ui, text::remove_from_list()) {
                    forget = Some(path.clone());
                }
            });
        }
        if let Some(path) = forget {
            settings::forget(&path);
        }
        if let Some(path) = open {
            if path.exists() {
                let ctx = ui.ctx().clone();
                self.pending_path = Some(path);
                self.request(&ctx, Pending::OpenRecent);
            } else {
                settings::forget(&path);
                self.dialog = Some(crate::dialogs::Dialog::Error {
                    title: text::app_name(),
                    message: text::missing_file().to_owned(),
                });
            }
        }
    }

    fn info_page(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(&self.book)
                .size(theme::BACKSTAGE_HEADING)
                .color(theme::chrome_text()),
        );
        let path = self.path.clone();
        if let Some(path) = &path {
            ui.label(RichText::new(path.display().to_string()).color(theme::chrome_text_muted()));
        }
        ui.add_space(theme::BACKSTAGE_GAP);
        let metadata = path.as_ref().and_then(|p| std::fs::metadata(p).ok());
        let cells: usize = self
            .doc
            .book()
            .sheets()
            .iter()
            .map(|sheet| sheet.cells().filter(|(_, c)| c.has_content()).count())
            .sum();
        let mut rows = vec![
            (text::sheets(), self.doc.book().sheets().len().to_string()),
            (text::filled_cells(), cells.to_string()),
        ];
        if let Some(metadata) = &metadata {
            rows.insert(0, (text::size(), settings::size_text(metadata.len())));
            if let Ok(modified) = metadata.modified() {
                rows.insert(1, (text::last_modified(), settings::ago(modified)));
            }
        } else {
            rows.insert(0, (text::size(), text::not_saved().to_owned()));
        }
        egui::Grid::new("book-info")
            .num_columns(2)
            .spacing(vec2(theme::BACKSTAGE_GAP, theme::DIALOG_SPACING))
            .show(ui, |ui| {
                for (label, value) in rows {
                    ui.label(RichText::new(label).color(theme::chrome_text_muted()));
                    ui.label(RichText::new(value).color(theme::chrome_text()));
                    ui.end_row();
                }
            });
        if let Some(path) = path {
            ui.add_space(theme::BACKSTAGE_GAP);
            if wide_button(ui, Icon::Plain(fluent::FOLDER), text::open_location()).clicked() {
                settings::reveal(&path);
            }
        }
    }

    fn export_page(&mut self, ui: &mut Ui) {
        let options = [
            (fluent::DOCUMENT_PDF, text::export_pdf(), "pdf"),
            (fluent::DOCUMENT_TABLE, text::export_ods(), "ods"),
            (fluent::DOCUMENT, text::export_csv(), "csv"),
            (fluent::DOCUMENT, text::export_txt(), "txt"),
            (fluent::GLOBE, text::export_html(), "html"),
            (fluent::TABLE, text::export_xlsx(), "xlsx"),
        ];
        let labels: Vec<&str> = options.iter().map(|(_, label, _)| *label).collect();
        let width = wide_width(ui, &labels);
        for (glyph, label, extension) in options {
            if sized_button(ui, Icon::Plain(glyph), label, width).clicked() {
                self.export(extension);
            }
            ui.add_space(theme::DIALOG_SPACING / 2.0);
        }
    }

    pub fn export(&mut self, extension: &str) {
        let Some(path) = files::pick_export(&self.book, extension) else {
            return;
        };
        let done = if extension == "pdf" {
            self.export_pdf(&path)
        } else {
            self.write_to(&path)
        };
        if done {
            self.backstage = None;
        }
    }
}
