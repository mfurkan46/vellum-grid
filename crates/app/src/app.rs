use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use eframe::egui::{self, Context, Frame, Id, Margin, TextEdit, Ui, ViewportCommand};
use vellum_grid_core::{
    Cell, CellAddr, CellRange, Color, Document, EN_US, Locale, MAX_COLS, MAX_ROWS, Sheet, Style,
    TR, Workbook,
};
use vellum_grid_io::tsv;

use crate::assist::Assist;
use crate::backstage::Page;
use crate::context::ContextTarget;
use crate::data::FilterMenu;
use crate::dialogs::Dialog;
use crate::format::BorderKind;
use crate::grid::SoftCursor;
use crate::ribbon::Tab;
use crate::search::FindState;
use crate::status::Stats;
use crate::tabs::Rename;
use crate::{clock, files, fluent, icons, palette, recovery, settings, text, theme, titlebar};

pub fn locale() -> &'static Locale {
    if vellum_ui::english() { &EN_US } else { &TR }
}
pub const FORMULA_BAR_ID: &str = "formula-bar";
const NAME_BOX_ID: &str = "name-box";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub active: CellAddr,
    pub anchor: CellAddr,
    pub corner: CellAddr,
}

impl Selection {
    pub fn at(addr: CellAddr) -> Self {
        Self {
            active: addr,
            anchor: addr,
            corner: addr,
        }
    }

