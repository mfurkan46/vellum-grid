use std::collections::HashSet;

use eframe::egui::{
    self, Align2, CornerRadius, CursorIcon, Event, Frame, Margin, MouseWheelUnit, Painter, Pos2,
    Rect, Response, Sense, Shape, Stroke, StrokeKind, TextEdit, Ui, Vec2, emath::GuiRounding, pos2,
    vec2,
};
use vellum_grid_core::{
    CellAddr, CellRange, HAlign, Highlighter, Layout, MAX_COLS, MAX_ROWS, Style, column_name,
    outline,
};

use crate::app::{
    Drag, EditMode, Resize, Selection, Spreadsheet, View, cursor_to_end, locale, select_chars,
};
use crate::context::ContextTarget;
use crate::geometry::Axis;
use crate::objects::{self, Object, ObjectDrag};
use crate::render::{self, Visual};
use crate::{charts, fonts, format, icons, paste, references, text, theme};
use vellum_grid_core::drawing::Shape as DrawnShape;

pub struct Screen {
    full: Rect,
    cells: Rect,
    col_header: Rect,
    row_header: Rect,
    corner: Rect,
    vbar: Rect,
    hbar: Rect,
    cols: Axis,
    rows: Axis,
    left: u32,
    top: u32,
    last_col: u32,
    last_row: u32,
    full_cols: u32,
    full_rows: u32,
    frozen_rows: u32,
    frozen_cols: u32,
    frozen_w: f32,
    frozen_h: f32,
    zoom: f32,
}

#[derive(Clone, Copy)]
struct Pane {
    clip: Rect,
    visible: CellRange,
}

#[derive(Clone, Copy)]
struct Segment {
    first: u32,
    last: u32,
    from: f32,
    to: f32,
}

impl Screen {
    fn new(full: Rect, view: &View, cols: Axis, rows: Axis, layout: &Layout, zoom: f32) -> Self {
        let headings = !layout.hide_headings;
        let (frozen_rows, frozen_cols) = (layout.freeze_rows, layout.freeze_cols);
        let frozen_h = rows.offset(0, frozen_rows);
        let frozen_w = cols.offset(0, frozen_cols);
        let right = full.right() - theme::SCROLLBAR_SIZE;
        let bottom = full.bottom() - theme::SCROLLBAR_SIZE;
        let (_, rows_shown) = rows.fit(view.top, bottom - full.top() - frozen_h);
        let header_w = if headings {
            row_header_width((view.top + rows_shown).min(MAX_ROWS)) * zoom
        } else {
            0.0
        };
        let header_h = if headings {
            theme::COL_HEADER_HEIGHT * zoom
        } else {
            0.0
        };
        let cells_top = full.top() + header_h;
        let cells_left = full.left() + header_w;
        let cells = Rect::from_min_max(pos2(cells_left, cells_top), pos2(right, bottom));
        let (full_cols, shown_cols) = cols.fit(view.left, (cells.width() - frozen_w).max(0.0));
        let (full_rows, shown_rows) = rows.fit(view.top, (cells.height() - frozen_h).max(0.0));
        Self {
            full,
            cells,
            col_header: Rect::from_min_max(pos2(cells_left, full.top()), pos2(right, cells_top)),
            row_header: Rect::from_min_max(pos2(full.left(), cells_top), pos2(cells_left, bottom)),
            corner: Rect::from_min_max(full.min, pos2(cells_left, cells_top)),
            vbar: Rect::from_min_max(pos2(right, cells_top), pos2(full.right(), bottom)),
            hbar: Rect::from_min_max(pos2(cells_left, bottom), pos2(right, full.bottom())),
            cols,
            rows,
            left: view.left,
            top: view.top,
            last_col: (view.left + shown_cols - 1).min(MAX_COLS - 1),
            last_row: (view.top + shown_rows - 1).min(MAX_ROWS - 1),
            full_cols,
            full_rows,
            frozen_rows,
            frozen_cols,
            frozen_w,
            frozen_h,
            zoom,
        }
    }

    fn x(&self, col: u32) -> f32 {
        if col < self.frozen_cols {
            self.cells.left() + self.cols.offset(0, col)
        } else {
            self.cells.left() + self.frozen_w + self.cols.offset(self.left, col)
        }
    }

    fn y(&self, row: u32) -> f32 {
        if row < self.frozen_rows {
            self.cells.top() + self.rows.offset(0, row)
        } else {
            self.cells.top() + self.frozen_h + self.rows.offset(self.top, row)
        }
    }

    fn cell_rect(&self, addr: CellAddr) -> Rect {
        Rect::from_min_size(
            pos2(self.x(addr.col()), self.y(addr.row())),
            vec2(self.cols.size(addr.col()), self.rows.size(addr.row())),
        )
    }

    fn range_rect(&self, range: CellRange) -> Rect {
        let (start, end) = (range.start(), range.end());
        let right = self.x(end.col()) + self.cols.size(end.col());
        let bottom = self.y(end.row()) + self.rows.size(end.row());
        Rect::from_min_max(
            pos2(self.x(start.col()), self.y(start.row())),
            pos2(right, bottom),
        )
    }

    fn hit(&self, pos: Pos2) -> CellAddr {
        let dx = pos.x - self.cells.left();
        let dy = pos.y - self.cells.top();
        let col = if self.frozen_cols > 0 && dx < self.frozen_w {
            self.cols.index_at(0, dx).min(self.frozen_cols - 1)
        } else {
            self.cols.index_at(self.left, dx - self.frozen_w)
        };
        let row = if self.frozen_rows > 0 && dy < self.frozen_h {
            self.rows.index_at(0, dy).min(self.frozen_rows - 1)
        } else {
            self.rows.index_at(self.top, dy - self.frozen_h)
        };
        CellAddr::new(row, col).expect("axis indices stay inside the sheet")
    }

    fn row_segments(&self) -> Vec<Segment> {
        let top = self.cells.top();
        let mut segments = Vec::new();
        if self.frozen_rows > 0 {
            segments.push(Segment {
                first: 0,
                last: self.frozen_rows - 1,
                from: top,
                to: top + self.frozen_h,
            });
        }
        segments.push(Segment {
            first: self.top,
            last: self.last_row,
            from: top + self.frozen_h,
            to: self.cells.bottom(),
        });
        segments
    }

    fn col_segments(&self) -> Vec<Segment> {
        let left = self.cells.left();
        let mut segments = Vec::new();
        if self.frozen_cols > 0 {
            segments.push(Segment {
                first: 0,
                last: self.frozen_cols - 1,
                from: left,
                to: left + self.frozen_w,
            });
        }
        segments.push(Segment {
            first: self.left,
            last: self.last_col,
            from: left + self.frozen_w,
            to: self.cells.right(),
        });
        segments
    }

    fn panes(&self) -> Vec<Pane> {
        let mut panes = Vec::new();
        for rows in self.row_segments() {
            for cols in self.col_segments() {
                panes.push(Pane {
                    clip: Rect::from_min_max(pos2(cols.from, rows.from), pos2(cols.to, rows.to)),
                    visible: CellRange::new(
                        addr(rows.first, cols.first),
                        addr(rows.last, cols.last),
                    ),
                });
            }
        }
        panes
    }

    fn col_border(&self, x: f32) -> Option<u32> {
        self.col_segments()
            .into_iter()
            .flat_map(|s| s.first..=s.last)
            .filter(|&col| self.cols.size(col) > 0.0 || col + 1 == self.frozen_cols)
            .find(|&col| (self.x(col) + self.cols.size(col) - x).abs() <= theme::RESIZE_GRIP)
    }

    fn row_border(&self, y: f32) -> Option<u32> {
        self.row_segments()
            .into_iter()
            .flat_map(|s| s.first..=s.last)
            .filter(|&row| self.rows.size(row) > 0.0)
            .find(|&row| (self.y(row) + self.rows.size(row) - y).abs() <= theme::RESIZE_GRIP)
    }

    fn fill_handle(&self, range: CellRange) -> Rect {
        let size = theme::FILL_HANDLE + 2.0 * theme::RESIZE_GRIP;
        Rect::from_center_size(self.range_rect(range).max, vec2(size, size))
    }

    fn object_clip(&self, anchor: CellAddr) -> Rect {
        let mut clip = self.cells;
        if anchor.row() >= self.frozen_rows {
            clip.min.y += self.frozen_h;
        }
        if anchor.col() >= self.frozen_cols {
            clip.min.x += self.frozen_w;
        }
        clip
    }

    fn object_rect(&self, frame: &objects::Frame) -> Rect {
        let min = pos2(
            self.x(frame.anchor.col()) + frame.offset.0 as f32 * self.zoom,
            self.y(frame.anchor.row()) + frame.offset.1 as f32 * self.zoom,
        );
        let size = vec2(frame.size.0 as f32, frame.size.1 as f32) * self.zoom;
        Rect::from_min_size(min, size)
    }

    fn filter_button(&self, addr: CellAddr) -> Rect {
        let cell = self.cell_rect(addr);
        let size = theme::FILTER_BUTTON.min(cell.height() - 2.0).max(0.0);
        Rect::from_min_size(
            pos2(cell.right() - size - 1.0, cell.bottom() - size - 1.0),
            vec2(size, size),
        )
    }
}

const SHAPE_EDITOR: &str = "shape-editor";

#[derive(Clone, Copy)]
struct Hit {
    object: Object,
    rect: Rect,
    flip: (bool, bool),
    line: bool,
    aspect: bool,
    clip: Rect,
}

fn handle_cursor(hit: Hit, handle: (u8, u8)) -> CursorIcon {
    if hit.line {
        return CursorIcon::Crosshair;
    }
    let (a, b) = objects::ends(hit.rect, hit.flip);
    let side = |code: u8, a: f32, b: f32| match code {
        1 => (a - b).signum(),
        2 => (b - a).signum(),
        _ => 0.0,
    };
    match (side(handle.0, a.x, b.x), side(handle.1, a.y, b.y)) {
        (x, y) if x != 0.0 && y != 0.0 && x == y => CursorIcon::ResizeNwSe,
        (x, y) if x != 0.0 && y != 0.0 => CursorIcon::ResizeNeSw,
        (x, _) if x != 0.0 => CursorIcon::ResizeHorizontal,
        _ => CursorIcon::ResizeVertical,
    }
}

