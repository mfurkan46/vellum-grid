use eframe::egui::{self, Context, Event, Id, Key, Modifiers};
use vellum_grid_core::{CellAddr, Direction, MAX_COLS, MAX_ROWS};

use crate::app::{EditMode, FORMULA_BAR_ID, Pending, Selection, Spreadsheet, origin};

impl Spreadsheet {
    pub fn handle_keys(&mut self, ctx: &Context) {
        if egui::Popup::is_any_open(ctx) {
            return;
        }
        let focused = ctx.memory(|m| m.focused());
        let editor = self.edit.as_ref().map(|edit| edit.id);
        let bar = Id::unique(FORMULA_BAR_ID);
        let ours =
            focused.is_none() || focused == editor || (focused == Some(bar) && editor.is_some());
        if !ours {
            return;
        }
        let in_bar = focused == Some(bar);
        let events = ctx.input_mut(|i| std::mem::take(&mut i.events));
        let kept = events
            .into_iter()
            .filter(|event| !self.handle_event(ctx, event, in_bar))
            .collect();
        ctx.input_mut(|i| i.events = kept);
    }

    fn handle_event(&mut self, ctx: &Context, event: &Event, in_bar: bool) -> bool {
        if self.edit.is_some() {
            return self.handle_edit_event(ctx, event, in_bar);
        }
        match event {
            Event::Text(text) if let Some((index, _)) = self.selected_shape() => {
                self.begin_shape_text(index);
                if let Some(edit) = self.shape_edit.as_mut() {
                    edit.text.push_str(text);
                }
                true
            }
            Event::Text(text) => {
                let shortcut = ctx.input(|i| i.modifiers.shift || i.modifiers.command);
                if !(text == " " && shortcut) {
                    self.start_edit(text.clone(), EditMode::Enter, true);
                }
                true
            }
            Event::Copy => {
                self.copy(ctx, false);
                true
            }
            Event::Cut => {
                self.copy(ctx, true);
                true
            }
            Event::Paste(text) => {
                self.paste(text);
                true
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => self.handle_grid_key(ctx, *key, *modifiers),
            _ => false,
        }
    }

    fn handle_edit_event(&mut self, ctx: &Context, event: &Event, in_bar: bool) -> bool {
        let Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            return false;
        };
        if self.assist_key(ctx, *key, *modifiers) {
            return true;
        }
        let enter_mode = !in_bar
            && self
                .edit
                .as_ref()
                .is_some_and(|e| e.mode == EditMode::Enter);
        let arrow = match key {
            Key::ArrowUp => Some((-1, 0)),
            Key::ArrowDown => Some((1, 0)),
            Key::ArrowLeft => Some((0, -1)),
            Key::ArrowRight => Some((0, 1)),
            _ => None,
        };
        if let (Some((rows, cols)), true, Some(edit)) = (arrow, enter_mode, self.edit.as_mut())
            && edit.can_point()
        {
            let (anchor, corner) = match edit.pointing() {
                Some(point) if modifiers.shift => (point.anchor, point.corner.offset(rows, cols)),
                Some(point) => {
                    let next = point.corner.offset(rows, cols);
                    (next, next)
                }
                None => {
                    let next = edit.addr.offset(rows, cols);
                    (next, next)
                }
            };
            edit.point_at(anchor, corner);
            self.view.reveal(corner);
            return true;
        }
        if *key == Key::Enter && modifiers.alt {
            self.insert_in_editor(ctx, in_bar, "\n");
            return true;
        }
        if *key == Key::F3 && modifiers.shift {
            self.open_insert_function();
            return true;
        }
        if *key == Key::F4 {
            self.toggle_editor_reference(ctx, in_bar);
            return true;
        }
        let step = match key {
            Key::Enter if modifiers.command => Some((0, 0)),
            Key::Enter if modifiers.shift => Some((-1, 0)),
            Key::Enter => Some((1, 0)),
            Key::Tab if modifiers.shift => Some((0, -1)),
            Key::Tab => Some((0, 1)),
            Key::ArrowUp if enter_mode => Some((-1, 0)),
            Key::ArrowDown if enter_mode => Some((1, 0)),
            Key::ArrowLeft if enter_mode => Some((0, -1)),
            Key::ArrowRight if enter_mode => Some((0, 1)),
            Key::Escape => {
                self.cancel_edit(ctx);
                return true;
            }
            Key::F2 => {
                if let Some(edit) = &mut self.edit {
                    edit.mode = match edit.mode {
                        EditMode::Enter => EditMode::Edit,
                        EditMode::Edit => EditMode::Enter,
                    };
                }
                return true;
            }
            _ => None,
        };
        let Some((rows, cols)) = step else {
            return false;
        };
        if self.commit_edit(ctx) {
            self.enter_step(rows, cols, *key == Key::Tab);
        }
        true
    }

