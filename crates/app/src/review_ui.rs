use eframe::egui::{
    self, Align, Area, Context, FontFamily, Frame, Id, Key, Layout, Margin, Order, Pos2, Rect,
    RichText, ScrollArea, Stroke, TextEdit, Ui, vec2,
};
use vellum_grid_core::review::{Comment, Note, Reply};
use vellum_grid_core::{CellAddr, CellValue, numfmt};

use crate::app::{Selection, Spreadsheet, locale};
use crate::dialogs::{accent_button, plain_button};
use crate::{clock, fonts, text, theme};

#[derive(Debug, Clone)]
pub struct NoteEdit {
    pub addr: CellAddr,
    pub text: String,
    pub fresh: bool,
}

#[derive(Debug, Clone)]
pub struct CommentEdit {
    pub addr: CellAddr,
    pub text: String,
    pub fresh: bool,
}

pub fn user_name() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| text::default_author().to_owned())
    }
    #[cfg(target_arch = "wasm32")]
    {
        text::default_author().to_owned()
    }
}

fn bold(text: &str) -> RichText {
    RichText::new(text)
        .family(FontFamily::Name(fonts::CHROME_BOLD.into()))
        .color(theme::chrome_text())
}

fn time_text(time: f64) -> String {
    locale().format_value(&CellValue::Number(time), numfmt::DATE_TIME)
}

fn note_frame() -> Frame {
    Frame::NONE
        .fill(theme::note_bg())
        .stroke(Stroke::new(1.0, theme::note_border()))
        .inner_margin(Margin::same(theme::MENU_MARGIN))
}

fn card_frame(ui: &Ui) -> Frame {
    Frame::popup(ui.style()).inner_margin(Margin::same(theme::DIALOG_MARGIN / 2))
}

fn note_body(ui: &mut Ui, note: &Note) {
    ui.set_width(theme::NOTE_WIDTH);
    if !note.author.is_empty() {
        ui.label(bold(&format!("{}:", note.author)));
    }
    ui.label(RichText::new(&note.text).color(theme::chrome_text()));
}

fn thread_body(ui: &mut Ui, comment: &Comment) {
    for (index, reply) in comment.thread.iter().enumerate() {
        if index > 0 {
            ui.separator();
        }
        ui.horizontal(|ui| {
            ui.label(bold(&reply.author));
            ui.label(
                RichText::new(time_text(reply.time))
                    .font(theme::small_font())
                    .color(theme::chrome_text_muted()),
            );
        });
        ui.label(RichText::new(&reply.text).color(theme::chrome_text()));
    }
    if comment.resolved {
        ui.label(RichText::new(text::resolved()).color(theme::chrome_text_muted()));
    }
}

fn floating(
    ctx: &Context,
    id: impl std::hash::Hash + std::fmt::Debug,
    at: Pos2,
    add: impl FnOnce(&mut Ui),
) -> Rect {
    Area::new(Id::unique(id))
        .order(Order::Middle)
        .fixed_pos(at)
        .show(ctx, add)
        .response
        .rect
}

pub enum ReviewAction {
    SaveNote,
    SendComment,
    CancelComment,
    Resolve(CellAddr, bool),
    DeleteComment(CellAddr),
    Reply(CellAddr),
    Select(CellAddr),
}

