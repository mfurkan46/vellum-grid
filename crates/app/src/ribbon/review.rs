use eframe::egui::{Popup, Ui};

use super::widgets::{big, big_checked, group, small, tip};
use crate::app::Spreadsheet;
use crate::icons::Icon;
use crate::{fluent, menu, text, theme};

impl Spreadsheet {
    pub(super) fn review(&mut self, ui: &mut Ui) {
        group(ui, text::proofing(), |ui| {
            let icon = Icon::Plain(fluent::TEXT_PROOFING);
            let hint = tip(text::spelling(), "F7");
            if big(ui, icon, text::spelling(), &hint, false).clicked() {
                self.open_spelling();
            }
        });
        group(ui, text::comments(), |ui| {
            ui.horizontal_top(|ui| {
                let icon = Icon::Tinted(fluent::COMMENT_ADD, theme::tint_blue());
                let hint = tip(text::new_comment(), "Ctrl+Alt+M");
                if big(ui, icon, text::new_comment(), &hint, false).clicked() {
                    self.open_comment(self.sel.active);
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let addr = self.sel.active;
                    let has = self.sheet().layout().comments.contains_key(&addr);
                    let icon = Icon::Plain(fluent::DISMISS);
                    let label = text::delete_comment();
                    if ui
                        .add_enabled_ui(has, |ui| small(ui, icon, label, label, false))
                        .inner
                        .clicked()
                    {
                        self.doc.set_layout(|layout| {
                            layout.comments.remove(&addr);
                        });
                    }
                    let label = text::previous_comment();
                    if small(ui, Icon::Plain(fluent::CHEVRON_UP), label, label, false).clicked() {
                        self.step_review(false, false);
                    }
                    let label = text::next_comment();
                    if small(ui, Icon::Plain(fluent::CHEVRON_DOWN), label, label, false).clicked() {
                        self.step_review(false, true);
                    }
                });
                let icon = Icon::Plain(fluent::COMMENT);
                let label = text::show_comments();
                if big_checked(ui, icon, label, label, self.comments_pane).clicked() {
                    self.comments_pane = !self.comments_pane;
                }
            });
        });
        group(ui, text::notes(), |ui| {
            let icon = Icon::Tinted(fluent::NOTE_ADD, theme::tint_yellow());
            let button = big(ui, icon, text::notes(), text::notes(), true);
            Popup::menu(&button).show(|ui| {
                let has = self.sheet().layout().notes.contains_key(&self.sel.active);
                let label = if has {
                    text::edit_note()
                } else {
                    text::new_note()
                };
                if menu::shortcut_item(ui, None, label, "Shift+F2") {
                    self.edit_note();
                }
                if menu::item(ui, text::previous_note()) {
                    self.step_review(true, false);
                }
                if menu::item(ui, text::next_note()) {
                    self.step_review(true, true);
                }
                if menu::enabled_item(ui, has, None, text::show_hide_note()) {
                    self.toggle_note();
                }
                let mut all = self.show_all_notes;
                menu::check(ui, &mut all, text::show_all_notes());
                self.show_all_notes = all;
                if menu::item(ui, text::notes_to_comments()) {
                    self.notes_to_comments();
                }
            });
        });
        group(ui, text::protect(), |ui| {
            ui.horizontal_top(|ui| {
                let sheet = if self.doc.is_protected() {
                    text::unprotect_sheet()
                } else {
                    text::protect_sheet()
                };
                let icon = Icon::Plain(fluent::LOCK_CLOSED);
                if big(ui, icon, sheet, sheet, false).clicked() {
                    self.toggle_sheet_protection();
                }
                let book = if self.doc.structure_locked() {
                    text::unprotect_workbook()
                } else {
                    text::protect_workbook()
                };
                let protected = self.doc.structure_locked();
                if big_checked(ui, Icon::Plain(fluent::SHIELD), book, book, protected).clicked() {
                    self.toggle_book_protection();
                }
            });
        });
    }
}