pub fn digit_width(ui: &Ui) -> f32 {
    ui.painter()
        .layout_no_wrap("0".into(), theme::cell_font(), theme::cell_text())
        .size()
        .x
        .round()
        .max(1.0)
}

impl Spreadsheet {
    fn axes(&self, digit: f32) -> (Axis, Axis) {
        let layout = self.sheet().layout();
        let mut cols = Axis::columns(layout, digit * self.zoom);
        let mut rows = Axis::rows(layout, self.zoom);
        if let Some(resize) = &self.resize {
            let axis = if resize.columns { &mut cols } else { &mut rows };
            for index in &resize.indices {
                axis.set(*index, resize.size);
            }
        }
        (cols, rows)
    }

    pub fn grid_ui(&mut self, ui: &mut Ui) {
        let digit = digit_width(ui);
        self.digit = digit;
        if !self.autofit_queue.is_empty() {
            let rows = std::mem::take(&mut self.autofit_queue);
            self.autofit_rows(ui, rows);
        }
        let (cols, rows) = self.axes(digit);
        let layout = self.sheet().layout();
        let (rows_frozen, cols_frozen) = (layout.freeze_rows, layout.freeze_cols);
        self.view.min_top = rows_frozen;
        self.view.min_left = cols_frozen;
        self.view.scroll(0, 0);
        let layout = self.sheet().layout();
        let outer = ui.max_rect();
        let gutter = outline_gutter(layout);
        let inner = Rect::from_min_max(outer.min + gutter, outer.max);
        let screen = Screen::new(inner, &self.view, cols, rows, layout, self.zoom);
        self.view.rows = screen.full_rows;
        self.view.cols = screen.full_cols;
        self.grid_area = screen.cells.size();

        let area = screen
            .cells
            .union(screen.col_header)
            .union(screen.row_header)
            .union(screen.corner);
        let response = ui.interact(area, ui.scope_id().with("grid"), Sense::click_and_drag());
        if ui.rect_contains_pointer(screen.full) {
            self.handle_wheel(ui, screen.cols.size(screen.left));
        }
        self.soft_cursor = None;
        self.handle_mouse(ui, &screen, &response, digit);
        let linked = self
            .hover
            .filter(|addr| self.sheet().layout().links.contains_key(addr) && self.edit.is_none());
        if let Some(addr) = linked {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            if response.clicked() && !ui.input(|i| i.modifiers.command) {
                self.follow_link(addr);
            }
        }
        if let Some(target) = self.context_target {
            response.context_menu(|ui| self.context_menu(ui, target));
        }
        self.sync_arrows();
        let panes = screen
            .panes()
            .into_iter()
            .filter(|pane| pane.clip.is_positive())
            .map(|pane| pane.visible)
            .collect();
        self.refresh_issues(panes);
        self.refresh_circles();
        self.prepare_pictures(ui.ctx());
        self.paint(ui, &screen);
        self.outline_ui(ui, &screen, outer, gutter);
        self.scrollbar(ui, &screen, true);
        self.scrollbar(ui, &screen, false);
        self.hover = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|pos| screen.cells.contains(*pos))
            .map(|pos| screen.hit(pos));
        let cells = screen.cells;
        let rect_of = |addr: CellAddr| {
            let rect = screen.range_rect(CellRange::single(addr));
            cells.contains(rect.center()).then_some(rect)
        };
        self.review_overlays(ui.ctx(), &rect_of);
        if let Some(addr) = self.hover.filter(|_| self.edit.is_none())
            && let Some(cell) = rect_of(addr)
        {
            self.link_tip(ui.ctx(), addr, cell);
        }
        self.issue_tag_rect = None;
        self.list_button_rect = None;
        let active = screen.range_rect(CellRange::single(self.sel.active));
        if screen.cells.contains(active.center()) {
            self.input_message(ui.ctx(), active);
            if self.edit.is_none() {
                self.list_button(ui, active);
            }
        }
        if self.edit.is_none() {
            let cell = screen.range_rect(CellRange::single(self.sel.active));
            let left = cell.left_top() - vec2(theme::ISSUE_TAG + theme::ISSUE_GAP, 0.0);
            let corner = if screen.cells.contains(left) {
                left
            } else {
                cell.right_top() + vec2(theme::ISSUE_GAP, 0.0)
            };
            if screen.cells.contains(corner) {
                self.issue_tag(ui, corner);
            }
        }
        self.shape_editor(ui, &screen);
        self.editor(ui, &screen);
        self.paint_soft_cursor(ui);
    }

    fn prepare_context(&mut self, screen: &Screen, pos: Pos2) -> ContextTarget {
        if let Some(hit) = self.object_at(screen, pos) {
            self.commit_shape_text();
            self.object = Some(hit.object);
            return ContextTarget::Object(hit.object);
        }
        self.object = None;
        let addr = screen.hit(pos);
        let range = self.selected();
        if screen.col_header.contains(pos) {
            let inside = range.is_full_cols()
                && (range.start().col()..=range.end().col()).contains(&addr.col());
            if !inside {
                self.sel = Selection {
                    active: CellAddr::new(self.view.top, addr.col()).expect("valid"),
                    anchor: CellAddr::new(0, addr.col()).expect("valid"),
                    corner: CellAddr::new(MAX_ROWS - 1, addr.col()).expect("valid"),
                };
            }
            return ContextTarget::Cols;
        }
        if screen.row_header.contains(pos) {
            let inside = range.is_full_rows()
                && (range.start().row()..=range.end().row()).contains(&addr.row());
            if !inside {
                self.sel = Selection {
                    active: CellAddr::new(addr.row(), self.view.left).expect("valid"),
                    anchor: CellAddr::new(addr.row(), 0).expect("valid"),
                    corner: CellAddr::new(addr.row(), MAX_COLS - 1).expect("valid"),
                };
            }
            return ContextTarget::Rows;
        }
        if !range.contains(addr) {
            let merged = self.sheet().layout().merge_at(addr);
            let active = merged.map_or(addr, |m| m.start());
            self.sel = Selection {
                active,
                anchor: active,
                corner: merged.map_or(addr, |m| m.end()),
            };
        }
        ContextTarget::Cells
    }

    fn paint_soft_cursor(&self, ui: &Ui) {
        let Some(cursor) = self.soft_cursor else {
            return;
        };
        let Some(pos) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        ui.ctx().set_cursor_icon(CursorIcon::None);
        let layer = egui::LayerId::new(egui::Order::Tooltip, ui.scope_id().with("soft-cursor"));
        let painter = ui.ctx().layer_painter(layer);
        let pos = pos.round();
        let outline = Stroke::new(1.0, theme::CURSOR_OUTLINE);
        match cursor {
            SoftCursor::Cell => {
                let (arm, half) = (theme::CURSOR_ARM, theme::CURSOR_HALF);
                let horizontal = Rect::from_center_size(pos, vec2(2.0 * arm, 2.0 * half));
                let vertical = Rect::from_center_size(pos, vec2(2.0 * half, 2.0 * arm));
                painter.rect_filled(horizontal, CornerRadius::ZERO, theme::CURSOR_FILL);
                painter.rect_filled(vertical, CornerRadius::ZERO, theme::CURSOR_FILL);
                let points = [
                    (-half, -arm),
                    (half, -arm),
                    (half, -half),
                    (arm, -half),
                    (arm, half),
                    (half, half),
                    (half, arm),
                    (-half, arm),
                    (-half, half),
                    (-arm, half),
                    (-arm, -half),
                    (-half, -half),
                ]
                .into_iter()
                .map(|(x, y)| pos + vec2(x, y))
                .collect();
                painter.add(Shape::closed_line(points, outline));
            }
            SoftCursor::Down | SoftCursor::Right => {
                let turn = |x: f32, y: f32| {
                    if cursor == SoftCursor::Down {
                        pos + vec2(x, y)
                    } else {
                        pos + vec2(y, x)
                    }
                };
                let (shaft, head, length) = (1.5, 5.0, 12.0);
                let shape = vec![
                    turn(-shaft, -length / 2.0),
                    turn(shaft, -length / 2.0),
                    turn(shaft, 0.0),
                    turn(head, 0.0),
                    turn(0.0, length / 2.0),
                    turn(-head, 0.0),
                    turn(-shaft, 0.0),
                ];
                painter.add(Shape::convex_polygon(
                    vec![turn(head, 0.0), turn(0.0, length / 2.0), turn(-head, 0.0)],
                    theme::CURSOR_ARROW,
                    Stroke::NONE,
                ));
                let bar = Rect::from_two_pos(turn(-shaft, -length / 2.0), turn(shaft, 0.0));
                painter.rect_filled(bar, CornerRadius::ZERO, theme::CURSOR_ARROW);
                painter.add(Shape::closed_line(
                    shape,
                    Stroke::new(1.0, theme::CURSOR_FILL),
                ));
            }
        }
    }

    fn handle_wheel(&mut self, ui: &Ui, col_width: f32) {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            let Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            if modifiers.command {
                let notches = match unit {
                    MouseWheelUnit::Point => delta.y / theme::WHEEL_NOTCH,
                    _ => delta.y,
                };
                self.view.wheel_zoom += notches;
                continue;
            }
            let delta = if modifiers.shift && delta.x == 0.0 {
                vec2(delta.y, 0.0)
            } else {
                delta
            };
            let (rows, cols) = match unit {
                MouseWheelUnit::Line => (delta.y * theme::WHEEL_ROWS_PER_NOTCH, delta.x),
                MouseWheelUnit::Point => (
                    delta.y / theme::WHEEL_NOTCH * theme::WHEEL_ROWS_PER_NOTCH,
                    delta.x / col_width.max(1.0),
                ),
                MouseWheelUnit::Page => (
                    delta.y * self.view.rows as f32,
                    delta.x * self.view.cols as f32,
                ),
            };
            self.view.wheel_rows -= rows;
            self.view.wheel_cols -= cols;
        }
        let steps = self.view.wheel_zoom.trunc();
        if steps != 0.0 {
            self.view.wheel_zoom -= steps;
            self.set_zoom(self.zoom + steps * theme::ZOOM_STEP);
        }
        let rows = self.view.wheel_rows.trunc();
        let cols = self.view.wheel_cols.trunc();
        self.view.wheel_rows -= rows;
        self.view.wheel_cols -= cols;
        self.view.scroll(rows as i64, cols as i64);
    }

    fn handle_mouse(&mut self, ui: &Ui, screen: &Screen, response: &Response, digit: f32) {
        let ctx = ui.ctx().clone();
        let (pressed, down, pos, shift) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
                i.modifiers.shift,
            )
        });
        let Some(pos) = pos else {
            return;
        };
        let covered = [self.issue_tag_rect, self.list_button_rect]
            .into_iter()
            .flatten()
            .any(|rect| rect.contains(pos));
        if covered {
            return;
        }
        let col_border = screen
            .col_header
            .contains(pos)
            .then(|| screen.col_border(pos.x))
            .flatten();
        let row_border = screen
            .row_header
            .contains(pos)
            .then(|| screen.row_border(pos.y))
            .flatten();
        let handle = (self.edit.is_none() && self.fill_allowed())
            .then(|| screen.fill_handle(self.selected()))
            .filter(|rect| rect.contains(pos) && screen.cells.contains(pos));
        let outer = screen.range_rect(self.selected());
        let on_border = self.edit.is_none()
            && self.extra.is_empty()
            && handle.is_none()
            && screen.cells.contains(pos)
            && outer.expand(theme::MOVE_GRIP).contains(pos)
            && !outer.shrink(theme::MOVE_GRIP).contains(pos);
        let moving = matches!(self.drag, Some(Drag::Move { .. }));
        if response.hovered() || self.resize.is_some() {
            if handle.is_some() || self.drag == Some(Drag::Fill) {
                ctx.set_cursor_icon(CursorIcon::Crosshair);
            } else if (on_border && self.drag.is_none()) || moving {
                ctx.set_cursor_icon(CursorIcon::Move);
            } else if col_border.is_some() || self.resize.as_ref().is_some_and(|r| r.columns) {
                ctx.set_cursor_icon(CursorIcon::ResizeColumn);
            } else if row_border.is_some() || self.resize.is_some() {
                ctx.set_cursor_icon(CursorIcon::ResizeRow);
            } else if screen.cells.contains(pos) {
                self.soft_cursor = Some(SoftCursor::Cell);
            } else if screen.col_header.contains(pos) {
                self.soft_cursor = Some(SoftCursor::Down);
            } else if screen.row_header.contains(pos) {
                self.soft_cursor = Some(SoftCursor::Right);
            }
        }
        if response.hovered() && ui.input(|i| i.pointer.secondary_pressed()) {
            if self.edit.is_some() && !self.commit_edit(&ctx) {
                return;
            }
            self.context_target = Some(self.prepare_context(screen, pos));
        }

        if response.double_clicked() && (col_border.is_some() || row_border.is_some()) {
            self.resize = None;
            self.drag = None;
            match (col_border, row_border) {
                (Some(col), _) => self.autofit_columns(ui, self.resized(col, true), digit),
                (_, Some(row)) => self.autofit_rows(ui, self.resized(row, false)),
                _ => {}
            }
            return;
        }

        let grip = self.handle_at(screen, pos);
        let object = match grip {
            Some(_) => None,
            None => self.object_at(screen, pos),
        };
        if response.hovered() && self.drag.is_none() {
            let cursor = if self.shape_tool.is_some() && screen.cells.contains(pos) {
                Some(CursorIcon::Crosshair)
            } else if let Some((hit, handle)) = grip {
                Some(handle_cursor(hit, handle))
            } else {
                object.map(|_| CursorIcon::Move)
            };
            if let Some(cursor) = cursor {
                ctx.set_cursor_icon(cursor);
                self.soft_cursor = None;
            }
        }
        if response.double_clicked()
            && let Some(hit) = object
        {
            self.drag = None;
            self.object_drag = None;
            match hit.object {
                Object::Chart(index) => self.open_chart_title(index),
                Object::Shape(index) => self.begin_shape_text(index),
                Object::Picture(_) => {}
            }
            return;
        }
        if response.double_clicked() && handle.is_some() {
            self.drag = None;
            self.fill_target = None;
            self.fill_to_neighbor();
            return;
        }

        if pressed && response.hovered() {
            self.commit_shape_text();
            let start = if self.shape_tool.is_some() && screen.cells.contains(pos) {
                Some(ObjectDrag::new(None, pos, (pos, pos), (2, 2)))
            } else if let Some((hit, handle)) = grip {
                let from = objects::ends(hit.rect, hit.flip);
                let mut drag = ObjectDrag::new(Some(hit.object), pos, from, handle);
                drag.aspect = hit.aspect;
                Some(drag)
            } else {
                object.map(|hit| {
                    let from = objects::ends(hit.rect, hit.flip);
                    ObjectDrag::new(Some(hit.object), pos, from, (0, 0))
                })
            };
            if let Some(drag) = start {
                if self.edit.is_some() && !self.commit_edit(&ctx) {
                    return;
                }
                self.object = drag.object;
                self.object_drag = Some(drag);
                self.drag = Some(Drag::Object);
                return;
            }
            self.object = None;
            if let Some(col) = self.filter_button_at(screen, pos) {
                self.open_filter_menu(col, screen.filter_button(self.filter_header(col)));
                return;
            }
            if handle.is_some() {
                self.drag = Some(Drag::Fill);
                self.fill_target = None;
                return;
            }
            if on_border {
                let range = self.selected();
                let hit = screen.hit(pos);
                self.drag = Some(Drag::Move {
                    grab_rows: hit
                        .row()
                        .saturating_sub(range.start().row())
                        .min(range.rows() - 1),
                    grab_cols: hit
                        .col()
                        .saturating_sub(range.start().col())
                        .min(range.cols() - 1),
                });
                self.move_target = None;
                return;
            }
            if let Some((index, columns)) = col_border
                .map(|c| (c, true))
                .or(row_border.map(|r| (r, false)))
            {
                let axis = if columns { &screen.cols } else { &screen.rows };
                self.resize = Some(Resize {
                    columns,
                    indices: self.resized(index, columns),
                    start: if columns { pos.x } else { pos.y },
                    original: axis.size(index),
                    size: axis.size(index),
                });
                self.drag = Some(Drag::Resize);
                return;
            }
            if screen.cells.contains(pos)
                && let Some(edit) = self.edit.as_mut()
                && edit.can_point()
            {
                let addr = screen.hit(pos);
                let anchor = match edit.pointing() {
                    Some(point) if shift => point.anchor,
                    _ => addr,
                };
                edit.point_at(anchor, addr);
                edit.focus_cell = true;
                self.drag = Some(Drag::Point);
                return;
            }
            if self.edit.is_some() && !self.commit_edit(&ctx) {
                return;
            }
            if screen.corner.contains(pos) {
                self.select_all();
                return;
            }
            let drag = if screen.col_header.contains(pos) {
                Drag::Cols
            } else if screen.row_header.contains(pos) {
                Drag::Rows
            } else {
                Drag::Cells
            };
            let addr = screen.hit(pos);
            let ctrl = ui.input(|i| i.modifiers.command);
            if ctrl && !shift {
                self.extra.push(self.selected());
            } else if !shift {
                self.extra.clear();
            }
            if !shift {
                self.sel = match drag {
                    Drag::Cols => Selection {
                        active: CellAddr::new(self.view.top, addr.col()).expect("valid"),
                        anchor: CellAddr::new(0, addr.col()).expect("valid"),
                        corner: addr,
                    },
                    Drag::Rows => Selection {
                        active: CellAddr::new(addr.row(), self.view.left).expect("valid"),
                        anchor: CellAddr::new(addr.row(), 0).expect("valid"),
                        corner: addr,
                    },
                    _ => {
                        let merged = self.sheet().layout().merge_at(addr);
                        let active = merged.map_or(addr, |m| m.start());
                        Selection {
                            active,
                            anchor: active,
                            corner: merged.map_or(addr, |m| m.end()),
                        }
                    }
                };
            }
            self.drag = Some(drag);
            if shift || drag != Drag::Cells {
                self.drag_to(addr, drag);
            }
        } else if down && self.drag == Some(Drag::Resize) {
            if let Some(resize) = self.resize.as_mut() {
                let now = if resize.columns { pos.x } else { pos.y };
                resize.size = (resize.original + now - resize.start).max(0.0).round();
            }
        } else if down
            && self.drag == Some(Drag::Point)
            && let Some(edit) = self.edit.as_mut()
            && let Some(point) = edit.pointing()
        {
            let addr = screen.hit(pos);
            if point.corner != addr {
                edit.point_at(point.anchor, addr);
                self.view.reveal(addr);
            }
            if !screen.cells.contains(pos) {
                ctx.request_repaint();
            }
        } else if down && self.drag == Some(Drag::Object) {
            if let Some(drag) = self.object_drag.as_mut() {
                drag.update(pos);
            }
        } else if down
            && let Some(Drag::Move {
                grab_rows,
                grab_cols,
            }) = self.drag
        {
            let range = self.selected();
            let hit = screen.hit(pos);
            let row = hit
                .row()
                .saturating_sub(grab_rows)
                .min(MAX_ROWS - range.rows());
            let col = hit
                .col()
                .saturating_sub(grab_cols)
                .min(MAX_COLS - range.cols());
            self.move_target = CellAddr::new(row, col).ok();
            self.view.reveal(hit);
            if !screen.cells.contains(pos) {
                ctx.request_repaint();
            }
        } else if down && self.drag == Some(Drag::Fill) {
            let addr = screen.hit(pos);
            self.fill_target = fill_target(self.selected(), addr);
            self.view.reveal(addr);
            if !screen.cells.contains(pos) {
                ctx.request_repaint();
            }
        } else if down && let Some(drag @ (Drag::Cells | Drag::Rows | Drag::Cols)) = self.drag {
            self.drag_to(screen.hit(pos), drag);
            if !screen.cells.contains(pos) {
                ctx.request_repaint();
            }
        }
        if !down {
            match self.drag {
                Some(Drag::Resize) => self.finish_resize(digit),
                Some(Drag::Cells | Drag::Rows | Drag::Cols) => {
                    self.drag = None;
                    self.apply_format_painter();
                }
                Some(Drag::Point) => self.drag = None,
                Some(Drag::Object) => self.finish_object(screen),
                Some(Drag::Fill) => {
                    let copy = ui.input(|i| i.modifiers.command);
                    self.finish_fill(copy);
                }
                Some(Drag::Move { .. }) => {
                    self.drag = None;
                    if let Some(to) = self.move_target.take() {
                        let copy = ui.input(|i| i.modifiers.command);
                        self.finish_move(to, copy);
                    }
                }
                _ => {}
            }
        }
        if response.double_clicked() && screen.cells.contains(pos) {
            let text = self.active_input();
            self.start_edit(text, EditMode::Edit, true);
        }
    }

    fn object_hit(&self, screen: &Screen, object: Object) -> Option<Hit> {
        let frame = object.frame(self.sheet().layout())?;
        let dragged = self
            .object_drag
            .filter(|drag| self.drag == Some(Drag::Object) && drag.object == Some(object));
        let (rect, flip) = match dragged {
            Some(drag) if matches!(object, Object::Shape(_)) => (drag.rect(), drag.flip()),
            Some(drag) => (drag.rect(), (false, false)),
            None => (screen.object_rect(&frame), frame.flip),
        };
        Some(Hit {
            object,
            rect,
            flip,
            line: frame.line,
            aspect: frame.aspect,
            clip: screen.object_clip(frame.anchor),
        })
    }

    fn object_at(&self, screen: &Screen, pos: Pos2) -> Option<Hit> {
        if !screen.cells.contains(pos) {
            return None;
        }
        Object::all(self.sheet().layout()).rev().find_map(|object| {
            let hit = self.object_hit(screen, object)?;
            let inside = if hit.line {
                let (a, b) = objects::ends(hit.rect, hit.flip);
                objects::near_segment(pos, a, b, theme::LINE_HIT)
            } else {
                hit.rect.contains(pos)
            };
            (inside && hit.clip.contains(pos)).then_some(hit)
        })
    }

    fn handle_at(&self, screen: &Screen, pos: Pos2) -> Option<(Hit, (u8, u8))> {
        let hit = self.object_hit(screen, self.object?)?;
        if !hit.clip.contains(pos) {
            return None;
        }
        objects::handles(hit.rect, hit.flip, hit.line)
            .into_iter()
            .find(|(p, _)| (*p - pos).length() <= theme::CHART_GRIP)
            .map(|(_, handle)| (hit, handle))
    }

    fn place(&self, screen: &Screen, rect: Rect) -> (CellAddr, (u32, u32), (u32, u32)) {
        let corner = rect.min.max(screen.cells.min);
        let anchor = screen.hit(corner);
        let cell = screen.cell_rect(anchor);
        let zoom = screen.zoom;
        let offset = (
            ((rect.min.x - cell.min.x) / zoom).max(0.0).round() as u32,
            ((rect.min.y - cell.min.y) / zoom).max(0.0).round() as u32,
        );
        let size = (
            (rect.width() / zoom).round() as u32,
            (rect.height() / zoom).round() as u32,
        );
        (anchor, offset, size)
    }

    fn finish_object(&mut self, screen: &Screen) {
        self.drag = None;
        let Some(drag) = self.object_drag.take() else {
            return;
        };
        let Some(object) = drag.object else {
            let Some(kind) = self.shape_tool else {
                return;
            };
            let mut shape = DrawnShape::new(kind, crate::app::origin());
            let drawn = drag.rect();
            let small =
                drawn.width() < theme::DRAW_THRESHOLD && drawn.height() < theme::DRAW_THRESHOLD;
            let rect = if small {
                Rect::from_min_size(
                    drag.press,
                    vec2(shape.size.0 as f32, shape.size.1 as f32) * screen.zoom,
                )
            } else {
                drawn
            };
            (shape.anchor, shape.offset, shape.size) = self.place(screen, rect);
            if kind.is_line() && !small {
                (shape.flip_h, shape.flip_v) = drag.flip();
            }
            self.add_shape(shape);
            return;
        };
        if drag.now == drag.from {
            return;
        }
        let (anchor, offset, size) = self.place(screen, drag.rect());
        let boxed = (size.0.max(1), size.1.max(1));
        let flip = drag.flip();
        let least = theme::CHART_MIN_SIZE as u32;
        self.doc.set_layout(|layout| match object {
            Object::Picture(index) => {
                if let Some(picture) = layout.pictures.get_mut(index) {
                    (picture.anchor, picture.offset, picture.size) = (anchor, offset, boxed);
                }
            }
            Object::Chart(index) => {
                if let Some(chart) = layout.charts.get_mut(index) {
                    chart.anchor = anchor;
                    chart.offset = offset;
                    chart.size = (size.0.max(least), size.1.max(least));
                }
            }
            Object::Shape(index) => {
                if let Some(shape) = layout.shapes.get_mut(index) {
                    shape.anchor = anchor;
                    shape.offset = offset;
                    shape.size = if shape.kind.is_line() { size } else { boxed };
                    (shape.flip_h, shape.flip_v) = flip;
                }
            }
        });
    }

    fn paint_objects(&self, painter: &Painter, screen: &Screen) {
        let sheet = self.sheet();
        let layout = sheet.layout();
        let editing = self.shape_edit.as_ref().map(|edit| edit.index);
        for object in Object::all(layout) {
            let Some(hit) = self.object_hit(screen, object) else {
                continue;
            };
            if !hit.rect.intersects(screen.cells) {
                continue;
            }
            let painter = painter.with_clip_rect(hit.clip);
            match object {
                Object::Picture(index) => {
                    self.paint_picture(&painter, hit.rect, &layout.pictures[index]);
                }
                Object::Chart(index) => {
                    let chart = &layout.charts[index];
                    let data = vellum_grid_core::chart_data(sheet, chart.data, locale());
                    charts::paint(&painter, hit.rect, chart, &data);
                }
                Object::Shape(index) if editing == Some(index) => {
                    let mut shape = layout.shapes[index].clone();
                    shape.text.clear();
                    objects::paint_shape(&painter, hit.rect, &shape, hit.flip, screen.zoom);
                }
                Object::Shape(index) => {
                    let shape = &layout.shapes[index];
                    objects::paint_shape(&painter, hit.rect, shape, hit.flip, screen.zoom);
                }
            }
            if self.object == Some(object) {
                objects::paint_selection(&painter, hit.rect, hit.flip, hit.line);
            }
        }
        if let (Some(drag), Some(kind)) = (self.object_drag, self.shape_tool)
            && drag.object.is_none()
            && self.drag == Some(Drag::Object)
        {
            let shape = DrawnShape::new(kind, crate::app::origin());
            let flip = if kind.is_line() {
                drag.flip()
            } else {
                (false, false)
            };
            let painter = painter.with_clip_rect(screen.cells);
            objects::paint_shape(&painter, drag.rect(), &shape, flip, screen.zoom);
        }
    }

    fn shape_editor(&mut self, ui: &mut Ui, screen: &Screen) {
        let Some(index) = self.shape_edit.as_ref().map(|edit| edit.index) else {
            return;
        };
        let hit = self.object_hit(screen, Object::Shape(index));
        let shape = self.sheet().layout().shapes.get(index).cloned();
        let (Some(hit), Some(shape)) = (hit, shape) else {
            self.shape_edit = None;
            return;
        };
        let area = objects::text_area(hit.rect, screen.zoom);
        ui.set_clip_rect(hit.clip);
        let Some(edit) = self.shape_edit.as_mut() else {
            return;
        };
        let id = egui::Id::unique(SHAPE_EDITOR);
        if edit.focus {
            cursor_to_end(ui.ctx(), id, &edit.text);
        }
        let response = ui.put(
            area,
            TextEdit::multiline(&mut edit.text)
                .id(id)
                .frame(Frame::NONE)
                .margin(Margin::ZERO)
                .font(objects::text_font(&shape, screen.zoom))
                .text_color(render::color(shape.color))
                .align(egui::Align2([
                    objects::text_align(&shape),
                    egui::Align::Min,
                ]))
                .desired_width(area.width()),
        );
        if edit.focus {
            response.request_focus();
            edit.focus = false;
        } else if response.lost_focus() {
            self.commit_shape_text();
        }
    }

    fn filter_button_at(&self, screen: &Screen, pos: Pos2) -> Option<u32> {
        let filter = self.sheet().layout().filter.as_ref()?;
        (filter.range.start().col()..=filter.range.end().col())
            .find(|&col| screen.filter_button(self.filter_header(col)).contains(pos))
    }

    fn resized(&self, index: u32, columns: bool) -> Vec<u32> {
        let range = self.sel.range();
        let whole = if columns {
            range.is_full_cols()
        } else {
            range.is_full_rows()
        };
        let (start, end) = if columns {
            (range.start().col(), range.end().col())
        } else {
            (range.start().row(), range.end().row())
        };
        if whole && (start..=end).contains(&index) {
            (start..=end).collect()
        } else {
            vec![index]
        }
    }

    fn finish_resize(&mut self, digit: f32) {
        self.drag = None;
        let Some(resize) = self.resize.take() else {
            return;
        };
        if resize.size == resize.original {
            return;
        }
        self.doc.set_layout(|layout| {
            for index in &resize.indices {
                if resize.columns {
                    layout
                        .col_widths
                        .insert(*index, f64::from(resize.size / (digit * self.zoom)));
                } else {
                    layout
                        .row_heights
                        .insert(*index, f64::from(resize.size * 72.0 / 96.0 / self.zoom));
                }
            }
        });
    }

    pub fn autofit_columns(&mut self, ui: &Ui, cols: Vec<u32>, digit: f32) {
        let painter = ui.painter();
        let sheet = self.sheet();
        let widths: Vec<(u32, f64)> = cols
            .into_iter()
            .map(|col| {
                let widest = sheet
                    .cells()
                    .filter(|(addr, _)| {
                        addr.col() == col && sheet.layout().merge_at(*addr).is_none()
                    })
                    .filter_map(|(_, cell)| {
                        render::visual(
                            painter,
                            cell,
                            cell.style(),
                            f32::INFINITY,
                            self.show_formulas,
                            1.0,
                        )
                    })
                    .map(|visual| visual.extent().x)
                    .fold(0.0, f32::max);
                let px = if widest > 0.0 {
                    widest + 2.0 * theme::CELL_PADDING + 2.0
                } else {
                    Axis::columns(&Layout::default(), digit).size(0)
                };
                (col, f64::from(px / digit))
            })
            .collect();
        self.doc.set_layout(|layout| {
            for (col, width) in widths {
                layout.col_widths.insert(col, width);
            }
        });
    }

    pub fn autofit_rows(&mut self, ui: &Ui, rows: Vec<u32>) {
        let painter = ui.painter();
        let sheet = self.sheet();
        let digit = digit_width(ui);
        let cols = Axis::columns(sheet.layout(), digit);
        let heights: Vec<(u32, Option<f64>)> = rows
            .into_iter()
            .map(|row| {
                let first = CellAddr::new(row, 0).expect("valid row");
                let last = CellAddr::new(row, MAX_COLS - 1).expect("valid row");
                let tallest = sheet
                    .cells_in(CellRange::new(first, last))
                    .filter(|(addr, _)| sheet.layout().merge_at(*addr).is_none())
                    .filter_map(|(addr, cell)| {
                        let width = cols.size(addr.col());
                        let visual = render::visual(
                            painter,
                            cell,
                            cell.style(),
                            width,
                            self.show_formulas,
                            1.0,
                        )?;
                        let text = visual.extent().y;
                        Some(text.max(fonts::px(cell.style().points()) * theme::LINE_FACTOR))
                    })
                    .fold(0.0, f32::max);
                let points = (tallest > fonts::px(11.0) * theme::LINE_FACTOR)
                    .then(|| f64::from((tallest + 2.0) * 72.0 / 96.0));
                (row, points)
            })
            .collect();
        self.doc.set_layout(|layout| {
            for (row, height) in heights {
                match height {
                    Some(points) => layout.row_heights.insert(row, points),
                    None => layout.row_heights.remove(&row),
                };
            }
        });
    }

    fn apply_format_painter(&mut self) {
        if let Some(style) = self.format_painter.take() {
            let range = self.selected();
            self.doc.style_cells(range, |_, s| *s = style.clone());
        }
    }

    fn drag_to(&mut self, addr: CellAddr, drag: Drag) {
        let (corner, reveal) = match drag {
            Drag::Cols => (
                CellAddr::new(MAX_ROWS - 1, addr.col()).expect("valid"),
                CellAddr::new(self.view.top, addr.col()).expect("valid"),
            ),
            Drag::Rows => (
                CellAddr::new(addr.row(), MAX_COLS - 1).expect("valid"),
                CellAddr::new(addr.row(), self.view.left).expect("valid"),
            ),
            _ => (addr, addr),
        };
        self.sel.corner = corner;
        self.view.reveal(reveal);
    }

    fn scrollbar(&mut self, ui: &Ui, screen: &Screen, vertical: bool) {
        let rect = if vertical { screen.vbar } else { screen.hbar };
        let (rows, cols) = self.sheet().extent();
        let (position, page, used) = if vertical {
            (self.view.top, self.view.rows, rows)
        } else {
            (self.view.left, self.view.cols, cols)
        };
        let frozen = match self.drag {
            Some(Drag::VerticalBar { total, .. }) if vertical => Some(total),
            Some(Drag::HorizontalBar { total, .. }) if !vertical => Some(total),
            _ => None,
        };
        let total = frozen.unwrap_or_else(|| used.max(position + page).max(page * 2));
        let along = |p: Pos2| if vertical { p.y } else { p.x };
        let (start, length) = if vertical {
            (rect.top(), rect.height())
        } else {
            (rect.left(), rect.width())
        };
        let thumb_len = (length * page as f32 / total as f32)
            .clamp(theme::SCROLLBAR_MIN_THUMB.min(length), length);
        let span = (total - page.min(total)).max(1) as f32;
        let thumb_start = start + (length - thumb_len) * (position as f32 / span).min(1.0);
        let thumb = if vertical {
            Rect::from_min_size(
                pos2(rect.left(), thumb_start),
                vec2(rect.width(), thumb_len),
            )
        } else {
            Rect::from_min_size(
                pos2(thumb_start, rect.top()),
                vec2(thumb_len, rect.height()),
            )
        };

        let id = ui.scope_id().with(("scrollbar", vertical));
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let (pressed, down, pointer) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
            )
        });
        if let Some(p) = pointer {
            if pressed && response.hovered() {
                if thumb.contains(p) {
                    let grab = along(p) - thumb_start;
                    self.drag = Some(if vertical {
                        Drag::VerticalBar { grab, total }
                    } else {
                        Drag::HorizontalBar { grab, total }
                    });
                } else {
                    let pages = if along(p) < thumb_start { -1 } else { 1 };
                    let amount = i64::from(page) * pages;
                    if vertical {
                        self.view.scroll(amount, 0);
                    } else {
                        self.view.scroll(0, amount);
                    }
                }
            } else if down && frozen.is_some() {
                let fraction = ((along(p) - grab_of(self.drag) - start)
                    / (length - thumb_len).max(1.0))
                .clamp(0.0, 1.0);
                let target = (fraction * span).round() as u32;
                if vertical {
                    self.view.top = target.min(MAX_ROWS - 1);
                } else {
                    self.view.left = target.min(MAX_COLS - 1);
                }
            }
        }
        if !down && frozen.is_some() {
            self.drag = None;
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::ZERO, theme::scrollbar_track());
        let color = if frozen.is_some() || response.hovered() {
            theme::scrollbar_thumb_active()
        } else {
            theme::scrollbar_thumb()
        };
        let radius = CornerRadius::same((theme::SCROLLBAR_SIZE / 2.0) as u8);
        painter.rect_filled(thumb.shrink(3.0), radius, color);
    }

    fn paint(&self, ui: &Ui, screen: &Screen) {
        let ppp = ui.pixels_per_point();
        let painter = ui.painter_at(screen.full);
        let layout = self.sheet().layout();
        let range = self.selected();

        painter.rect_filled(screen.full, CornerRadius::ZERO, theme::header_bg());
        painter.rect_filled(screen.cells, CornerRadius::ZERO, theme::cell_bg());
        for pane in screen.panes() {
            if pane.clip.is_positive() {
                self.paint_pane(
                    &painter.with_clip_rect(pane.clip),
                    screen,
                    pane.visible,
                    ppp,
                );
            }
        }
        let divider = Stroke::new(1.0, theme::freeze_line());
        if screen.frozen_rows > 0 {
            let y = (screen.cells.top() + screen.frozen_h).round_to_pixel_center(ppp);
            painter.hline(screen.cells.x_range(), y, divider);
        }
        if screen.frozen_cols > 0 {
            let x = (screen.cells.left() + screen.frozen_w).round_to_pixel_center(ppp);
            painter.vline(x, screen.cells.y_range(), divider);
        }
        self.paint_objects(&painter, screen);
        if let Some(resize) = &self.resize {
            let guide = Stroke::new(1.0, theme::chrome_text_muted());
            for index in &resize.indices {
                if resize.columns {
                    let x = screen.x(*index) + screen.cols.size(*index);
                    painter.vline(x, screen.cells.y_range(), guide);
                } else {
                    let y = screen.y(*index) + screen.rows.size(*index);
                    painter.hline(screen.cells.x_range(), y, guide);
                }
            }
        }
        if !layout.hide_headings {
            self.paint_headers(&painter, screen, range, ppp);
        }
        if let Some(resize) = &self.resize {
            resize_tip(&painter, screen, resize, self.digit);
        }
    }

    fn paint_pane(&self, cells: &Painter, screen: &Screen, visible: CellRange, ppp: f32) {
        let sheet = self.sheet();
        let layout = sheet.layout();
        let range = self.selected();
        let editing = self.edit.as_ref().map(|edit| edit.addr);
        let pane = cells.clip_rect();

        let merges: Vec<CellRange> = layout
            .merges
            .iter()
            .copied()
            .filter(|m| vellum_grid_core::overlaps(*m, visible))
            .collect();
        let merged = |addr: CellAddr| merges.iter().any(|m| m.contains(addr));
        let highlighter = Highlighter::new(
            self.doc.book(),
            self.doc.active(),
            visible,
            self.doc.today(),
        );
        let mut graphics = Vec::new();

        let mut joined = HashSet::new();
        let mut contents: Vec<(Rect, Pos2, Visual)> = Vec::new();
        let mut fills = Vec::new();
        let mut patterns = Vec::new();
        let mut borders = Vec::new();
        for merge in &merges {
            let rect = screen.range_rect(*merge);
            if !rect.is_positive() {
                continue;
            }
            if let Some(cell) = sheet.get(merge.start()) {
                let style = self.effective_style(&highlighter, merge.start(), cell);
                if let Some(fill) = style.fill {
                    fills.push((rect, fill));
                }
                if let Some(pattern) = style.pattern {
                    patterns.push((rect, pattern));
                }
                borders.push((rect, style.borders));
                if editing != Some(merge.start())
                    && let Some(visual) = render::visual(
                        cells,
                        cell,
                        &style,
                        rect.width(),
                        self.show_formulas,
                        self.zoom,
                    )
                {
                    let pos = render::position(&visual, rect);
                    contents.push((rect, pos, visual));
                }
            }
        }
        let tabled = self.paint_table_backgrounds(
            visible,
            |addr| screen.cell_rect(addr),
            |addr| merged(addr) || layout.is_hidden(addr),
        );
        for (rect, fill, table_borders) in tabled {
            if let Some(fill) = fill {
                fills.push((rect, fill));
            }
            borders.push((rect, table_borders));
        }
        for range in highlighter.empty_ranges() {
            if !vellum_grid_core::overlaps(range, visible) {
                continue;
            }
            let top = range.start().row().max(visible.start().row());
            let bottom = range.end().row().min(visible.end().row());
            let left = range.start().col().max(visible.start().col());
            let right = range.end().col().min(visible.end().col());
            for row in top..=bottom {
                for col in left..=right {
                    let at = addr(row, col);
                    if sheet.get(at).is_some() || merged(at) || layout.is_hidden(at) {
                        continue;
                    }
                    let mut style = Style::default();
                    let empty = vellum_grid_core::CellValue::Empty;
                    crate::data::apply_highlight(&mut style, &highlighter.highlight(at, &empty));
                    let rect = screen.cell_rect(at);
                    if let Some(fill) = style.fill {
                        fills.push((rect, fill));
                    }
                    if style.borders != Default::default() {
                        borders.push((rect, style.borders));
                    }
                }
            }
        }
        for (addr, cell) in sheet.cells_in(visible) {
            if merged(addr) || layout.is_hidden(addr) {
                continue;
            }
            let rect = screen.cell_rect(addr);
            let (style, extras) = self.effective(&highlighter, addr, cell);
            if let Some(fill) = style.fill {
                fills.push((rect, fill));
            }
            if let Some(pattern) = style.pattern {
                patterns.push((rect, pattern));
            }
            if style.borders != Default::default() {
                borders.push((rect, style.borders));
            }
            if extras.bar.is_some() || extras.icon.is_some() {
                graphics.push((rect, extras));
            }
            if editing == Some(addr) || extras.hide_value {
                continue;
            }
            let Some(visual) = render::visual(
                cells,
                cell,
                &style,
                rect.width(),
                self.show_formulas,
                self.zoom,
            ) else {
                continue;
            };
            let mut rect = rect;
            if style.h_align == HAlign::CenterAcross {
                let mut col = addr.col() + 1;
                while col <= visible.end().col() {
                    let next = CellAddr::new(addr.row(), col).expect("visible cell");
                    let across = sheet.get(next).is_some_and(|c| {
                        !c.has_content() && c.style().h_align == HAlign::CenterAcross
                    });
                    if !across || merged(next) {
                        break;
                    }
                    joined.insert((addr.row(), col));
                    rect.max.x += screen.cols.size(col);
                    col += 1;
                }
            }
            let mut clip = rect;
            if visual.overflow {
                let mut col = addr.col() + 1;
                while clip.width() - 2.0 * theme::CELL_PADDING < visual.galley.size().x
                    && col <= visible.end().col()
                {
                    let next = CellAddr::new(addr.row(), col).expect("visible cell");
                    if sheet.get(next).is_some_and(|c| c.has_content())
                        || editing == Some(next)
                        || merged(next)
                    {
                        break;
                    }
                    joined.insert((addr.row(), col));
                    clip.max.x += screen.cols.size(col);
                    col += 1;
                }
            }
            let pos = render::position(&visual, rect);
            contents.push((clip, pos, visual));
        }

        if !layout.hide_gridlines {
            let line = Stroke::new(1.0, theme::grid_line());
            let inside =
                |a: CellAddr, b: CellAddr| merges.iter().any(|m| m.contains(a) && m.contains(b));
            for row in visible.start().row()..=visible.end().row() + 1 {
                let y = screen.y(row).round_to_pixel_center(ppp);
                for col in visible.start().col()..=visible.end().col() {
                    let x = screen.x(col);
                    let skip =
                        row > 0 && row < MAX_ROWS && inside(addr(row - 1, col), addr(row, col));
                    if !skip {
                        cells.hline(x..=x + screen.cols.size(col), y, line);
                    }
                }
            }
            for col in visible.start().col()..=visible.end().col() + 1 {
                let x = screen.x(col).round_to_pixel_center(ppp);
                for row in visible.start().row()..=visible.end().row() {
                    let skip = joined.contains(&(row, col))
                        || (col > 0
                            && col < MAX_COLS
                            && inside(addr(row, col - 1), addr(row, col)));
                    if !skip {
                        let y = screen.y(row);
                        cells.vline(x, y..=y + screen.rows.size(row), line);
                    }
                }
            }
        }
        for (rect, fill) in fills {
            cells.rect_filled(rect, CornerRadius::ZERO, render::cell_color(fill));
        }
        for (rect, pattern) in patterns {
            render::paint_pattern(cells, rect, pattern);
        }
        for group in &layout.sparklines {
            for (location, data) in &group.cells {
                if !visible.contains(*location) || layout.is_hidden(*location) {
                    continue;
                }
                let rect = screen.cell_rect(*location);
                let values = vellum_grid_core::spark_values(sheet, *data);
                crate::sparklines_ui::paint(
                    &cells.with_clip_rect(rect.intersect(pane)),
                    rect,
                    group,
                    &values,
                );
            }
        }
        for (rect, extras) in &graphics {
            crate::conditional_ui::paint_extras(
                &cells.with_clip_rect(rect.intersect(pane)),
                *rect,
                extras.bar.as_ref(),
                extras.icon,
            );
        }

        let active_area = layout
            .merge_at(self.sel.active)
            .unwrap_or(CellRange::single(self.sel.active));
        let active = screen.range_rect(active_area);
        let outer = screen.range_rect(range);
        for part in [
            Rect::from_min_max(outer.min, pos2(outer.right(), active.top())),
            Rect::from_min_max(pos2(outer.left(), active.bottom()), outer.max),
            Rect::from_min_max(
                pos2(outer.left(), active.top()),
                pos2(active.left(), active.bottom()),
            ),
            Rect::from_min_max(
                pos2(active.right(), active.top()),
                pos2(outer.right(), active.bottom()),
            ),
        ] {
            if part.is_positive() {
                cells.rect_filled(part, CornerRadius::ZERO, theme::selection_fill());
            }
        }
        for extra in &self.extra {
            cells.rect_filled(
                screen.range_rect(*extra),
                CornerRadius::ZERO,
                theme::selection_fill(),
            );
        }

        for (rect, cell_borders) in borders {
            render::paint_borders(cells, rect, &cell_borders);
        }
        for (clip, pos, visual) in contents {
            let clip = clip.shrink2(vec2(1.0, 0.0)).intersect(pane);
            render::paint_text(&cells.with_clip_rect(clip), &visual, pos);
        }
        self.paint_filter_buttons(cells, screen, visible);

        let border = Stroke::new(theme::SELECTION_BORDER, theme::accent());
        cells.rect_stroke(outer, CornerRadius::ZERO, border, StrokeKind::Middle);
        if self.fill_allowed() {
            let handle =
                Rect::from_center_size(outer.max, vec2(theme::FILL_HANDLE, theme::FILL_HANDLE));
            cells.rect_filled(handle.expand(1.0), CornerRadius::ZERO, theme::cell_bg());
            cells.rect_filled(handle, CornerRadius::ZERO, theme::accent());
        }
        if self.page_break_view {
            self.paint_page_breaks(cells, screen);
        }
        if let Some(target) = self.fill_target {
            marquee(cells, screen.range_rect(target), theme::fill_preview());
        }
        if let (Some(Drag::Move { .. }), Some(to)) = (self.drag, self.move_target) {
            let end = to.offset(i64::from(range.rows()) - 1, i64::from(range.cols()) - 1);
            cells.rect_stroke(
                screen.range_rect(CellRange::new(to, end)),
                CornerRadius::ZERO,
                Stroke::new(theme::SELECTION_BORDER, theme::fill_preview()),
                StrokeKind::Middle,
            );
        }
        if let Some(clip) = self
            .clipboard
            .as_ref()
            .filter(|c| c.sheet == self.doc.active())
        {
            marquee(
                cells,
                screen.range_rect(clip.range),
                theme::clipboard_marquee(),
            );
        }
        if let Some(edit) = &self.edit {
            let sheet = self.doc.active();
            for part in references::colored(&edit.text, self.doc.book(), sheet) {
                if let Some((_, range)) = part.target.filter(|(index, _)| *index == sheet) {
                    reference_box(cells, screen.range_rect(range), part.color);
                }
            }
        }
        if let Some(point) = self.edit.as_ref().and_then(|edit| edit.point) {
            marquee(cells, screen.range_rect(point.range()), theme::REFERENCE);
        }
        for addr in self
            .issue_cells
            .iter()
            .filter(|addr| visible.contains(**addr))
        {
            let corner = screen.range_rect(CellRange::single(*addr)).left_top() + vec2(1.0, 1.0);
            cells.add(Shape::convex_polygon(
                vec![
                    corner,
                    corner + vec2(theme::ISSUE_MARK_SIZE, 0.0),
                    corner + vec2(0.0, theme::ISSUE_MARK_SIZE),
                ],
                theme::ISSUE_MARK,
                Stroke::NONE,
            ));
        }
        for addr in layout.notes.keys().filter(|addr| visible.contains(**addr)) {
            let cell = screen.range_rect(CellRange::single(*addr));
            crate::review_ui::indicator(cells, cell, theme::NOTE_MARK);
        }
        for (addr, comment) in layout
            .comments
            .iter()
            .filter(|(addr, _)| visible.contains(**addr))
        {
            let cell = screen.range_rect(CellRange::single(*addr));
            let color = if comment.resolved {
                theme::COMMENT_RESOLVED
            } else {
                theme::COMMENT_MARK
            };
            crate::review_ui::indicator(cells, cell, color);
        }
        for addr in self
            .circle_cells
            .iter()
            .filter(|addr| visible.contains(**addr))
        {
            crate::validation_ui::paint_circle(cells, screen.range_rect(CellRange::single(*addr)));
        }
        crate::trace::paint(cells, &self.doc, &self.arrows, self.doc.active(), |range| {
            screen.range_rect(range)
        });
    }

    fn paint_page_breaks(&self, painter: &Painter, screen: &Screen) {
        let sheet = self.sheet();
        let Some(pagination) = vellum_grid_core::page::paginate(sheet) else {
            return;
        };
        let pane = painter.clip_rect();
        let area = screen.range_rect(pagination.area);
        for outside in [
            Rect::from_min_max(pane.min, pos2(pane.right(), area.top())),
            Rect::from_min_max(pos2(pane.left(), area.bottom()), pane.max),
            Rect::from_min_max(
                pos2(pane.left(), area.top()),
                pos2(area.left(), area.bottom()),
            ),
            Rect::from_min_max(
                pos2(area.right(), area.top()),
                pos2(pane.right(), area.bottom()),
            ),
        ] {
            if outside.is_positive() {
                painter.rect_filled(outside, CornerRadius::ZERO, theme::BREAK_OUTSIDE);
            }
        }
        let setup = &sheet.layout().page;
        let manual = Stroke::new(theme::BREAK_WIDTH, theme::BREAK_LINE);
        for (number, page) in pagination.pages.iter().enumerate() {
            let range = CellRange::new(
                addr(page.rows.0, page.cols.0),
                addr(page.rows.1, page.cols.1),
            );
            let rect = screen.range_rect(range);
            let galley = painter.layout_no_wrap(
                format!("{} {}", text::page_word(), number + 1),
                egui::FontId::proportional(theme::BREAK_LABEL_SIZE * screen.zoom),
                theme::BREAK_LABEL,
            );
            if rect.intersects(pane) && galley.size().x < rect.width() {
                painter.galley(
                    rect.center() - galley.size() / 2.0,
                    galley,
                    theme::BREAK_LABEL,
                );
            }
            if page.rows.0 > pagination.area.start().row() {
                let y = rect.top();
                if setup.row_breaks.contains(&page.rows.0) {
                    painter.hline(rect.x_range(), y, manual);
                } else {
                    dashed(painter, pos2(rect.left(), y), pos2(rect.right(), y));
                }
            }
            if page.cols.0 > pagination.area.start().col() {
                let x = rect.left();
                if setup.col_breaks.contains(&page.cols.0) {
                    painter.vline(x, rect.y_range(), manual);
                } else {
                    dashed(painter, pos2(x, rect.top()), pos2(x, rect.bottom()));
                }
            }
        }
        painter.rect_stroke(area, CornerRadius::ZERO, manual, StrokeKind::Middle);
    }

    fn paint_filter_buttons(&self, painter: &Painter, screen: &Screen, visible: CellRange) {
        let Some(filter) = &self.sheet().layout().filter else {
            return;
        };
        let header = filter.range.start().row();
        if !(visible.start().row()..=visible.end().row()).contains(&header) {
            return;
        }
        for col in filter.range.start().col()..=filter.range.end().col() {
            if !(visible.start().col()..=visible.end().col()).contains(&col) {
                continue;
            }
            let rect = screen.filter_button(addr(header, col));
            if !rect.is_positive() {
                continue;
            }
            painter.rect(
                rect,
                CornerRadius::same(2),
                theme::cell_bg(),
                Stroke::new(1.0, theme::header_line()),
                StrokeKind::Inside,
            );
            if filter.criteria.contains_key(&col) {
                icons::funnel(painter, rect.shrink(3.0), theme::icon());
            } else {
                icons::chevron(painter, rect.center(), theme::icon());
            }
        }
    }

    fn paint_headers(&self, painter: &Painter, screen: &Screen, range: CellRange, ppp: f32) {
        let mut font = theme::header_font();
        font.size *= screen.zoom;
        let line = Stroke::new(1.0, theme::header_line());
        let marker = Stroke::new(theme::SELECTION_BORDER, theme::accent());

        painter.rect_filled(screen.col_header, CornerRadius::ZERO, theme::header_bg());
        for segment in screen.col_segments() {
            let clip =
                Rect::from_x_y_ranges(segment.from..=segment.to, screen.col_header.y_range());
            let cols = painter.with_clip_rect(clip.intersect(screen.col_header));
            for col in segment.first..=segment.last {
                let width = screen.cols.size(col);
                if width <= 0.0 {
                    continue;
                }
                let x = screen.x(col);
                let rect = Rect::from_min_max(
                    pos2(x, screen.col_header.top()),
                    pos2(x + width, screen.col_header.bottom()),
                );
                let selected = (range.start().col()..=range.end().col()).contains(&col);
                let text = header_style(&cols, rect, selected, range.is_full_cols());
                cols.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    column_name(col),
                    font.clone(),
                    text,
                );
                cols.vline(
                    rect.right().round_to_pixel_center(ppp),
                    rect.y_range(),
                    line,
                );
                if selected && !range.is_full_cols() {
                    cols.hline(rect.x_range(), rect.bottom() - 1.0, marker);
                }
            }
        }
        painter.with_clip_rect(screen.col_header).hline(
            screen.col_header.x_range(),
            screen.col_header.bottom().round_to_pixel_center(ppp) - 1.0,
            line,
        );

        painter.rect_filled(screen.row_header, CornerRadius::ZERO, theme::header_bg());
        for segment in screen.row_segments() {
            let clip =
                Rect::from_x_y_ranges(screen.row_header.x_range(), segment.from..=segment.to);
            let rows = painter.with_clip_rect(clip.intersect(screen.row_header));
            for row in segment.first..=segment.last {
                let height = screen.rows.size(row);
                if height <= 0.0 {
                    continue;
                }
                let y = screen.y(row);
                let rect = Rect::from_min_max(
                    pos2(screen.row_header.left(), y),
                    pos2(screen.row_header.right(), y + height),
                );
                let selected = (range.start().row()..=range.end().row()).contains(&row);
                let text = header_style(&rows, rect, selected, range.is_full_rows());
                let filtered = self.sheet().layout().filter.as_ref().is_some_and(|f| {
                    !f.criteria.is_empty()
                        && f.range.start().row() < row
                        && row <= f.range.end().row()
                });
                let text = if filtered && !selected {
                    theme::FILTERED_ROW
                } else {
                    text
                };
                rows.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    (row + 1).to_string(),
                    font.clone(),
                    text,
                );
                rows.hline(
                    rect.x_range(),
                    rect.bottom().round_to_pixel_center(ppp),
                    line,
                );
                if selected && !range.is_full_rows() {
                    rows.vline(rect.right() - 1.0, rect.y_range(), marker);
                }
            }
        }
        painter.with_clip_rect(screen.row_header).vline(
            screen.row_header.right().round_to_pixel_center(ppp) - 1.0,
            screen.row_header.y_range(),
            line,
        );

        let corner = screen.corner;
        painter.rect_filled(corner, CornerRadius::ZERO, theme::header_bg());
        painter.rect_stroke(corner, CornerRadius::ZERO, line, StrokeKind::Inside);
        let all = range.is_full_rows() && range.is_full_cols();
        let tip = corner.right_bottom() - vec2(4.0, 4.0);
        let size = corner.height() * 0.5;
        painter.add(Shape::convex_polygon(
            vec![tip, tip - vec2(0.0, size), tip - vec2(size, 0.0)],
            if all {
                theme::accent()
            } else {
                theme::header_line()
            },
            Stroke::NONE,
        ));
    }

    fn editor(&mut self, ui: &mut Ui, screen: &Screen) {
        let Some(edit) = &self.edit else {
            return;
        };
        let addr = edit.addr;
        let area = self
            .sheet()
            .layout()
            .merge_at(addr)
            .unwrap_or(CellRange::single(addr));
        let style: Style = self
            .sheet()
            .get(addr)
            .map(|cell| cell.style().clone())
            .unwrap_or_default();
        let font = fonts::cell_font(&style, self.zoom);
        let cell = screen.range_rect(area);
        let Some(edit) = &mut self.edit else {
            return;
        };
        let text_width = ui
            .painter()
            .layout_no_wrap(edit.text.clone(), font.id.clone(), theme::cell_text())
            .size()
            .x
            + 2.0 * theme::CELL_PADDING
            + theme::EDITOR_SLACK;
        let width = text_width
            .max(cell.width())
            .min((screen.cells.right() - cell.left()).max(cell.width()));
        let multiline = edit.text.contains('\n');
        let lines_height = ui
            .painter()
            .layout_no_wrap(edit.text.clone(), font.id.clone(), theme::cell_text())
            .size()
            .y
            + 2.0 * theme::CELL_PADDING;
        let height = if multiline {
            lines_height.max(cell.height())
        } else {
            cell.height()
        };
        let rect = Rect::from_min_size(cell.min, vec2(width, height));

        ui.set_clip_rect(screen.cells);
        let painter = ui.painter();
        let background = style.fill.map_or(theme::cell_bg(), render::cell_color);
        painter.rect_filled(rect, CornerRadius::ZERO, background);
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(theme::SELECTION_BORDER, theme::accent()),
            StrokeKind::Middle,
        );
        if edit.cursor_to_end {
            cursor_to_end(ui.ctx(), edit.id, &edit.text);
            edit.cursor_to_end = false;
        }
        let sample = Visual {
            galley: ui
                .painter()
                .layout_no_wrap("0".into(), font.id.clone(), theme::cell_text()),
            align: HAlign::Left,
            v_align: style.v_align,
            size: font.id.size,
            faux_bold: false,
            overflow: false,
            underline: None,
            indent: 0.0,
            angle: 0.0,
            shift: 0.0,
        };
        let inner = rect.shrink(theme::SELECTION_BORDER);
        let top = (render::position(&sample, rect).y - inner.top()).max(0.0);
        let color = style.color.map_or(theme::cell_text(), render::cell_color);
        let book = self.doc.book();
        let sheet = self.doc.active();
        let font_id = font.id.clone();
        let mut layouter = |ui: &Ui, buffer: &dyn egui::TextBuffer, wrap: f32| {
            let text = buffer.as_str();
            let parts = references::colored(text, book, sheet);
            let job = references::job(text, &parts, font_id.clone(), color, wrap);
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };
        let editor = if multiline {
            TextEdit::multiline(&mut edit.text).desired_rows(1)
        } else {
            TextEdit::singleline(&mut edit.text)
        };
        let response = ui.put(
            inner,
            editor
                .layouter(&mut layouter)
                .id(edit.id)
                .event_filter(crate::app::EDITOR_KEYS)
                .frame(Frame::NONE)
                .margin(Margin {
                    left: 1,
                    right: 1,
                    top: top.round().min(f32::from(i8::MAX)) as i8,
                    bottom: 0,
                })
                .font(font.id)
                .text_color(color)
                .align(Align2::LEFT_TOP)
                .desired_width(rect.width()),
        );
        if edit.focus_cell {
            response.request_focus();
            edit.focus_cell = false;
        }
        let base = match &edit.suggestion {
            Some(suffix) if edit.text.ends_with(suffix.as_str()) => {
                edit.text[..edit.text.len() - suffix.len()].to_owned()
            }
            _ => {
                edit.suggestion = None;
                edit.text.clone()
            }
        };
        let grew = base.len() > edit.typed.len() && base.starts_with(edit.typed.as_str());
        if edit.mode == EditMode::Enter
            && edit.suggestion.is_none()
            && grew
            && let Some(suffix) = paste::completion(self.doc.sheet(), edit.addr, &base)
        {
            edit.text = format!("{base}{suffix}");
            let start = base.chars().count();
            select_chars(ui.ctx(), edit.id, start, edit.text.chars().count());
            edit.suggestion = Some(suffix);
            ui.ctx().request_repaint();
        }
        edit.typed = base;
        if response.has_focus() {
            let id = edit.id;
            self.formula_assist(ui.ctx(), id, rect);
        }
    }
}

