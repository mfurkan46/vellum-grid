use eframe::egui::{
    self, Align, CornerRadius, FontFamily, Layout, Popup, Pos2, Rect, RichText, Sense, Ui, vec2,
};
use vellum_grid_core::check::{Issue, all_issues, cell_issue, issues};
use vellum_grid_core::{CellAddr, CellError, CellRange, CellValue};

use crate::app::{Selection, Spreadsheet, locale};
use crate::dialogs::{Dialog, plain_button};
use crate::icons::{self, Icon};
use crate::{fluent, fonts, menu, text, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckAction {
    Fix,
    Ignore,
    Previous,
    Next,
}

pub fn title(issue: Issue) -> &'static str {
    match issue {
        Issue::ErrorValue(CellError::Div0) => text::issue_div0(),
        Issue::ErrorValue(CellError::Name) => text::issue_name(),
        Issue::ErrorValue(CellError::NA) => text::issue_na(),
        Issue::ErrorValue(CellError::Ref) => text::issue_ref(),
        Issue::ErrorValue(CellError::Value) => text::issue_value(),
        Issue::ErrorValue(CellError::Num) => text::issue_num(),
        Issue::ErrorValue(CellError::Null) => text::issue_null(),
        Issue::NumberAsText => text::issue_text_number(),
        Issue::Inconsistent { .. } => text::issue_inconsistent(),
    }
}

pub fn detail(issue: Issue) -> &'static str {
    match issue {
        Issue::ErrorValue(_) => text::issue_error_detail(),
        Issue::NumberAsText => text::issue_text_number_detail(),
        Issue::Inconsistent { .. } => text::issue_inconsistent_detail(),
    }
}

pub fn fix_label(addr: CellAddr, issue: Issue) -> &'static str {
    match issue {
        Issue::ErrorValue(_) => text::trace_error(),
        Issue::NumberAsText => text::convert_to_number(),
        Issue::Inconsistent { from } if from.col() == addr.col() => text::copy_formula_above(),
        Issue::Inconsistent { .. } => text::copy_formula_left(),
    }
}

pub fn dialog_ui(ui: &mut Ui, current: Option<(CellAddr, Issue)>) -> Option<CheckAction> {
    let mut action = None;
    ui.set_width(theme::GO_TO_WIDTH);
    let Some((addr, issue)) = current else {
        ui.label(text::check_done());
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if ui.add(plain_button(text::ok())).clicked() {
                action = Some(CheckAction::Next);
            }
        });
        return action;
    };
    ui.label(format!("{} {addr}", text::error_in_cell()));
    ui.label(RichText::new(title(issue)).family(FontFamily::Name(fonts::CHROME_BOLD.into())));
    ui.label(detail(issue));
    ui.horizontal(|ui| {
        if ui.add(plain_button(fix_label(addr, issue))).clicked() {
            action = Some(CheckAction::Fix);
        }
        if ui.add(plain_button(text::ignore_error())).clicked() {
            action = Some(CheckAction::Ignore);
        }
    });
    ui.horizontal(|ui| {
        if ui.add(plain_button(text::previous())).clicked() {
            action = Some(CheckAction::Previous);
        }
        if ui.add(plain_button(text::next())).clicked() {
            action = Some(CheckAction::Next);
        }
    });
    action
}

impl Spreadsheet {
    pub fn active_issue(&self) -> Option<Issue> {
        cell_issue(self.sheet(), self.sel.active, locale())
    }

    pub fn refresh_issues(&mut self, panes: Vec<CellRange>) {
        let key = (self.doc.revision(), self.doc.active(), panes);
        if self.issue_key.as_ref() == Some(&key) {
            return;
        }
        self.issue_cells = key
            .2
            .iter()
            .flat_map(|range| issues(self.sheet(), *range, locale()))
            .filter(|(_, issue)| issue.marked())
            .map(|(addr, _)| addr)
            .collect();
        self.issue_key = Some(key);
    }