    pub fn range(self) -> CellRange {
        CellRange::new(self.anchor, self.corner)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct View {
    pub top: u32,
    pub left: u32,
    pub rows: u32,
    pub cols: u32,
    pub wheel_rows: f32,
    pub wheel_cols: f32,
    pub wheel_zoom: f32,
    pub min_top: u32,
    pub min_left: u32,
}

impl View {
    pub fn reveal(&mut self, addr: CellAddr) {
        let rows = self.rows.max(1);
        let cols = self.cols.max(1);
        if addr.row() >= self.min_top {
            if addr.row() < self.top {
                self.top = addr.row();
            } else if addr.row() >= self.top + rows {
                self.top = addr.row() + 1 - rows;
            }
        }
        if addr.col() >= self.min_left {
            if addr.col() < self.left {
                self.left = addr.col();
            } else if addr.col() >= self.left + cols {
                self.left = addr.col() + 1 - cols;
            }
        }
    }

    pub fn scroll(&mut self, rows: i64, cols: i64) {
        let shift = |value: u32, delta: i64, min: u32, max: u32| {
            (i64::from(value) + delta).clamp(i64::from(min.min(max - 1)), i64::from(max - 1)) as u32
        };
        self.top = shift(self.top, rows, self.min_top, MAX_ROWS);
        self.left = shift(self.left, cols, self.min_left, MAX_COLS);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Enter,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub start: usize,
    pub anchor: CellAddr,
    pub corner: CellAddr,
}

impl Point {
    pub fn range(self) -> CellRange {
        CellRange::new(self.anchor, self.corner)
    }
}

#[derive(Debug)]
pub struct Edit {
    pub addr: CellAddr,
    pub text: String,
    pub mode: EditMode,
    pub id: Id,
    pub focus_cell: bool,
    pub point: Option<Point>,
    pub cursor_to_end: bool,
    pub typed: String,
    pub suggestion: Option<String>,
    pub assist: Option<Assist>,
    pub assist_closed: Option<String>,
}

impl Edit {
    pub fn pointing(&mut self) -> Option<Point> {
        let point = self.point?;
        let written = point.range().to_string();
        if self.text.get(point.start..) == Some(written.as_str()) {
            Some(point)
        } else {
            self.point = None;
            None
        }
    }

    pub fn can_point(&mut self) -> bool {
        self.text.starts_with('=')
            && (self.pointing().is_some()
                || self
                    .text
                    .trim_end()
                    .ends_with(|c: char| "=(+-*/^&<>:".contains(c) || c == locale().list))
    }

    pub fn point_at(&mut self, anchor: CellAddr, corner: CellAddr) {
        let start = match self.pointing() {
            Some(point) => point.start,
            None => self.text.len(),
        };
        let point = Point {
            start,
            anchor,
            corner,
        };
        self.text.truncate(start);
        self.text.push_str(&point.range().to_string());
        self.point = Some(point);
        self.cursor_to_end = true;
    }
}

#[derive(Debug, Clone)]
pub struct Clipboard {
    pub sheet: usize,
    pub range: CellRange,
    pub cut: bool,
    text: String,
    pub cells: Vec<(u32, u32, Cell)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drag {
    Cells,
    Rows,
    Cols,
    Point,
    Resize,
    Fill,
    Move { grab_rows: u32, grab_cols: u32 },
    Object,
    VerticalBar { grab: f32, total: u32 },
    HorizontalBar { grab: f32, total: u32 },
}

#[derive(Debug, Clone)]
pub struct Resize {
    pub columns: bool,
    pub indices: Vec<u32>,
    pub start: f32,
    pub original: f32,
    pub size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    New,
    Open,
    OpenRecent,
    Close,
}

pub struct Spreadsheet {
    pub doc: Document,
    pub sel: Selection,
    pub extra: Vec<CellRange>,
    pub move_target: Option<CellAddr>,
    pub view: View,
    pub edit: Option<Edit>,
    pub clipboard: Option<Clipboard>,
    pub drag: Option<Drag>,
    pub resize: Option<Resize>,
    pub format_painter: Option<Style>,
    pub show_formulas: bool,
    pub show_formula_bar: bool,
    pub formula_expanded: bool,
    pub autosave: bool,
    dirty_since: Option<f64>,
    pub ribbon_collapsed: bool,
    pub fill_target: Option<CellRange>,
    pub autofit_queue: Vec<u32>,
    pub backstage: Option<Page>,
    pub pending_path: Option<PathBuf>,
    pub print: crate::printing::PrintOptions,
    pub printers: crate::printers::Printers,
    pub print_preview: Option<crate::printing::Preview>,
    pub print_page: usize,
    pub page_break_view: bool,
    recovery: Option<PathBuf>,
    recovery_revision: u64,
    recovery_since: Option<f64>,
    pub filter_menu: Option<FilterMenu>,
    pub find: Option<FindState>,
    pub object: Option<crate::objects::Object>,
    pub object_drag: Option<crate::objects::ObjectDrag>,
    pub shape_tool: Option<vellum_grid_core::drawing::ShapeKind>,
    pub shape_edit: Option<crate::objects::ShapeEdit>,
    pub textures: crate::objects::Textures,
    pub zoom: f32,
    pub stats: Stats,
    pub grid_area: egui::Vec2,
    pub context_target: Option<ContextTarget>,
    pub soft_cursor: Option<SoftCursor>,
    pub tab: Tab,
    pub fill_color: Option<Color>,
    pub font_color: Option<Color>,
    pub border: BorderKind,
    pub renaming: Option<Rename>,
    pub tab_drag: Option<usize>,
    pub tab_scroll: usize,
    pub digit: f32,
    pub book: String,
    pub dialog: Option<Dialog>,
    pub views: HashMap<usize, (Selection, View)>,
    pub path: Option<PathBuf>,
    edit_serial: u64,
    name_box: String,
    pub arrows: Vec<crate::trace::Arrow>,
    pub arrows_revision: u64,
    pub issue_cells: Vec<CellAddr>,
    pub issue_tag_rect: Option<egui::Rect>,
    pub list_popup: Option<CellAddr>,
    pub list_button_rect: Option<egui::Rect>,
    pub circles: bool,
    pub circle_cells: Vec<CellAddr>,
    pub circle_key: Option<(u64, usize)>,
    pub skip_validation: bool,
    pub advanced_list: Option<(usize, CellRange)>,
    tab_start: Option<u32>,
    pub note_edit: Option<crate::review_ui::NoteEdit>,
    pub comment_edit: Option<crate::review_ui::CommentEdit>,
    pub comments_pane: bool,
    pub show_all_notes: bool,
    pub hover: Option<CellAddr>,
    pub table_name_edit: Option<String>,
    pub speller: Option<crate::spelling::Speller>,
    pub spell_ignored: HashSet<String>,
    pub issue_key: Option<(u64, usize, Vec<CellRange>)>,
    closing: bool,
    title: String,
    pub inbox: files::Inbox,
    pub prefs: vellum_ui::settings::Prefs,
    styled: Option<bool>,
}

impl Spreadsheet {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        vellum_ui::theme::install(&cc.egui_ctx);
        vellum_ui::theme::set_accent(vellum_ui::theme::GRID);
        let prefs = vellum_ui::settings::Prefs::load();
        prefs.apply(cc.egui_ctx.system_theme() == Some(egui::Theme::Dark));
        let mut app = Self {
            doc: new_document(blank_book()),
            sel: Selection::at(origin()),
            extra: Vec::new(),
            move_target: None,
            view: View::default(),
            edit: None,
            clipboard: None,
            drag: None,
            resize: None,
            format_painter: None,
            show_formulas: false,
            show_formula_bar: true,
            formula_expanded: false,
            autosave: false,
            dirty_since: None,
            ribbon_collapsed: false,
            fill_target: None,
            autofit_queue: Vec::new(),
            backstage: None,
            pending_path: None,
            print: crate::printing::PrintOptions::default(),
            printers: crate::printers::Printers::default(),
            print_preview: None,
            print_page: 0,
            page_break_view: false,
            recovery: None,
            recovery_revision: 0,
            recovery_since: None,
            filter_menu: None,
            find: None,
            object: None,
            object_drag: None,
            shape_tool: None,
            shape_edit: None,
            textures: Default::default(),
            zoom: 1.0,
            stats: Stats::default(),
            grid_area: egui::Vec2::ZERO,
            context_target: None,
            soft_cursor: None,
            tab: Tab::default(),
            fill_color: Some(palette::DEFAULT_FILL),
            font_color: Some(palette::DEFAULT_FONT_COLOR),
            border: BorderKind::Bottom,
            renaming: None,
            tab_drag: None,
            tab_scroll: 0,
            digit: theme::DEFAULT_DIGIT_WIDTH,
            book: text::new_book().to_owned(),
            path: None,
            edit_serial: 0,
            name_box: String::new(),
            arrows: Vec::new(),
            arrows_revision: 0,
            issue_cells: Vec::new(),
            issue_tag_rect: None,
            list_popup: None,
            list_button_rect: None,
            circles: false,
            circle_cells: Vec::new(),
            circle_key: None,
            skip_validation: false,
            advanced_list: None,
            tab_start: None,
            note_edit: None,
            comment_edit: None,
            comments_pane: false,
            show_all_notes: false,
            hover: None,
            table_name_edit: None,
            speller: None,
            spell_ignored: HashSet::new(),
            issue_key: None,
            dialog: None,
            closing: false,
            title: String::new(),
            views: HashMap::new(),
            inbox: files::Inbox::default(),
            prefs,
            styled: None,
        };
        if let Some(path) = path {
            app.open_path(path);
        }
        app.offer_recovery();
        app
    }

    pub fn sheet(&self) -> &Sheet {
        self.doc.sheet()
    }

    pub fn selected(&self) -> CellRange {
        self.sheet().layout().expand(self.sel.range())
    }

    pub fn ranges(&self) -> Vec<CellRange> {
        let mut ranges = self.extra.clone();
        ranges.push(self.selected());
        ranges
    }

    pub fn select(&mut self, sel: Selection) {
        self.tab_start = None;
        self.extra.clear();
        self.sel = sel;
        self.view.reveal(sel.corner);
    }

    pub fn enter_step(&mut self, rows: i64, cols: i64, tab: bool) {
        let start = if tab {
            Some(self.tab_start.unwrap_or(self.sel.active.col()))
        } else {
            self.tab_start
        };
        match start {
            Some(col) if !tab && rows == 1 && cols == 0 => {
                let from = CellAddr::new(self.sel.active.row(), col).expect("inside the sheet");
                let target = self.step_visible(from, 1, 0);
                self.select(Selection::at(target));
            }
            _ => self.move_active(rows, cols),
        }
        self.tab_start = if tab { start } else { None };
    }

    pub fn move_active(&mut self, rows: i64, cols: i64) {
        let target = self.step_visible(self.sel.active, rows, cols);
        self.select(Selection::at(target));
    }

    pub fn step_visible(&self, from: CellAddr, rows: i64, cols: i64) -> CellAddr {
        let layout = self.sheet().layout();
        let mut target = from.offset(rows, cols);
        while layout.is_hidden(target) {
            let next = target.offset(rows.signum(), cols.signum());
            if next == target {
                return from;
            }
            target = next;
        }
        target
    }

    pub fn start_edit(&mut self, text: String, mode: EditMode, focus_cell: bool) {
        if self.locked_blocked() {
            return;
        }
        self.edit_serial += 1;
        self.clipboard = None;
        self.view.reveal(self.sel.active);
        let typed = if mode == EditMode::Enter {
            String::new()
        } else {
            text.clone()
        };
        self.edit = Some(Edit {
            addr: self.sel.active,
            text,
            typed,
            suggestion: None,
            assist: None,
            assist_closed: None,
            mode,
            id: Id::unique(("cell-editor", self.edit_serial)),
            focus_cell,
            point: None,
            cursor_to_end: false,
        });
    }

    pub fn active_input(&self) -> String {
        if self.doc.hides_formula(self.sel.active) {
            return String::new();
        }
        self.doc.input_text(self.sel.active)
    }

    pub fn commit_edit(&mut self, ctx: &Context) -> bool {
        let Some(edit) = self.edit.as_mut() else {
            return true;
        };
        let addr = edit.addr;
        let input = edit.text.clone();
        let lines = input.contains('\n');
        if !self.skip_validation
            && let Some(rule) = self.check_entry(addr, &input)
        {
            self.dialog = Some(Dialog::ValidationAlert(
                crate::validation_ui::ValidationAlert { rule },
            ));
            return false;
        }
        let Some(edit) = self.edit.as_mut() else {
            return true;
        };
        self.doc.tick(clock::now());
        match self.doc.enter(addr, &input) {
            Ok(_) => {
                if lines && !self.sheet().layout().row_heights.contains_key(&addr.row()) {
                    self.autofit_queue.push(addr.row());
                }
                self.edit = None;
                release_focus(ctx);
                true
            }
            Err(error) => {
                edit.focus_cell = true;
                edit.point = None;
                self.dialog = Some(Dialog::Formula(error));
                false
            }
        }
    }

    pub fn cancel_edit(&mut self, ctx: &Context) {
        self.edit = None;
        release_focus(ctx);
    }

    pub fn clear_selection(&mut self) {
        self.clipboard = None;
        self.doc.clear_ranges(&self.ranges());
    }

    pub fn undo(&mut self) {
        let before = self.doc.active();
        if let Some((sheet, range)) = self.doc.undo() {
            self.show_changed(before, sheet, range);
        }
    }

    pub fn redo(&mut self) {
        let before = self.doc.active();
        if let Some((sheet, range)) = self.doc.redo() {
            self.show_changed(before, sheet, range);
        }
    }

    pub fn activate(&mut self, ctx: &Context, index: usize) {
        let current = self.doc.active();
        if index == current || index >= self.doc.book().sheets().len() || !self.commit_edit(ctx) {
            return;
        }
        self.switch_sheet(index);
    }

    pub fn switch_sheet(&mut self, index: usize) {
        let current = self.doc.active();
        if index == current {
            return;
        }
        self.views.insert(current, (self.sel, self.view));
        self.extra.clear();
        self.doc.set_active(index);
        (self.sel, self.view) = self
            .views
            .remove(&index)
            .unwrap_or((Selection::at(origin()), View::default()));
    }

    fn show_changed(&mut self, before: usize, sheet: usize, range: Option<CellRange>) {
        self.clipboard = None;
        if self.sheet().layout().sheet_hidden
            && let Some(&first) = self.doc.visible_sheets().first()
        {
            self.doc.set_active(first);
            self.views.clear();
            self.sel = Selection::at(origin());
            self.view = View::default();
            return;
        }
        if sheet != before {
            self.views.insert(before, (self.sel, self.view));
            self.view = self
                .views
                .remove(&sheet)
                .map_or_else(View::default, |(_, view)| view);
        }
        let Some(range) = range else {
            return;
        };
        self.select(Selection {
            active: range.start(),
            anchor: range.start(),
            corner: range.end(),
        });
    }

    pub fn copy(&mut self, ctx: &Context, cut: bool) {
        if !self.extra.is_empty() {
            self.dialog = Some(Dialog::Error {
                title: text::app_name(),
                message: text::multi_selection().to_owned(),
            });
            return;
        }
        let range = self.sheet().trim_to_extent(self.selected());
        let start = range.start();
        let cells = self
            .sheet()
            .cells_in(range)
            .map(|(addr, cell)| {
                (
                    addr.row() - start.row(),
                    addr.col() - start.col(),
                    cell.clone(),
                )
            })
            .collect();
        let text = tsv::to_tsv(self.sheet(), range, |cell| {
            locale().format_value(&cell.value, cell.number_format())
        });
        ctx.copy_text(text.clone());
        self.clipboard = Some(Clipboard {
            sheet: self.doc.active(),
            range,
            cut,
            text,
            cells,
        });
    }

    pub fn paste(&mut self, pasted: &str) {
        let origin = self.sel.range().start();
        let active = self.doc.active();
        let mut edits = Vec::new();
        let normalize = |s: &str| s.replace("\r\n", "\n");
        let same = |clip: &Clipboard| normalize(&clip.text) == normalize(pasted);
        if let Some(clip) = self
            .clipboard
            .as_ref()
            .filter(|clip| clip.cut && clip.sheet == active && same(clip))
        {
            let range = clip.range;
            self.clipboard = None;
            self.move_now(range, origin, false);
            return;
        }
        let (rows, cols) = match self.clipboard.take().filter(same) {
            Some(clip) => {
                if clip.cut {
                    let source = &self.doc.book().sheets()[clip.sheet];
                    let addrs: Vec<CellAddr> =
                        source.cells_in(clip.range).map(|(addr, _)| addr).collect();
                    edits.extend(
                        addrs
                            .into_iter()
                            .map(|addr| (clip.sheet, addr, Cell::default())),
                    );
                } else {
                    self.clipboard = Some(clip.clone());
                }
                let (rows, cols) = (clip.range.rows(), clip.range.cols());
                let shift_rows = i64::from(origin.row()) - i64::from(clip.range.start().row());
                let shift_cols = i64::from(origin.col()) - i64::from(clip.range.start().col());
                let mut block = vec![Cell::default(); (rows * cols) as usize];
                for (row, col, cell) in clip.cells {
                    block[(row * cols + col) as usize] = if clip.cut {
                        cell
                    } else {
                        cell.shifted(shift_rows, shift_cols)
                    };
                }
                for (i, cell) in block.into_iter().enumerate() {
                    let (row, col) = (i as u32 / cols, i as u32 % cols);
                    edits.extend(target(origin, row, col).map(|addr| (active, addr, cell)));
                }
                (rows, cols)
            }
            None => {
                let rows = tsv::parse_tsv(pasted);
                let height = rows.len() as u32;
                let width = rows.iter().map(Vec::len).max().unwrap_or(0) as u32;
                for (r, row) in rows.iter().enumerate() {
                    for (c, value) in row.iter().enumerate() {
                        if let Some(addr) = target(origin, r as u32, c as u32) {
                            edits.push((active, addr, self.doc.styled_input(addr, value)));
                        }
                    }
                }
                (height, width)
            }
        };
        if rows == 0 || cols == 0 {
            return;
        }
        self.doc.edit_cells(edits);
        let corner = origin.offset(i64::from(rows) - 1, i64::from(cols) - 1);
        self.select(Selection {
            active: origin,
            anchor: origin,
            corner,
        });
    }

    pub fn request(&mut self, ctx: &Context, action: Pending) {
        if action == Pending::Close {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else if self.doc.is_modified() {
            self.dialog = Some(Dialog::SavePrompt(action));
        } else {
            self.run(ctx, action);
        }
    }

    pub fn run(&mut self, ctx: &Context, action: Pending) {
        match action {
            Pending::New => self.reset(blank_book(), text::new_book().to_owned(), None),
            Pending::Open => {
                if let Some((path, bytes)) = files::pick_open(&self.inbox, ctx) {
                    self.open_bytes(path, bytes);
                }
            }
            Pending::OpenRecent => {
                if let Some(path) = self.pending_path.take() {
                    self.open_path(path);
                }
            }
            Pending::Close => {
                self.closing = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        match files::load(&path) {
            Ok(book) => {
                settings::remember(&path);
                self.reset(book, files::book_name(&path), Some(path));
            }
            Err(message) => self.fail(text::open_failed(), &path, message),
        }
    }

    fn open_bytes(&mut self, path: PathBuf, bytes: Vec<u8>) {
        match files::read(&path, bytes) {
            Ok(book) => {
                settings::remember(&path);
                self.reset(book, files::book_name(&path), Some(path));
            }
            Err(message) => self.fail(text::open_failed(), &path, message),
        }
    }

    fn reset(&mut self, book: Workbook, name: String, path: Option<PathBuf>) {
        self.doc = new_document(book);
        self.backstage = None;
        self.drop_recovery();
        self.views.clear();
        self.sel = Selection::at(origin());
        self.extra.clear();
        self.view = View::default();
        self.edit = None;
        self.clipboard = None;
        self.resize = None;
        self.format_painter = None;
        self.renaming = None;
        self.fill_target = None;
        self.filter_menu = None;
        self.object = None;
        self.object_drag = None;
        self.shape_tool = None;
        self.shape_edit = None;
        self.book = name;
        self.path = path;
    }

    pub fn save(&mut self, save_as: bool) -> bool {
        let path = match &self.path {
            Some(path) if !save_as && files::can_save(path) => path.clone(),
            _ => match files::pick_save(&self.book) {
                Some(path) => path,
                None => return false,
            },
        };
        self.write_to(&path)
    }

    pub fn write_to(&mut self, path: &std::path::Path) -> bool {
        if files::is_pdf(path) {
            return self.export_pdf(path);
        }
        let path = path.to_path_buf();
        let result = files::write(self.doc.book(), self.doc.active(), &path)
            .and_then(|bytes| files::store(&path, &bytes));
        match result {
            Ok(()) => {
                if files::single_sheet(&path) && self.doc.book().sheets().len() > 1 {
                    self.dialog = Some(Dialog::Error {
                        title: text::app_name(),
                        message: text::csv_one_sheet().to_owned(),
                    });
                }
                self.doc.mark_saved();
                self.drop_recovery();
                settings::remember(&path);
                self.book = files::book_name(&path);
                self.path = Some(path);
                true
            }
            Err(message) => {
                self.fail(text::save_failed(), &path, message);
                false
            }
        }
    }

    fn fail(&mut self, title: &'static str, path: &std::path::Path, message: String) {
        self.dialog = Some(Dialog::Error {
            title,
            message: format!("{}\n\n{message}", path.display()),
        });
    }

    pub fn toggle_autosave(&mut self) {
        if self.autosave {
            self.autosave = false;
            return;
        }
        let savable = self.path.as_deref().is_some_and(files::can_save);
        self.autosave = savable || self.save(true);
    }

    fn recovery_tick(&mut self, ctx: &Context) {
        if cfg!(target_arch = "wasm32")
            || !self.doc.is_modified()
            || self.doc.revision() == self.recovery_revision
        {
            self.recovery_since = None;
            return;
        }
        let now = ctx.input(|i| i.time);
        let since = *self.recovery_since.get_or_insert(now);
        if now - since < theme::RECOVERY_INTERVAL {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(theme::RECOVERY_INTERVAL));
            return;
        }
        self.recovery_since = None;
        self.recovery_revision = self.doc.revision();
        if self.recovery.is_none() {
            self.recovery = recovery::file_for(&self.book);
        }
        if let Some(path) = self.recovery.clone() {
            recovery::write(self.doc.book().clone(), self.doc.active(), path);
        }
    }

    fn drop_recovery(&mut self) {
        if let Some(path) = self.recovery.take() {
            recovery::remove(&path);
        }
        self.recovery_revision = self.doc.revision();
        self.recovery_since = None;
    }

    pub fn offer_recovery(&mut self) {
        let files = recovery::list();
        if !files.is_empty() && self.dialog.is_none() {
            self.dialog = Some(Dialog::Recover { files, selected: 0 });
        }
    }

    pub fn open_recovered(&mut self, file: &recovery::Recovered) {
        match files::load(&file.path) {
            Ok(book) => {
                let name = format!("{} ({})", file.name, text::recovered());
                self.reset(book, name, None);
                self.doc.mark_modified();
                recovery::remove(&file.path);
            }
            Err(message) => self.fail(text::open_failed(), &file.path, message),
        }
    }

    fn autosave_tick(&mut self, ctx: &Context) {
        let savable = self.path.as_deref().is_some_and(files::can_save);
        if !self.autosave || !savable || !self.doc.is_modified() {
            self.dirty_since = None;
            return;
        }
        let now = ctx.input(|i| i.time);
        let since = *self.dirty_since.get_or_insert(now);
        if now - since >= theme::AUTOSAVE_DELAY {
            self.dirty_since = None;
            self.save(false);
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(theme::AUTOSAVE_DELAY));
        }
    }

    pub fn modal_open(&self) -> bool {
        self.dialog.is_some()
    }

    fn sync_title(&mut self, ctx: &Context) {
        let title = format!("{} - {}", self.book, text::app_name());
        if self.title.is_empty() {
            ctx.send_viewport_cmd(ViewportCommand::SetTheme(egui::SystemTheme::Light));
        }
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn handle_close(&mut self, ctx: &Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing && self.doc.is_modified()
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::SavePrompt(Pending::Close));
        }
    }

    pub fn select_all(&mut self) {
        let last = CellAddr::new(MAX_ROWS - 1, MAX_COLS - 1).expect("last cell is valid");
        self.sel = Selection {
            active: self.sel.active,
            anchor: origin(),
            corner: last,
        };
    }

    fn formula_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = theme::CHROME_SPACING;
            let name_id = Id::unique(NAME_BOX_ID);
            if !ctx.memory(|m| m.has_focus(name_id)) {
                self.name_box = self
                    .selection_name()
                    .unwrap_or_else(|| self.sel.active.to_string());
            }
            let name = TextEdit::singleline(&mut self.name_box)
                .id(name_id)
                .font(theme::chrome_font())
                .desired_width(theme::NAME_BOX_WIDTH)
                .background_color(theme::input_bg())
                .show(ui)
                .response;
            if name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.go_to_reference();
            }
            let names: Vec<String> = self
                .doc
                .book()
                .names()
                .iter()
                .filter(|defined| defined.scope.is_none_or(|scope| scope == self.doc.active()))
                .map(|defined| defined.name.clone())
                .collect();
            let list = bar_button(
                ui,
                fluent::CHEVRON_DOWN,
                text::names_in_book(),
                !names.is_empty(),
            );
            egui::Popup::menu(&list).show(|ui| {
                for name in &names {
                    if crate::menu::item(ui, name) {
                        self.go_to_name(name);
                    }
                }
            });
            let editing = self.edit.is_some();
            if bar_button(ui, fluent::DISMISS, text::cancel(), editing).clicked() {
                self.cancel_edit(&ctx);
            }
            if bar_button(ui, fluent::CHECKMARK, text::enter_button(), editing).clicked() {
                self.commit_edit(&ctx);
            }
            if bar_button(ui, fluent::MATH_FORMULA, text::insert_function(), true).clicked() {
                match self.edit.as_mut() {
                    Some(edit) if edit.text.is_empty() => edit.text.push('='),
                    Some(_) => {}
                    None => self.start_edit("=".to_owned(), EditMode::Enter, true),
                }
            }
            let bar_id = Id::unique(FORMULA_BAR_ID);
            let expanded = self.formula_expanded;
            let book = self.doc.book();
            let sheet = self.doc.active();
            let mut layouter = |ui: &Ui, buffer: &dyn egui::TextBuffer, wrap: f32| {
                let text = buffer.as_str();
                let parts = crate::references::colored(text, book, sheet);
                let job = crate::references::job(
                    text,
                    &parts,
                    theme::bar_font(),
                    theme::chrome_text(),
                    wrap,
                );
                ui.fonts_mut(|fonts| fonts.layout_job(job))
            };
            let editing = self.edit.is_some();
            let mut preview = self.active_input();
            let buffer = match &mut self.edit {
                Some(edit) => {
                    if edit.cursor_to_end {
                        cursor_to_end(&ctx, bar_id, &edit.text);
                    }
                    &mut edit.text
                }
                None => &mut preview,
            };
            let width = ui.available_width() - theme::TOOL_SIZE - theme::CHROME_SPACING;
            let editor = if expanded {
                TextEdit::multiline(buffer).desired_rows(theme::EXPANDED_ROWS)
            } else if buffer.contains('\n') {
                TextEdit::multiline(buffer).desired_rows(1)
            } else {
                TextEdit::singleline(buffer)
            };
            let editor = if editing {
                editor.layouter(&mut layouter)
            } else {
                editor
            };
            let bar = editor
                .id(bar_id)
                .event_filter(EDITOR_KEYS)
                .margin(bar_margin(ui))
                .font(theme::bar_font())
                .text_color(theme::chrome_text())
                .desired_width(width)
                .background_color(theme::input_bg())
                .show(ui)
                .response;
            if bar.has_focus() && self.edit.is_some() {
                self.formula_assist(&ctx, bar_id, bar.rect);
            }
            if bar.gained_focus() && self.edit.is_none() {
                self.start_edit(preview, EditMode::Edit, false);
            }
            let (glyph, tip) = if expanded {
                (fluent::CHEVRON_UP, text::collapse_bar())
            } else {
                (fluent::CHEVRON_DOWN, text::expand_bar())
            };
            if bar_button(ui, glyph, tip, true).clicked() {
                self.formula_expanded = !expanded;
            }
        });
    }

    fn go_to_reference(&mut self) {
        let typed = self.name_box.trim().to_owned();
        if self.go_to_name(&typed) {
            return;
        }
        if vellum_grid_core::is_valid_name(&typed) {
            let range = self.selected();
            let formula = self.doc.absolute_reference(range.start(), range.end());
            if let Err(error) = self
                .doc
                .define_name(None, vellum_grid_core::DefinedName::new(&typed, &formula))
            {
                self.dialog = Some(Dialog::Error {
                    title: text::app_name(),
                    message: crate::names::error_text(&error).to_owned(),
                });
            }
            return;
        }
        match typed.parse::<CellRange>() {
            Ok(range) => {
                self.select(Selection {
                    active: range.start(),
                    anchor: range.start(),
                    corner: range.end(),
                });
                self.view.reveal(range.start());
            }
            Err(e) => {
                self.dialog = Some(Dialog::Error {
                    title: text::bad_reference(),
                    message: e.to_string(),
                });
            }
        }
    }
}

