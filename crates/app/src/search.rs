use eframe::egui::{self, Align, Align2, ComboBox, Context, Key, Layout, TextEdit, Window, vec2};
use vellum_grid_core::find::{self, FindOptions};

use crate::app::{Selection, Spreadsheet, locale};
use crate::dialogs::{plain_button, roomy};
use crate::{text, theme};

#[derive(Debug, Default)]
pub struct FindState {
    replace: bool,
    query: String,
    replacement: String,
    options: FindOptions,
    message: Option<String>,
    focus: bool,
}

enum Action {
    Next,
    Replace,
    ReplaceAll,
}

impl Spreadsheet {
    pub fn open_find(&mut self, replace: bool) {
        let mut state = self.find.take().unwrap_or_default();
        state.replace = replace;
        state.focus = true;
        state.message = None;
        self.find = Some(state);
    }

    pub fn find_ui(&mut self, ctx: &Context) {
        let Some(mut state) = self.find.take() else {
            return;
        };
        let mut open = true;
        let mut action = None;
        Window::new(text::find_replace())
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .default_width(theme::FIND_WIDTH)
            .pivot(Align2::CENTER_TOP)
            .default_pos(ctx.content_rect().center_top() + vec2(0.0, theme::FIND_TOP))
            .show(ctx, |ui| {
                roomy(ui);
                ui.horizontal(|ui| {
                    if ui.selectable_label(!state.replace, text::find()).clicked() {
                        state.replace = false;
                    }
                    if ui
                        .selectable_label(state.replace, text::replace())
                        .clicked()
                    {
                        state.replace = true;
                    }
                });
                ui.separator();
                egui::Grid::new("find-fields")
                    .num_columns(2)
                    .spacing([theme::CHROME_SPACING, theme::CHROME_SPACING])
                    .show(ui, |ui| {
                        ui.label(text::find_what());
                        let query = ui.add(
                            TextEdit::singleline(&mut state.query)
                                .desired_width(theme::FIND_INPUT_WIDTH)
                                .background_color(theme::input_bg()),
                        );
                        if state.focus {
                            query.request_focus();
                            state.focus = false;
                        }
                        if query.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            action = Some(Action::Next);
                            query.request_focus();
                        }
                        ui.end_row();
                        if state.replace {
                            ui.label(text::replace_with());
                            ui.add(
                                TextEdit::singleline(&mut state.replacement)
                                    .desired_width(theme::FIND_INPUT_WIDTH)
                                    .background_color(theme::input_bg()),
                            );
                            ui.end_row();
                        }
                    });
                ui.checkbox(&mut state.options.match_case, text::match_case());
                ui.checkbox(&mut state.options.whole_cell, text::whole_cell());
                if !state.replace {
                    ui.horizontal(|ui| {
                        ui.label(text::look_in());
                        let shown = if state.options.values {
                            text::look_values()
                        } else {
                            text::look_formulas()
                        };
                        ComboBox::from_id_salt("find-look-in")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut state.options.values,
                                    false,
                                    text::look_formulas(),
                                );
                                ui.selectable_value(
                                    &mut state.options.values,
                                    true,
                                    text::look_values(),
                                );
                            });
                    });
                }
                if let Some(message) = &state.message {
                    ui.label(message.as_str());
                }
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if ui.add(plain_button(text::find_next())).clicked() {
                        action = Some(Action::Next);
                    }
                    if state.replace {
                        if ui.add(plain_button(text::replace())).clicked() {
                            action = Some(Action::Replace);
                        }
                        if ui.add(plain_button(text::replace_all())).clicked() {
                            action = Some(Action::ReplaceAll);
                        }
                    }
                });
            });
        let options = FindOptions {
            values: state.options.values && !state.replace,
            ..state.options
        };
        match action {
            Some(Action::Next) => state.message = self.find_next(&state.query, options),
            Some(Action::Replace) => {
                let active = self.sel.active;
                let input = self.doc.input_text(active);
                if find::cell_matches(&input, &state.query, options) {
                    let replaced =
                        find::replace_text(&input, &state.query, &state.replacement, options);
                    let cell = self.doc.styled_input(active, &replaced);
                    self.doc.edit([(active, cell)]);
                }
                state.message = self.find_next(&state.query, options);
            }
            Some(Action::ReplaceAll) => {
                let count = self
                    .doc
                    .replace_all(&state.query, &state.replacement, options);
                state.message = Some(if count == 0 {
                    text::not_found().to_owned()
                } else {
                    format!(
                        "{} {count} {}",
                        text::replaced_prefix(),
                        text::replaced_suffix()
                    )
                });
            }
            None => {}
        }
        if open && !ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.find = Some(state);
        }
    }

    fn find_next(&mut self, query: &str, options: FindOptions) -> Option<String> {
        let found = find::find_next(self.sheet(), self.sel.active, query, options, locale());
        match found {
            Some(addr) => {
                self.select(Selection::at(addr));
                None
            }
            None => Some(text::not_found().to_owned()),
        }
    }
}