fn resize_tip(painter: &Painter, screen: &Screen, resize: &Resize, digit: f32) {
    let Some(&index) = resize.indices.first() else {
        return;
    };
    let px = (resize.size / screen.zoom).round();
    let message = if resize.columns {
        let chars = format::chars_from_width(f64::from(px / digit), digit);
        format!(
            "{}: {} ({px} {})",
            text::width(),
            locale().format_number(chars),
            text::pixels()
        )
    } else {
        let points = f64::from(px * 72.0 / 96.0);
        format!(
            "{}: {} ({px} {})",
            text::height(),
            locale().format_number(format::round_points(points)),
            text::pixels()
        )
    };
    let anchor = if resize.columns {
        pos2(screen.x(index) + screen.cols.size(index), screen.full.top())
    } else {
        pos2(
            screen.full.left(),
            screen.y(index) + screen.rows.size(index),
        )
    };
    let galley = painter.layout_no_wrap(message, theme::small_font(), theme::chrome_text());
    let rect = Rect::from_min_size(anchor + vec2(4.0, 4.0), galley.size() + vec2(8.0, 4.0));
    painter.rect(
        rect,
        CornerRadius::ZERO,
        theme::tip_bg(),
        Stroke::new(1.0, theme::chrome_text_muted()),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(4.0, 2.0), galley, theme::chrome_text());
}