impl eframe::App for Spreadsheet {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.prefs
            .apply(ctx.system_theme() == Some(egui::Theme::Dark));
        if self.styled != Some(vellum_ui::dark()) {
            vellum_ui::theme::apply(&ctx);
            self.styled = Some(vellum_ui::dark());
        }
        titlebar::resize_edges(&ctx);
        self.handle_close(&ctx);
        self.autosave_tick(&ctx);
        self.recovery_tick(&ctx);
        if let Some((path, bytes)) = self.inbox.poll() {
            self.open_bytes(path, bytes);
        }
        if let Some(bytes) = self.inbox.poll_image() {
            self.insert_picture(bytes);
        }
        self.sync_title(&ctx);
        if self.backstage.is_some() {
            if !self.modal_open() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.backstage = None;
            }
        } else if !self.modal_open() {
            self.handle_keys(&ctx);
        }
        let inset = titlebar::maximized_inset(&ctx);
        if inset > 0.0 {
            Frame::NONE
                .fill(theme::chrome_bg())
                .inner_margin(Margin::same(inset as i8))
                .show(ui, |ui| self.layout(ui));
        } else {
            self.layout(ui);
        }
        self.filter_menu_ui(&ctx);
        self.find_ui(&ctx);
        self.dialogs(&ctx);
        if self.doc.take_blocked() && self.dialog.is_none() {
            self.dialog = Some(Dialog::Error {
                title: text::app_name(),
                message: text::protected_cell().to_owned(),
            });
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.drop_recovery();
    }
}