impl Spreadsheet {
    pub fn review_overlays(&mut self, ctx: &Context, rect_of: &dyn Fn(CellAddr) -> Option<Rect>) {
        let layout = self.sheet().layout().clone();
        let mut action = None;
        for (addr, note) in &layout.notes {
            let editing = self
                .note_edit
                .as_ref()
                .is_some_and(|edit| edit.addr == *addr);
            let hovered = self.hover == Some(*addr) && self.edit.is_none();
            if editing || !(note.visible || self.show_all_notes || hovered) {
                continue;
            }
            if let Some(cell) = rect_of(*addr) {
                let at = cell.right_top() + vec2(theme::NOTE_GAP, 0.0);
                floating(ctx, ("note", *addr), at, |ui| {
                    note_frame().show(ui, |ui| note_body(ui, note));
                });
            }
        }
        if let Some(addr) = self
            .hover
            .filter(|_| self.edit.is_none() && self.comment_edit.is_none())
            && let (Some(comment), Some(cell)) = (layout.comments.get(&addr), rect_of(addr))
        {
            let at = cell.right_top() + vec2(theme::NOTE_GAP, 0.0);
            floating(ctx, ("comment", addr), at, |ui| {
                card_frame(ui).show(ui, |ui| {
                    ui.set_width(theme::COMMENT_WIDTH);
                    thread_body(ui, comment);
                });
            });
        }
        if let Some(edit) = self.note_edit.as_mut()
            && let Some(cell) = rect_of(edit.addr)
        {
            let author = layout
                .notes
                .get(&edit.addr)
                .map_or_else(user_name, |note| note.author.clone());
            let at = cell.right_top() + vec2(theme::NOTE_GAP, 0.0);
            let area = floating(ctx, "note-edit", at, |ui| {
                note_frame().show(ui, |ui| {
                    ui.set_width(theme::NOTE_WIDTH);
                    ui.label(bold(&format!("{author}:")));
                    let input = ui.add(
                        TextEdit::multiline(&mut edit.text)
                            .frame(Frame::NONE)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    if edit.fresh {
                        input.request_focus();
                    }
                });
            });
            let outside = ctx.input(|i| {
                i.pointer.any_pressed()
                    && i.pointer.interact_pos().is_some_and(|p| !area.contains(p))
            });
            if (outside && !edit.fresh) || ctx.input(|i| i.key_pressed(Key::Escape)) {
                action = Some(ReviewAction::SaveNote);
            }
            edit.fresh = false;
        }
        if let Some(edit) = self.comment_edit.as_mut()
            && let Some(cell) = rect_of(edit.addr)
        {
            let existing = layout.comments.get(&edit.addr).cloned();
            let addr = edit.addr;
            let at = cell.right_top() + vec2(theme::NOTE_GAP, 0.0);
            floating(ctx, "comment-edit", at, |ui| {
                card_frame(ui).show(ui, |ui| {
                    ui.set_width(theme::COMMENT_WIDTH);
                    if let Some(comment) = &existing {
                        thread_body(ui, comment);
                        ui.separator();
                    }
                    let hint = if existing.is_some() {
                        text::reply_hint()
                    } else {
                        text::comment_hint()
                    };
                    let input = ui.add(
                        TextEdit::multiline(&mut edit.text)
                            .hint_text(hint)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .background_color(theme::input_bg()),
                    );
                    if edit.fresh {
                        input.request_focus();
                    }
                    let send = input.has_focus()
                        && ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command);
                    ui.horizontal(|ui| {
                        if let Some(comment) = &existing {
                            let label = if comment.resolved {
                                text::reopen_thread()
                            } else {
                                text::resolve_thread()
                            };
                            if ui.add(plain_button(label)).clicked() {
                                action = Some(ReviewAction::Resolve(addr, !comment.resolved));
                            }
                            if ui.add(plain_button(text::delete_thread())).clicked() {
                                action = Some(ReviewAction::DeleteComment(addr));
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(plain_button(text::cancel())).clicked() {
                                action = Some(ReviewAction::CancelComment);
                            }
                            let ready = !edit.text.trim().is_empty();
                            if ui.add_enabled(ready, accent_button(text::send())).clicked()
                                || (send && ready)
                            {
                                action = Some(ReviewAction::SendComment);
                            }
                        });
                    });
                });
            });
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                action = Some(ReviewAction::CancelComment);
            }
            edit.fresh = false;
        }
        if let Some(action) = action {
            self.review_action(action);
        }
    }

    pub fn comments_pane(&mut self, ui: &mut Ui) {
        let comments = self.sheet().layout().comments.clone();
        let mut action = None;
        ui.horizontal(|ui| {
            ui.label(bold(text::comments()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(plain_button(text::new_comment())).clicked() {
                    action = Some(ReviewAction::Reply(self.sel.active));
                }
            });
        });
        ui.separator();
        ScrollArea::vertical().show(ui, |ui| {
            if comments.is_empty() {
                ui.label(RichText::new(text::no_comments()).color(theme::chrome_text_muted()));
            }
            for (addr, comment) in &comments {
                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if ui.link(addr.to_string()).clicked() {
                        action = Some(ReviewAction::Select(*addr));
                    }
                    thread_body(ui, comment);
                    ui.horizontal(|ui| {
                        if ui.button(text::reply()).clicked() {
                            action = Some(ReviewAction::Reply(*addr));
                        }
                        let label = if comment.resolved {
                            text::reopen_thread()
                        } else {
                            text::resolve_thread()
                        };
                        if ui.button(label).clicked() {
                            action = Some(ReviewAction::Resolve(*addr, !comment.resolved));
                        }
                        if ui.button(text::delete_thread()).clicked() {
                            action = Some(ReviewAction::DeleteComment(*addr));
                        }
                    });
                });
                ui.add_space(theme::DIALOG_SPACING / 2.0);
            }
        });
        if let Some(action) = action {
            self.review_action(action);
        }
    }

    fn review_action(&mut self, action: ReviewAction) {
        match action {
            ReviewAction::SaveNote => self.save_note(),
            ReviewAction::SendComment => {
                let Some(edit) = self.comment_edit.take() else {
                    return;
                };
                let reply = Reply {
                    author: user_name(),
                    text: edit.text.trim().to_owned(),
                    time: clock::now(),
                };
                self.doc.set_layout(|layout| {
                    layout
                        .comments
                        .entry(edit.addr)
                        .or_default()
                        .thread
                        .push(reply);
                });
            }
            ReviewAction::CancelComment => self.comment_edit = None,
            ReviewAction::Resolve(addr, resolved) => {
                self.doc.set_layout(|layout| {
                    if let Some(comment) = layout.comments.get_mut(&addr) {
                        comment.resolved = resolved;
                    }
                });
            }
            ReviewAction::DeleteComment(addr) => {
                self.comment_edit = None;
                self.doc.set_layout(|layout| {
                    layout.comments.remove(&addr);
                });
            }
            ReviewAction::Reply(addr) => self.open_comment(addr),
            ReviewAction::Select(addr) => {
                self.select(Selection::at(addr));
                self.view.reveal(addr);
            }
        }
    }

    pub fn open_comment(&mut self, addr: CellAddr) {
        if self.edit.is_some() {
            return;
        }
        self.select(Selection::at(addr));
        self.comment_edit = Some(CommentEdit {
            addr,
            text: String::new(),
            fresh: true,
        });
    }

    pub fn edit_note(&mut self) {
        let addr = self.sel.active;
        let text = self
            .sheet()
            .layout()
            .notes
            .get(&addr)
            .map(|note| note.text.clone())
            .unwrap_or_default();
        self.note_edit = Some(NoteEdit {
            addr,
            text,
            fresh: true,
        });
    }

    fn save_note(&mut self) {
        let Some(edit) = self.note_edit.take() else {
            return;
        };
        let existing = self.sheet().layout().notes.get(&edit.addr).cloned();
        if existing.is_none() && edit.text.trim().is_empty() {
            return;
        }
        let note = Note {
            author: existing
                .as_ref()
                .map_or_else(user_name, |note| note.author.clone()),
            text: edit.text,
            visible: existing.is_some_and(|note| note.visible),
        };
        self.doc.set_layout(|layout| {
            layout.notes.insert(edit.addr, note);
        });
    }

    pub fn delete_note(&mut self) {
        let addr = self.sel.active;
        self.doc.set_layout(|layout| {
            layout.notes.remove(&addr);
        });
    }

    pub fn toggle_note(&mut self) {
        let addr = self.sel.active;
        self.doc.set_layout(|layout| {
            if let Some(note) = layout.notes.get_mut(&addr) {
                note.visible = !note.visible;
            }
        });
    }

    pub fn notes_to_comments(&mut self) {
        let now = clock::now();
        self.doc.set_layout(|layout| {
            for (addr, note) in std::mem::take(&mut layout.notes) {
                let comment = layout.comments.entry(addr).or_default();
                comment.thread.insert(
                    0,
                    Reply {
                        author: note.author,
                        text: note.text,
                        time: now,
                    },
                );
            }
        });
    }

    pub fn step_review(&mut self, notes: bool, forward: bool) {
        let layout = self.sheet().layout();
        let keys: Vec<CellAddr> = if notes {
            layout.notes.keys().copied().collect()
        } else {
            layout.comments.keys().copied().collect()
        };
        let order = |addr: &CellAddr| (addr.row(), addr.col());
        let here = order(&self.sel.active);
        let mut sorted = keys;
        sorted.sort_by_key(order);
        let next = if forward {
            sorted
                .iter()
                .find(|addr| order(addr) > here)
                .or(sorted.first())
        } else {
            sorted
                .iter()
                .rev()
                .find(|addr| order(addr) < here)
                .or(sorted.last())
        };
        if let Some(addr) = next.copied() {
            self.select(Selection::at(addr));
            self.view.reveal(addr);
            if notes {
                self.hover = Some(addr);
            } else {
                self.open_comment(addr);
            }
        }
    }
}

pub fn indicator(painter: &egui::Painter, cell: Rect, color: egui::Color32) {
    let corner = cell.right_top() + vec2(-theme::SELECTION_BORDER, theme::SELECTION_BORDER);
    painter.add(egui::Shape::convex_polygon(
        vec![
            corner,
            corner + vec2(-theme::ISSUE_MARK_SIZE, 0.0),
            corner + vec2(0.0, theme::ISSUE_MARK_SIZE),
        ],
        color,
        Stroke::NONE,
    ));
}