fn fill_target(range: CellRange, addr: CellAddr) -> Option<CellRange> {
    let (start, end) = (range.start(), range.end());
    let distance = |value: u32, low: u32, high: u32| {
        if value > high {
            value - high
        } else {
            low.saturating_sub(value)
        }
    };
    let rows = distance(addr.row(), start.row(), end.row());
    let cols = distance(addr.col(), start.col(), end.col());
    if rows == 0 && cols == 0 {
        return None;
    }
    let target = if rows >= cols {
        CellAddr::new(
            addr.row(),
            if addr.row() > end.row() {
                end.col()
            } else {
                start.col()
            },
        )
    } else {
        CellAddr::new(
            if addr.col() > end.col() {
                end.row()
            } else {
                start.row()
            },
            addr.col(),
        )
    };
    target.ok().map(|corner| range.including(corner))
}

fn addr(row: u32, col: u32) -> CellAddr {
    CellAddr::new(row, col).expect("visible cell")
}

fn outline_gutter(layout: &Layout) -> Vec2 {
    let span = |depth: u8| {
        if depth == 0 {
            0.0
        } else {
            f32::from(depth + 1) * theme::OUTLINE_STEP
        }
    };
    vec2(
        span(outline::max_level(&layout.row_levels)),
        span(outline::max_level(&layout.col_levels)),
    )
}