impl Spreadsheet {
    fn layout(&mut self, ui: &mut Ui) {
        let chrome = Frame::NONE
            .fill(theme::chrome_bg())
            .inner_margin(Margin::symmetric(theme::CHROME_SPACING as i8, 0));
        egui::Panel::top("title-bar")
            .exact_size(theme::TITLE_BAR_HEIGHT)
            .frame(Frame::NONE.fill(theme::chrome_bg()))
            .show(ui, |ui| self.title_bar(ui));
        let shown = vellum_ui::motion::reveal(
            ui.ctx(),
            Id::unique("backstage"),
            self.backstage.is_some(),
            theme::BACKSTAGE_ANIMATION,
        );
        if let Some(page) = self.backstage {
            self.backstage(ui, page, shown);
            return;
        }
        egui::Panel::top("ribbon-tabs")
            .exact_size(theme::RIBBON_TABS_HEIGHT)
            .frame(chrome)
            .show(ui, |ui| self.ribbon_tabs(ui));
        if !self.ribbon_collapsed {
            egui::Panel::top("ribbon")
                .exact_size(theme::RIBBON_HEIGHT)
                .frame(Frame::NONE.fill(theme::chrome_bg()).inner_margin(Margin {
                    left: theme::RIBBON_MARGIN,
                    right: theme::RIBBON_MARGIN,
                    top: 0,
                    bottom: theme::RIBBON_MARGIN,
                }))
                .show(ui, |ui| self.ribbon(ui));
        }
        if self.show_formula_bar {
            let height = if self.formula_expanded {
                theme::FORMULA_BAR_EXPANDED
            } else {
                theme::FORMULA_BAR_HEIGHT
            };
            egui::Panel::top("formula-bar")
                .exact_size(height)
                .frame(chrome)
                .show(ui, |ui| self.formula_bar(ui));
        }
        egui::Panel::bottom("status-bar")
            .exact_size(theme::STATUS_BAR_HEIGHT)
            .frame(chrome)
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("sheet-tabs")
            .exact_size(theme::SHEET_TABS_HEIGHT)
            .frame(Frame::NONE.fill(theme::chrome_bg()))
            .show(ui, |ui| self.sheet_tabs(ui));
        if self.comments_pane {
            egui::Panel::right("comments-pane")
                .default_size(theme::COMMENTS_PANE_WIDTH)
                .resizable(true)
                .frame(
                    Frame::NONE
                        .fill(theme::chrome_bg())
                        .inner_margin(Margin::same(theme::DIALOG_MARGIN / 2)),
                )
                .show(ui, |ui| self.comments_pane(ui));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.grid_ui(ui));
    }
}

