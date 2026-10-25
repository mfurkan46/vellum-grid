use eframe::egui::{self, Context, CornerRadius, Id, Key, Rect, Sense, TextEdit, Ui, pos2, vec2};
use vellum_grid_core::WorkbookError;

use crate::app::{Selection, Spreadsheet, View, origin};
use crate::dialogs::Dialog;
use crate::icons::{self, Icon};
use crate::ribbon::widgets::palette_ui;
use crate::{fluent, menu, render, text, theme};

#[derive(Debug)]
pub struct Rename {
    pub index: usize,
    pub name: String,
    pub focus: bool,
}

enum TabAction {
    Insert,
    Delete(usize),
    Rename(usize),
    MoveCopy(usize),
    Color(usize, Option<vellum_grid_core::Color>),
    Hide(usize),
    Unhide,
    Protect(usize),
}

impl Spreadsheet {
    pub fn sheet_tabs(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut clicked = None;
        let mut action = None;
        let mut rects = Vec::new();
        let visible = self.doc.visible_sheets();
        let any_hidden = visible.len() < self.doc.book().sheets().len();
        self.tab_scroll = self.tab_scroll.min(visible.len().saturating_sub(1));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(theme::CHROME_SPACING / 2.0);
            for (glyph, step) in [(fluent::CHEVRON_LEFT, -1), (fluent::CHEVRON_RIGHT, 1)] {
                if nav_button(ui, glyph).clicked() {
                    let next = self.tab_scroll as i64 + step;
                    self.tab_scroll = next.clamp(0, visible.len() as i64 - 1) as usize;
                }
            }
            ui.add_space(theme::CHROME_SPACING / 2.0);
            for &index in visible.iter().skip(self.tab_scroll) {
                if ui.available_width() < theme::TOOL_SIZE * 2.0 {
                    break;
                }
                if let Some(rect) = self.rename_field(ui, &ctx, index) {
                    rects.push((index, rect));
                    continue;
                }
                let sheet = &self.doc.book().sheets()[index];
                let name = sheet.name().to_owned();
                let tab_color = sheet.layout().tab_color.map(render::color);
                let active = index == self.doc.active();
                let (font, color) = match (active, tab_color) {
                    (true, None) => (theme::tab_font_bold(), theme::accent()),
                    (true, Some(_)) => (theme::tab_font_bold(), theme::chrome_text()),
                    (false, Some(fill)) => (theme::chrome_font(), theme::text_on(fill)),
                    (false, None) => (theme::chrome_font(), theme::chrome_text()),
                };
                let width = ui
                    .painter()
                    .layout_no_wrap(name.clone(), theme::tab_font_bold(), color)
                    .size()
                    .x;
                let galley = ui.painter().layout_no_wrap(name, font, color);
                let size = vec2(
                    width + 2.0 * theme::SHEET_TAB_PADDING,
                    ui.available_height(),
                );
                let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
                let painter = ui.painter();
                let tab = rect.shrink2(vec2(0.0, 2.0));
                let radius = CornerRadius::same(4);
                match (active, tab_color) {
                    (true, None) => {
                        painter.rect_filled(tab, radius, theme::surface());
                        let bar = Rect::from_center_size(
                            pos2(tab.center().x, tab.bottom() - theme::RIBBON_TAB_MARKER),
                            vec2(galley.size().x, theme::RIBBON_TAB_MARKER),
                        );
                        painter.rect_filled(bar, CornerRadius::same(2), theme::accent());
                    }
                    (true, Some(fill)) => {
                        let tint = theme::surface().lerp_to_gamma(fill, theme::TAB_COLOR_TINT);
                        painter.rect_filled(tab, radius, tint);
                        let bar = Rect::from_min_max(
                            pos2(tab.left(), tab.bottom() - theme::TAB_COLOR_BAR),
                            tab.right_bottom(),
                        );
                        let corners = CornerRadius {
                            sw: radius.sw,
                            se: radius.se,
                            ..CornerRadius::ZERO
                        };
                        painter.rect_filled(bar, corners, fill);
                    }
                    (false, Some(fill)) => {
                        let fill = if response.hovered() {
                            fill.lerp_to_gamma(theme::text_on(fill), theme::TAB_COLOR_HOVER)
                        } else {
                            fill
                        };
                        painter.rect_filled(tab, radius, fill);
                    }
                    (false, None) => {
                        let hover = vellum_ui::motion::hover_color(
                            ui,
                            &response,
                            response.hovered(),
                            theme::tab_hover(),
                        );
                        painter.rect_filled(tab, radius, hover);
                    }
                }
                let lift = if active && tab_color.is_some() {
                    theme::TAB_COLOR_BAR / 2.0
                } else {
                    0.0
                };
                let at = tab.center() - galley.size() / 2.0 - vec2(0.0, lift);
                painter.galley(at, galley, color);
                if response.clicked() || response.secondary_clicked() || response.drag_started() {
                    clicked = Some(index);
                }
                if response.drag_started() {
                    self.tab_drag = Some(index);
                }
                if response.double_clicked() {
                    action = Some(TabAction::Rename(index));
                }
                response.context_menu(|ui| {
                    ui.set_min_width(theme::CONTEXT_MENU_WIDTH);
                    if menu::icon_item(ui, Icon::Plain(fluent::TAB_ADD), text::insert_sheet()) {
                        action = Some(TabAction::Insert);
                    }
                    if menu::icon_item(ui, Icon::Plain(fluent::DELETE), text::delete_sheet()) {
                        action = Some(TabAction::Delete(index));
                    }
                    if menu::icon_item(ui, Icon::Plain(fluent::RENAME), text::rename()) {
                        action = Some(TabAction::Rename(index));
                    }
                    let copy = Icon::Plain(fluent::DOCUMENT_COPY);
                    if menu::icon_item(ui, copy, text::move_copy()) {
                        action = Some(TabAction::MoveCopy(index));
                    }
                    let lock = Icon::Plain(fluent::LOCK_CLOSED);
                    let label = if self.doc.is_protected() && index == self.doc.active() {
                        text::unprotect_sheet_menu()
                    } else {
                        text::protect_sheet_menu()
                    };
                    if menu::icon_item(ui, lock, label) {
                        action = Some(TabAction::Protect(index));
                    }
                    menu::sub(
                        ui,
                        Some(Icon::Plain(fluent::COLOR)),
                        text::tab_color(),
                        |ui| {
                            if let Some(choice) = palette_ui(ui, text::no_color()) {
                                action = Some(TabAction::Color(index, choice));
                            }
                        },
                    );
                    menu::separator(ui);
                    if menu::icon_item(ui, Icon::Plain(fluent::EYE_OFF), text::hide()) {
                        action = Some(TabAction::Hide(index));
                    }
                    if ui
                        .add_enabled_ui(any_hidden, |ui| {
                            menu::icon_item(ui, Icon::Plain(fluent::EYE), text::unhide())
                        })
                        .inner
                    {
                        action = Some(TabAction::Unhide);
                    }
                });
                rects.push((index, rect));
            }
            ui.add_space(theme::CHROME_SPACING / 2.0);
            let size = vec2(theme::TOOL_SIZE, theme::TOOL_SIZE);
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(4), theme::tab_hover());
            }
            let area =
                Rect::from_center_size(rect.center(), vec2(theme::ICON_SIZE, theme::ICON_SIZE));
            icons::paint(
                ui.painter(),
                area,
                Icon::Plain(fluent::ADD),
                theme::icon(),
                None,
            );
            if response
                .on_hover_text(format!("{} (Shift+F11)", text::new_sheet_tip()))
                .clicked()
            {
                action = Some(TabAction::Insert);
            }
        });

        if let Some(index) = clicked {
            self.activate(&ctx, index);
        }
        self.drag_tab(ui, &rects);
        match action {
            Some(TabAction::Insert) => self.insert_sheet(&ctx),
            Some(TabAction::Delete(index)) => self.delete_sheet(index),
            Some(TabAction::Rename(index)) => self.start_rename(index),
            Some(TabAction::MoveCopy(index)) => {
                self.dialog = Some(Dialog::MoveCopy {
                    index,
                    before: None,
                    copy: false,
                });
            }
            Some(TabAction::Color(index, color)) => {
                self.doc.set_tab_color(index, color);
            }
            Some(TabAction::Hide(index)) => self.hide_sheet(index),
            Some(TabAction::Protect(index)) => {
                self.activate(&ctx, index);
                self.toggle_sheet_protection();
            }
            Some(TabAction::Unhide) => {
                let hidden: Vec<usize> = (0..self.doc.book().sheets().len())
                    .filter(|i| !visible.contains(i))
                    .collect();
                if let Some(&first) = hidden.first() {
                    self.dialog = Some(Dialog::Unhide { selected: first });
                }
            }
            None => {}
        }
    }

    fn rename_field(&mut self, ui: &mut Ui, ctx: &Context, index: usize) -> Option<Rect> {
        let rename = self.renaming.as_mut().filter(|r| r.index == index)?;
        let width = ui
            .painter()
            .layout_no_wrap(
                rename.name.clone(),
                theme::chrome_font(),
                theme::chrome_text(),
            )
            .size()
            .x
            .max(theme::RENAME_MIN_WIDTH)
            + 2.0 * theme::SHEET_TAB_PADDING;
        let size = vec2(width, ui.available_height());
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, theme::surface());
        let id = Id::unique("sheet-rename");
        let response = ui.put(
            rect.shrink2(vec2(theme::SHEET_TAB_PADDING / 2.0, 4.0)),
            TextEdit::singleline(&mut rename.name)
                .id(id)
                .font(theme::chrome_font()),
        );
        if rename.focus {
            response.request_focus();
            select_all_text(ctx, id, &rename.name);
            rename.focus = false;
        }
        if response.lost_focus() {
            let cancel = ctx.input(|i| i.key_pressed(Key::Escape));
            self.finish_rename(!cancel);
        }
        Some(rect)
    }

    fn drag_tab(&mut self, ui: &Ui, rects: &[(usize, Rect)]) {
        let Some(from) = self.tab_drag else {
            return;
        };
        let (down, pointer) = ui.input(|i| (i.pointer.primary_down(), i.pointer.interact_pos()));
        let target = pointer.and_then(|p| {
            rects
                .iter()
                .find(|(_, rect)| p.x < rect.center().x)
                .map(|(index, rect)| (*index, rect.left()))
                .or_else(|| rects.last().map(|(index, rect)| (index + 1, rect.right())))
        });
        let moved = target.is_some_and(|(slot, _)| slot != from && slot != from + 1);
        if down {
            if let Some((_, x)) = target.filter(|_| moved) {
                let top = rects.first().map_or(0.0, |(_, r)| r.top());
                let marker = Rect::from_min_size(pos2(x - 1.0, top), vec2(2.0, theme::TOOL_SIZE));
                ui.painter()
                    .rect_filled(marker, CornerRadius::ZERO, theme::accent());
            }
            return;
        }
        self.tab_drag = None;
        if let Some((slot, _)) = target.filter(|_| moved && !self.structure_blocked()) {
            let to = if slot > from { slot - 1 } else { slot };
            self.doc.move_sheet(from, to);
            self.views.clear();
        }
    }

    pub fn activate_step(&mut self, ctx: &Context, step: i64) {
        let visible = self.doc.visible_sheets();
        let Some(position) = visible.iter().position(|&i| i == self.doc.active()) else {
            return;
        };
        let next = position as i64 + step;
        if let Some(&index) = usize::try_from(next).ok().and_then(|n| visible.get(n)) {
            self.activate(ctx, index);
        }
    }

    pub fn insert_sheet(&mut self, ctx: &Context) {
        if self.structure_blocked() || !self.commit_edit(ctx) {
            return;
        }
        self.doc.add_sheet(text::new_sheet_prefix());
        self.reset_views();
    }

    pub fn delete_sheet(&mut self, index: usize) {
        if self.structure_blocked() {
            return;
        }
        if self.doc.visible_sheets().len() == 1 {
            self.dialog = Some(Dialog::Error {
                title: text::app_name(),
                message: text::last_sheet().to_owned(),
            });
        } else if self.doc.book().sheets()[index].is_empty() {
            self.delete_sheet_now(index);
        } else {
            self.dialog = Some(Dialog::DeleteSheet(index));
        }
    }

    pub fn delete_sheet_now(&mut self, index: usize) {
        if let Err(error) = self.doc.remove_sheet(index) {
            self.sheet_error(&error);
        }
        self.reset_views();
        if self.sheet().layout().sheet_hidden
            && let Some(&first) = self.doc.visible_sheets().first()
        {
            self.doc.set_active(first);
        }
    }

    fn hide_sheet(&mut self, index: usize) {
        if self.structure_blocked() {
            return;
        }
        if !self.doc.set_sheet_hidden(index, true) {
            self.dialog = Some(Dialog::Error {
                title: text::app_name(),
                message: text::last_sheet().to_owned(),
            });
            return;
        }
        self.reset_views();
    }

    pub fn unhide_sheet(&mut self, index: usize) {
        if self.structure_blocked() {
            return;
        }
        self.doc.set_sheet_hidden(index, false);
        self.reset_views();
    }

    pub fn move_or_copy(&mut self, index: usize, before: Option<usize>, copy: bool) {
        if self.structure_blocked() {
            return;
        }
        let end = self.doc.book().sheets().len();
        let slot = before.unwrap_or(end);
        if copy {
            self.doc.copy_sheet(index, slot);
        } else {
            let to = if slot > index { slot - 1 } else { slot };
            self.doc.move_sheet(index, to);
        }
        self.reset_views();
    }

    pub fn start_rename(&mut self, index: usize) {
        if self.structure_blocked() {
            return;
        }
        self.renaming = Some(Rename {
            index,
            name: self.doc.book().sheets()[index].name().to_owned(),
            focus: true,
        });
    }

    fn finish_rename(&mut self, apply: bool) {
        let Some(rename) = self.renaming.take() else {
            return;
        };
        let current = self.doc.book().sheets()[rename.index].name();
        if !apply || rename.name == current {
            return;
        }
        if let Err(error) = self.doc.rename_sheet(rename.index, rename.name.trim()) {
            self.sheet_error(&error);
        }
    }

    fn sheet_error(&mut self, error: &WorkbookError) {
        let message = match error {
            WorkbookError::DuplicateSheetName(_) => text::duplicate_sheet_name(),
            WorkbookError::LastSheet => text::last_sheet(),
            _ => text::bad_sheet_name(),
        };
        self.dialog = Some(Dialog::Error {
            title: text::app_name(),
            message: message.to_owned(),
        });
    }

    fn reset_views(&mut self) {
        self.views.clear();
        self.edit = None;
        self.clipboard = None;
        self.object = None;
        self.shape_edit = None;
        self.sel = Selection::at(origin());
        self.view = View::default();
    }
}

fn nav_button(ui: &mut Ui, glyph: fluent::Glyph) -> egui::Response {
    let size = vec2(theme::NAV_BUTTON, theme::TOOL_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), theme::tab_hover());
    }
    let area = Rect::from_center_size(rect.center(), vec2(theme::NAV_GLYPH, theme::NAV_GLYPH));
    icons::paint(
        ui.painter(),
        area,
        Icon::Plain(glyph),
        theme::chrome_text_muted(),
        None,
    );
    response
}

fn select_all_text(ctx: &Context, id: Id, text: &str) {
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let end = egui::text::CCursor::new(text.chars().count());
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            end,
        )));
    state.store(ctx, id);
}