enum OutlineAction {
    Level(bool, u8),
    Toggle(bool, outline::Group),
}

fn outline_button(
    ui: &Ui,
    center: Pos2,
    label: &str,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> bool {
    let rect = Rect::from_center_size(center, vec2(theme::OUTLINE_BUTTON, theme::OUTLINE_BUTTON));
    let response = ui.interact(rect, ui.scope_id().with(id), Sense::click());
    let painter = ui.painter();
    let fill = if response.hovered() {
        theme::tool_hover()
    } else {
        theme::surface()
    };
    painter.rect_filled(rect, CornerRadius::same(2), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, theme::header_line()),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::small_font(),
        theme::header_text(),
    );
    response.clicked()
}

impl Spreadsheet {
    fn outline_ui(&mut self, ui: &mut Ui, screen: &Screen, outer: Rect, gutter: Vec2) {
        if gutter == Vec2::ZERO {
            return;
        }
        let layout = self.sheet().layout().clone();
        let painter = ui.painter().clone();
        let line = Stroke::new(1.0, theme::header_text());
        let mut action = None;
        if gutter.x > 0.0 {
            let strip =
                Rect::from_min_max(outer.min, pos2(outer.left() + gutter.x, outer.bottom()));
            painter.rect_filled(strip, CornerRadius::ZERO, theme::header_bg());
            painter.vline(
                strip.right(),
                strip.y_range(),
                Stroke::new(1.0, theme::header_line()),
            );
            let column = |level: u8| {
                outer.left()
                    + f32::from(level - 1) * theme::OUTLINE_STEP
                    + theme::OUTLINE_STEP / 2.0
            };
            let depth = outline::max_level(&layout.row_levels);
            for level in 1..=depth + 1 {
                let center = pos2(column(level), screen.col_header.center().y);
                if outline_button(ui, center, &level.to_string(), ("row-level", level)) {
                    action = Some(OutlineAction::Level(true, level));
                }
            }
            let cells = screen.cells.y_range();
            for group in outline::groups(&layout.row_levels) {
                let x = column(group.level);
                let top = screen.y(group.start).max(cells.min);
                let bottom = (screen.y(group.end) + screen.rows.size(group.end)).min(cells.max);
                if bottom > top + 2.0 {
                    painter.vline(x, top + 2.0..=bottom - 2.0, line);
                    painter.hline(x..=x + theme::OUTLINE_TICK, top + 2.0, line);
                }
                let summary = group.end + 1;
                if summary >= MAX_ROWS || layout.hidden_rows.contains(&summary) {
                    continue;
                }
                let y = screen.y(summary) + screen.rows.size(summary) / 2.0;
                if !cells.contains(y) {
                    continue;
                }
                let collapsed = outline::is_collapsed(&layout.hidden_rows, group);
                let label = if collapsed { "+" } else { "−" };
                if outline_button(ui, pos2(x, y), label, ("row-group", group.level, group.end)) {
                    action = Some(OutlineAction::Toggle(true, group));
                }
            }
        }
        if gutter.y > 0.0 {
            let strip = Rect::from_min_max(outer.min, pos2(outer.right(), outer.top() + gutter.y));
            painter.rect_filled(strip, CornerRadius::ZERO, theme::header_bg());
            painter.hline(
                strip.x_range(),
                strip.bottom(),
                Stroke::new(1.0, theme::header_line()),
            );
            let row = |level: u8| {
                outer.top() + f32::from(level - 1) * theme::OUTLINE_STEP + theme::OUTLINE_STEP / 2.0
            };
            let depth = outline::max_level(&layout.col_levels);
            for level in 1..=depth + 1 {
                let center = pos2(screen.row_header.center().x, row(level));
                if outline_button(ui, center, &level.to_string(), ("col-level", level)) {
                    action = Some(OutlineAction::Level(false, level));
                }
            }
            let cells = screen.cells.x_range();
            for group in outline::groups(&layout.col_levels) {
                let y = row(group.level);
                let left = screen.x(group.start).max(cells.min);
                let right = (screen.x(group.end) + screen.cols.size(group.end)).min(cells.max);
                if right > left + 2.0 {
                    painter.hline(left + 2.0..=right - 2.0, y, line);
                    painter.vline(left + 2.0, y..=y + theme::OUTLINE_TICK, line);
                }
                let summary = group.end + 1;
                if summary >= MAX_COLS || layout.hidden_cols.contains(&summary) {
                    continue;
                }
                let x = screen.x(summary) + screen.cols.size(summary) / 2.0;
                if !cells.contains(x) {
                    continue;
                }
                let collapsed = outline::is_collapsed(&layout.hidden_cols, group);
                let label = if collapsed { "+" } else { "−" };
                if outline_button(ui, pos2(x, y), label, ("col-group", group.level, group.end)) {
                    action = Some(OutlineAction::Toggle(false, group));
                }
            }
        }
        match action {
            Some(OutlineAction::Level(rows, level)) => {
                self.doc.show_level(rows, level);
            }
            Some(OutlineAction::Toggle(rows, group)) => {
                self.doc.toggle_group(rows, group);
            }
            None => {}
        }
    }
}