fn blank_book() -> Workbook {
    Workbook::from_sheets(vec![Sheet::new(text::new_sheet())]).expect("default sheet name is valid")
}

fn new_document(book: Workbook) -> Document {
    let mut doc = Document::with_locale(book, locale());
    doc.set_now(clock::now());
    doc
}

pub const EDITOR_KEYS: egui::EventFilter = egui::EventFilter {
    tab: true,
    horizontal_arrows: true,
    vertical_arrows: true,
    escape: true,
};

pub fn editor_cursor(ctx: &Context, id: Id, text: &str) -> (usize, usize) {
    let end = text.chars().count();
    egui::text_edit::TextEditState::load(ctx, id)
        .and_then(|state| state.cursor.char_range())
        .map_or((end, end), |range| {
            let (a, b): (usize, usize) = (range.primary.index.into(), range.secondary.index.into());
            (a.min(b).min(end), a.max(b).min(end))
        })
}

pub fn select_chars(ctx: &Context, id: Id, start: usize, end: usize) {
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    state.store(ctx, id);
}

pub fn cursor_to_end(ctx: &Context, id: Id, text: &str) {
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let end = egui::text::CCursor::new(text.chars().count());
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(end)));
    state.store(ctx, id);
}

pub fn origin() -> CellAddr {
    CellAddr::new(0, 0).expect("A1 is valid")
}