    fn assist_key(&mut self, ctx: &Context, key: Key, modifiers: Modifiers) -> bool {
        let Some(edit) = self.edit.as_mut() else {
            return false;
        };
        let Some(assist) = edit.assist.as_mut() else {
            return false;
        };
        match key {
            Key::ArrowDown => {
                assist.selected = (assist.selected + 1).min(assist.choices.len() - 1);
                assist.reveal = true;
            }
            Key::ArrowUp => {
                assist.selected = assist.selected.saturating_sub(1);
                assist.reveal = true;
            }
            Key::Tab if !modifiers.shift => {
                let index = assist.selected;
                self.accept_completion(ctx, index);
            }
            Key::Escape => {
                edit.assist = None;
                edit.assist_closed = Some(edit.text.clone());
            }
            _ => return false,
        }
        true
    }

    fn handle_grid_key(&mut self, ctx: &Context, key: Key, m: Modifiers) -> bool {
        let ctrl = m.command;
        match key {
            Key::ArrowUp => self.arrow(Direction::Up, m),
            Key::ArrowDown if m.alt => self.toggle_list(),
            Key::ArrowDown => self.arrow(Direction::Down, m),
            Key::ArrowRight if m.alt && m.shift => self.group_selection(true, None),
            Key::ArrowLeft if m.alt && m.shift => self.group_selection(false, None),
            Key::ArrowLeft => self.arrow(Direction::Left, m),
            Key::ArrowRight => self.arrow(Direction::Right, m),
            Key::Enter if m.shift => self.move_active(-1, 0),
            Key::Enter => self.enter_step(1, 0, false),
            Key::Tab if m.shift => self.enter_step(0, -1, true),
            Key::Tab => self.enter_step(0, 1, true),
            Key::PageDown if ctrl => self.activate_step(ctx, 1),
            Key::PageUp if ctrl => self.activate_step(ctx, -1),
            Key::PageDown => self.page(1, m.shift),
            Key::PageUp => self.page(-1, m.shift),
            Key::Home => {
                let target = if ctrl {
                    origin()
                } else {
                    CellAddr::new(self.sel.corner.row(), 0).expect("row is valid")
                };
                self.go(target, m.shift);
            }
            Key::End if ctrl => {
                let (rows, cols) = self.sheet().extent();
                let last = CellAddr::new(rows.saturating_sub(1), cols.saturating_sub(1));
                self.go(last.expect("extent is within the sheet"), m.shift);
            }
            Key::F2 if m.shift => self.edit_note(),
            Key::M if ctrl && m.alt => self.open_comment(self.sel.active),
            Key::F2 if let Some((index, _)) = self.selected_shape() => {
                self.begin_shape_text(index);
            }
            Key::F2 => {
                let text = self.active_input();
                self.start_edit(text, EditMode::Edit, true);
            }
            Key::F12 if ctrl => self.request(ctx, Pending::Open),
            Key::F12 => {
                self.save(true);
            }
            Key::F7 => self.open_spelling(),
            Key::F9 if m.shift => self.calculate_sheet(),
            Key::F9 => self.calculate(),
            Key::F1 if ctrl => self.ribbon_collapsed = !self.ribbon_collapsed,
            Key::Escape if self.list_popup.is_some() => self.list_popup = None,
            Key::F3 if ctrl && m.shift => self.open_create_names(),
            Key::F3 if ctrl => self.open_name_manager(),
            Key::F3 if m.shift => self.open_insert_function(),
            Key::F11 if m.shift => self.insert_sheet(ctx),
            Key::Delete if self.object.is_some() => self.delete_object(),
            Key::Escape if self.shape_tool.is_some() => self.shape_tool = None,
            Key::Escape if self.object.is_some() => self.object = None,
            Key::Delete => self.clear_selection(),
            Key::Backspace => self.start_edit(String::new(), EditMode::Enter, true),
            Key::Escape => self.clipboard = None,
            Key::Space if ctrl && !m.shift => self.select_columns(),
            Key::Space if m.shift && !ctrl => self.select_rows(),
            Key::L if ctrl && m.shift => self.toggle_filter(),
            Key::L | Key::T if ctrl && !m.shift => self.open_create_table(None),
            Key::E if ctrl => self.flash_fill(),
            Key::K if ctrl => self.open_link_dialog(),
            Key::F if ctrl => self.open_find(false),
            Key::H if ctrl => self.open_find(true),
            Key::A if ctrl => self.select_all(),
            Key::B | Key::Num2 if ctrl => self.toggle_bold(),
            Key::I | Key::Num3 if ctrl => self.toggle_italic(),
            Key::U | Key::Num4 if ctrl => self.toggle_underline(),
            Key::Num5 if ctrl => self.toggle_strike(),
            Key::Num1 if ctrl => self.open_format_cells(),
            Key::Backtick if ctrl => self.show_formulas = !self.show_formulas,
            Key::Equals if m.alt => self.auto_sum(ctx, "SUM"),
            Key::V if ctrl && m.alt => self.open_paste_special(),
            Key::D if ctrl => self.fill_down(),
            Key::P if ctrl => self.backstage = Some(crate::backstage::Page::Print),
            Key::F5 => self.open_go_to(),
            Key::G if ctrl => self.open_go_to(),
            Key::R if ctrl => self.fill_right(),
            Key::Plus if ctrl => self.insert_cells(),
            Key::Equals if ctrl && m.shift => self.insert_cells(),
            Key::Minus if ctrl => self.delete_cells(),
            Key::Z if ctrl => self.undo(),
            Key::Y if ctrl => self.redo(),
            Key::S if ctrl => {
                self.save(false);
            }
            Key::O if ctrl => self.backstage = Some(crate::backstage::Page::Open),
            Key::N if ctrl => self.request(ctx, Pending::New),
            _ => return false,
        }
        true
    }