    pub fn fix_issue(&mut self, addr: CellAddr, issue: Issue) {
        match issue {
            Issue::NumberAsText => {
                let CellValue::Text(text) = self.sheet().value(addr).clone() else {
                    return;
                };
                let cell = self.doc.styled_input(addr, text.trim());
                self.doc.edit([(addr, cell)]);
            }
            Issue::Inconsistent { from } => {
                let Some(text) = self.sheet().get(from).and_then(|cell| cell.formula.clone())
                else {
                    return;
                };
                let Ok(expr) = vellum_grid_core::formula::parse(&text[1..], &vellum_grid_core::EN)
                else {
                    return;
                };
                let rows = i64::from(addr.row()) - i64::from(from.row());
                let cols = i64::from(addr.col()) - i64::from(from.col());
                let input = format!("={}", expr.shifted(rows, cols).write(locale()));
                let cell = self.doc.styled_input(addr, &input);
                self.doc.edit([(addr, cell)]);
            }
            Issue::ErrorValue(_) => {
                self.select(Selection::at(addr));
                self.arrows.clear();
                self.trace(true);
            }
        }
    }

    pub fn ignore_issue(&mut self, addr: CellAddr) {
        self.doc.set_layout(|layout| {
            layout.ignored_errors.insert(addr);
        });
    }

    pub fn open_error_check(&mut self) {
        let found = all_issues(self.sheet(), locale());
        let active = self.sel.active;
        let first = found
            .iter()
            .find(|(addr, _)| (addr.row(), addr.col()) >= (active.row(), active.col()))
            .or(found.first())
            .map(|(addr, _)| *addr);
        if let Some(addr) = first {
            self.select(Selection::at(addr));
        }
        self.dialog = Some(Dialog::ErrorCheck { at: first });
    }

    pub fn current_issue(&self, at: Option<CellAddr>) -> Option<(CellAddr, Issue)> {
        let addr = at?;
        cell_issue(self.sheet(), addr, locale()).map(|issue| (addr, issue))
    }

    pub fn error_check_action(&mut self, at: Option<CellAddr>, action: CheckAction) {
        let Some(current) = at else {
            return;
        };
        if let Some((addr, issue)) = self.current_issue(at) {
            match action {
                CheckAction::Fix => self.fix_issue(addr, issue),
                CheckAction::Ignore => self.ignore_issue(addr),
                _ => {}
            }
        }
        let found = all_issues(self.sheet(), locale());
        let order = |addr: CellAddr| (addr.row(), addr.col());
        let next = match action {
            CheckAction::Previous => found
                .iter()
                .rev()
                .find(|(addr, _)| order(*addr) < order(current))
                .map(|(addr, _)| *addr)
                .or(Some(current)),
            CheckAction::Next | CheckAction::Ignore | CheckAction::Fix => found
                .iter()
                .find(|(addr, _)| order(*addr) > order(current))
                .map(|(addr, _)| *addr),
        };
        let next = match (action, next) {
            (CheckAction::Fix | CheckAction::Ignore, None) => found.first().map(|(addr, _)| *addr),
            (_, next) => next,
        };
        if let Some(addr) = next {
            self.select(Selection::at(addr));
        }
        self.dialog = Some(Dialog::ErrorCheck { at: next });
    }

    pub fn issue_tag(&mut self, ui: &mut Ui, corner: Pos2) {
        let Some(issue) = self.active_issue() else {
            return;
        };
        let addr = self.sel.active;
        let size = vec2(theme::ISSUE_TAG, theme::ISSUE_TAG);
        let rect = Rect::from_min_size(corner, size);
        self.issue_tag_rect = Some(rect);
        let response = ui.interact(rect, ui.scope_id().with("issue-tag"), Sense::click());
        let fill = if response.hovered() {
            theme::tool_hover()
        } else {
            theme::surface()
        };
        ui.painter().rect_filled(rect, CornerRadius::same(2), fill);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(2),
            egui::Stroke::new(1.0, theme::chrome_line()),
            egui::StrokeKind::Inside,
        );
        icons::paint(
            ui.painter(),
            rect.shrink(2.0),
            Icon::Plain(fluent::WARNING),
            theme::ISSUE_ICON,
            None,
        );
        let response = response.on_hover_text(title(issue));
        Popup::menu(&response).show(|ui| {
            menu::soon(ui, None, title(issue));
            menu::separator(ui);
            if menu::item(ui, fix_label(addr, issue)) {
                self.fix_issue(addr, issue);
            }
            if menu::item(ui, text::ignore_error()) {
                self.ignore_issue(addr);
            }
            if menu::item(ui, text::error_check_options()) {
                self.open_error_check();
            }
        });
    }
}