fn reference_box(painter: &Painter, rect: Rect, color: egui::Color32) {
    let [r, g, b, _] = color.to_array();
    let fill = egui::Color32::from_rgba_unmultiplied(r, g, b, theme::REFERENCE_FILL_ALPHA);
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    painter.rect_stroke(
        rect.shrink(0.5),
        CornerRadius::ZERO,
        Stroke::new(1.5, color),
        StrokeKind::Middle,
    );
    let handle = vec2(theme::REFERENCE_HANDLE, theme::REFERENCE_HANDLE);
    for corner in [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ] {
        painter.rect_filled(
            Rect::from_center_size(corner, handle),
            CornerRadius::ZERO,
            color,
        );
    }
}

fn marquee(painter: &Painter, rect: Rect, color: egui::Color32) {
    let r = rect.shrink(1.0);
    let path = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    let stroke = Stroke::new(theme::SELECTION_BORDER, color);
    painter.extend(Shape::dashed_line(
        &path,
        stroke,
        theme::MARQUEE_DASH,
        theme::MARQUEE_DASH,
    ));
}

fn grab_of(drag: Option<Drag>) -> f32 {
    match drag {
        Some(Drag::VerticalBar { grab, .. } | Drag::HorizontalBar { grab, .. }) => grab,
        _ => 0.0,
    }
}

fn header_style(painter: &Painter, rect: Rect, selected: bool, full: bool) -> egui::Color32 {
    match (selected, full) {
        (true, true) => {
            painter.rect_filled(rect, CornerRadius::ZERO, theme::HEADER_BG_FULL);
            theme::ON_ACCENT
        }
        (true, false) => {
            painter.rect_filled(rect, CornerRadius::ZERO, theme::header_bg_selected());
            theme::header_text_selected()
        }
        _ => theme::header_text(),
    }
}

fn row_header_width(last_row: u32) -> f32 {
    let digits = last_row.to_string().len() as f32;
    (digits * theme::ROW_HEADER_DIGIT_WIDTH + theme::ROW_HEADER_PADDING)
        .max(theme::ROW_HEADER_MIN_WIDTH)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftCursor {
    Cell,
    Down,
    Right,
}

fn dashed(painter: &Painter, from: Pos2, to: Pos2) {
    painter.extend(Shape::dashed_line(
        &[from, to],
        Stroke::new(theme::BREAK_WIDTH, theme::BREAK_LINE),
        theme::BREAK_DASH,
        theme::BREAK_DASH,
    ));
}