    fn arrow(&mut self, direction: Direction, m: Modifiers) {
        let from = if m.shift {
            self.sel.corner
        } else {
            self.sel.active
        };
        let target = if m.command {
            self.sheet().jump(from, direction)
        } else {
            let (rows, cols) = match direction {
                Direction::Up => (-1, 0),
                Direction::Down => (1, 0),
                Direction::Left => (0, -1),
                Direction::Right => (0, 1),
            };
            self.step_visible(from, rows, cols)
        };
        self.go(target, m.shift);
    }

    fn page(&mut self, pages: i64, extend: bool) {
        let rows = i64::from(self.view.rows.max(1)) * pages;
        let from = if extend {
            self.sel.corner
        } else {
            self.sel.active
        };
        self.view.scroll(rows, 0);
        self.go(from.offset(rows, 0), extend);
    }

    fn go(&mut self, target: CellAddr, extend: bool) {
        if extend {
            self.select(Selection {
                corner: target,
                ..self.sel
            });
        } else {
            self.select(Selection::at(target));
        }
    }

    fn select_columns(&mut self) {
        let range = self.sel.range();
        let top = CellAddr::new(0, range.start().col()).expect("column is valid");
        let bottom = CellAddr::new(MAX_ROWS - 1, range.end().col()).expect("column is valid");
        self.sel = Selection {
            active: self.sel.active,
            anchor: top,
            corner: bottom,
        };
    }

    fn select_rows(&mut self) {
        let range = self.sel.range();
        let left = CellAddr::new(range.start().row(), 0).expect("row is valid");
        let right = CellAddr::new(range.end().row(), MAX_COLS - 1).expect("row is valid");
        self.sel = Selection {
            active: self.sel.active,
            anchor: left,
            corner: right,
        };
    }
}
