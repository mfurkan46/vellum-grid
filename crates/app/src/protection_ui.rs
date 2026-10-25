use eframe::egui::{RichText, TextEdit, Ui};
use vellum_grid_core::protect::{PasswordHash, Permissions, Protection};

use crate::app::Spreadsheet;
use crate::dialogs::Dialog;
use crate::{text, theme};

#[derive(Debug, Clone)]
pub struct ProtectForm {
    pub book: bool,
    pub password: String,
    pub confirm: String,
    pub allow: Permissions,
    pub error: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct UnprotectForm {
    pub book: bool,
    pub password: String,
    pub wrong: bool,
}

fn password_field(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(
        TextEdit::singleline(value)
            .password(true)
            .desired_width(f32::INFINITY)
            .background_color(theme::input_bg()),
    );
}

pub fn protect_ui(ui: &mut Ui, form: &mut ProtectForm) {
    ui.set_width(theme::GO_TO_WIDTH);
    if form.book {
        ui.label(text::protect_structure());
    } else {
        ui.label(text::protect_sheet_intro());
    }
    password_field(ui, text::password_optional(), &mut form.password);
    password_field(ui, text::password_confirm(), &mut form.confirm);
    if !form.book {
        ui.label(text::allow_all_users());
        crate::dialogs::list_box(ui, |ui| {
            let allow = &mut form.allow;
            for (value, label) in [
                (&mut allow.select_locked, text::allow_select_locked()),
                (&mut allow.select_unlocked, text::allow_select_unlocked()),
                (&mut allow.format_cells, text::allow_format_cells()),
                (&mut allow.format_columns, text::allow_format_columns()),
                (&mut allow.format_rows, text::allow_format_rows()),
                (&mut allow.insert_columns, text::allow_insert_columns()),
                (&mut allow.insert_rows, text::allow_insert_rows()),
                (&mut allow.insert_links, text::allow_insert_links()),
                (&mut allow.delete_columns, text::allow_delete_columns()),
                (&mut allow.delete_rows, text::allow_delete_rows()),
                (&mut allow.sort, text::allow_sort()),
                (&mut allow.autofilter, text::allow_autofilter()),
                (&mut allow.edit_objects, text::allow_edit_objects()),
            ] {
                ui.checkbox(value, label);
            }
        });
    }
    if let Some(error) = form.error {
        ui.label(RichText::new(error).color(theme::error_text()));
    }
}

pub fn unprotect_ui(ui: &mut Ui, form: &mut UnprotectForm) {
    ui.set_width(theme::GO_TO_WIDTH);
    let input = ui.add(
        TextEdit::singleline(&mut form.password)
            .password(true)
            .hint_text(text::password())
            .desired_width(f32::INFINITY)
            .background_color(theme::input_bg()),
    );
    if !input.has_focus() && form.password.is_empty() {
        input.request_focus();
    }
    if form.wrong {
        ui.label(RichText::new(text::wrong_password()).color(theme::error_text()));
    }
}

impl Spreadsheet {
    pub fn toggle_sheet_protection(&mut self) {
        match &self.sheet().layout().protection {
            Some(protection) if protection.password.is_some() => {
                self.dialog = Some(Dialog::Unprotect(UnprotectForm {
                    book: false,
                    password: String::new(),
                    wrong: false,
                }));
            }
            Some(_) => {
                self.doc.unprotect_sheet("");
            }
            None => {
                self.dialog = Some(Dialog::Protect(ProtectForm {
                    book: false,
                    password: String::new(),
                    confirm: String::new(),
                    allow: Permissions::default(),
                    error: None,
                }));
            }
        }
    }

    pub fn toggle_book_protection(&mut self) {
        match &self.doc.book().protection {
            Some(Some(_)) => {
                self.dialog = Some(Dialog::Unprotect(UnprotectForm {
                    book: true,
                    password: String::new(),
                    wrong: false,
                }));
            }
            Some(None) => {
                self.doc.unprotect_book("");
            }
            None => {
                self.dialog = Some(Dialog::Protect(ProtectForm {
                    book: true,
                    password: String::new(),
                    confirm: String::new(),
                    allow: Permissions::default(),
                    error: None,
                }));
            }
        }
    }

    pub fn apply_protect(&mut self, mut form: ProtectForm) {
        if form.password != form.confirm {
            form.error = Some(text::passwords_differ());
            form.confirm.clear();
            self.dialog = Some(Dialog::Protect(form));
            return;
        }
        let password = PasswordHash::of(&form.password);
        if form.book {
            self.doc.protect_book(password);
        } else {
            self.doc.protect_sheet(Some(Protection {
                password,
                allow: form.allow,
            }));
        }
    }

    pub fn apply_unprotect(&mut self, mut form: UnprotectForm) {
        let done = if form.book {
            self.doc.unprotect_book(&form.password)
        } else {
            self.doc.unprotect_sheet(&form.password)
        };
        if !done {
            form.wrong = true;
            form.password.clear();
            self.dialog = Some(Dialog::Unprotect(form));
        }
    }

    pub fn structure_blocked(&mut self) -> bool {
        if !self.doc.structure_locked() {
            return false;
        }
        self.dialog = Some(Dialog::Error {
            title: text::app_name(),
            message: text::structure_locked().to_owned(),
        });
        true
    }

    pub fn locked_blocked(&mut self) -> bool {
        if !self.doc.is_locked(self.sel.active) {
            return false;
        }
        self.dialog = Some(Dialog::Error {
            title: text::app_name(),
            message: text::protected_cell().to_owned(),
        });
        true
    }

    pub fn toggle_lock(&mut self) {
        let locked = self.active_style().locked;
        self.restyle(|_, style| style.locked = !locked);
    }
}