fn target(origin: CellAddr, row: u32, col: u32) -> Option<CellAddr> {
    CellAddr::new(origin.row() + row, origin.col() + col).ok()
}

fn release_focus(ctx: &Context) {
    ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
}

fn bar_margin(ui: &Ui) -> Margin {
    let font = theme::bar_font();
    let size = font.size;
    let galley = ui
        .painter()
        .layout_no_wrap("H".into(), font, theme::chrome_text());
    let baseline = galley
        .rows
        .first()
        .and_then(|row| row.row.glyphs.first().map(|glyph| row.pos.y + glyph.pos.y))
        .unwrap_or(size);
    let centered = galley.size().y / 2.0 + size * theme::CAP_HEIGHT / 2.0;
    let shift = (centered - baseline).round() as i8;
    let padding = ui.spacing().button_padding;
    Margin {
        left: padding.x as i8,
        right: padding.x as i8,
        top: padding.y as i8 + shift,
        bottom: padding.y as i8 - shift,
    }
}

fn bar_button(ui: &mut Ui, glyph: fluent::Glyph, tip: &str, enabled: bool) -> egui::Response {
    let size = egui::vec2(theme::TOOL_SIZE, theme::TOOL_SIZE);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if enabled && response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(3), theme::tab_hover());
    }
    let area = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(theme::STATUS_GLYPH, theme::STATUS_GLYPH),
    );
    let icon = icons::Icon::Plain(glyph);
    if enabled {
        icons::paint(ui.painter(), area, icon, theme::icon(), None);
    } else {
        icons::paint_muted(ui.painter(), area, icon, theme::icon_disabled());
    }
    response.on_hover_text(tip)
}
