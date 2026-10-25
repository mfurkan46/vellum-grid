use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};
use vellum_grid_core::{CellAddr, CellRange, CellValue, Document};

use crate::app::Spreadsheet;
use crate::icons::{self, Icon};
use crate::{fluent, theme};

const RANGE_LIMIT: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrow {
    pub from: (usize, CellRange),
    pub to: (usize, CellAddr),
    pub precedent: bool,
}

fn push(arrows: &mut Vec<Arrow>, arrow: Arrow) -> bool {
    if arrows.contains(&arrow) {
        return false;
    }
    arrows.push(arrow);
    true
}

pub fn trace_precedents(doc: &Document, arrows: &mut Vec<Arrow>, start: (usize, CellAddr)) -> bool {
    let traced: Vec<Arrow> = arrows.iter().copied().filter(|a| a.precedent).collect();
    let frontier: Vec<(usize, CellAddr)> = if traced.is_empty() {
        vec![start]
    } else {
        let mut cells = Vec::new();
        for arrow in &traced {
            let (sheet, range) = arrow.from;
            let formulas = doc.book().sheets()[sheet]
                .cells_in(range)
                .filter(|(_, cell)| cell.formula.is_some())
                .take(RANGE_LIMIT)
                .map(|(addr, _)| (sheet, addr));
            for cell in formulas {
                let done = traced.iter().any(|a| a.to == cell);
                if !done && !cells.contains(&cell) {
                    cells.push(cell);
                }
            }
        }
        cells
    };
    let mut added = false;
    for (sheet, addr) in frontier {
        for from in doc.precedents(sheet, addr) {
            let arrow = Arrow {
                from,
                to: (sheet, addr),
                precedent: true,
            };
            added |= push(arrows, arrow);
        }
    }
    added
}

pub fn trace_dependents(doc: &Document, arrows: &mut Vec<Arrow>, start: (usize, CellAddr)) -> bool {
    let traced: Vec<Arrow> = arrows.iter().copied().filter(|a| !a.precedent).collect();
    let frontier: Vec<(usize, CellAddr)> = if traced.is_empty() {
        vec![start]
    } else {
        let mut cells = Vec::new();
        for arrow in &traced {
            let cell = arrow.to;
            let done = traced
                .iter()
                .any(|a| a.from == (cell.0, CellRange::single(cell.1)));
            if !done && !cells.contains(&cell) {
                cells.push(cell);
            }
        }
        cells
    };
    let mut added = false;
    for (sheet, addr) in frontier {
        for to in doc.dependents(sheet, addr) {
            let arrow = Arrow {
                from: (sheet, CellRange::single(addr)),
                to,
                precedent: false,
            };
            added |= push(arrows, arrow);
        }
    }
    added
}

fn arrow_color(doc: &Document, arrow: &Arrow) -> Color32 {
    let (sheet, range) = arrow.from;
    let error = doc.book().sheets()[sheet]
        .cells_in(range)
        .any(|(_, cell)| matches!(cell.value, CellValue::Error(_)));
    if error {
        theme::TRACE_ERROR
    } else {
        theme::TRACE
    }
}

fn head(painter: &Painter, from: Pos2, to: Pos2, color: Color32) {
    let direction = (to - from).normalized();
    let side = vec2(-direction.y, direction.x) * theme::TRACE_HEAD / 2.0;
    let base = to - direction * theme::TRACE_HEAD;
    painter.add(Shape::convex_polygon(
        vec![to, base + side, base - side],
        color,
        Stroke::NONE,
    ));
}

fn sheet_icon(painter: &Painter, at: Pos2) {
    let rect = Rect::from_center_size(at, vec2(theme::TRACE_ICON, theme::TRACE_ICON));
    painter.rect_filled(rect, 0.0, theme::cell_bg());
    icons::paint(
        painter,
        rect,
        Icon::Plain(fluent::DOCUMENT_TABLE),
        theme::icon(),
        None,
    );
}

pub fn paint(
    painter: &Painter,
    doc: &Document,
    arrows: &[Arrow],
    sheet: usize,
    rect_of: impl Fn(CellRange) -> Rect,
) {
    for arrow in arrows {
        let color = arrow_color(doc, arrow);
        let stroke = Stroke::new(theme::TRACE_WIDTH, color);
        let (from_sheet, range) = arrow.from;
        let (to_sheet, target) = arrow.to;
        let end = rect_of(CellRange::single(target)).center();
        match (from_sheet == sheet, to_sheet == sheet) {
            (true, true) => {
                if !range.is_single() {
                    painter.rect_stroke(rect_of(range), 0.0, stroke, StrokeKind::Inside);
                }
                let start = rect_of(CellRange::single(range.start())).center();
                painter.line_segment([start, end], stroke);
                painter.circle_filled(start, theme::TRACE_DOT, color);
                head(painter, start, end, color);
            }
            (false, true) => {
                let start = end - vec2(theme::TRACE_OFFSHEET, theme::TRACE_OFFSHEET / 2.0);
                painter.extend(Shape::dashed_line(
                    &[start, end],
                    stroke,
                    theme::MARQUEE_DASH,
                    theme::MARQUEE_DASH,
                ));
                head(painter, start, end, color);
                sheet_icon(painter, start);
            }
            (true, false) => {
                let start = rect_of(CellRange::single(range.start())).center();
                let stop = start + vec2(theme::TRACE_OFFSHEET, theme::TRACE_OFFSHEET / 2.0);
                painter.extend(Shape::dashed_line(
                    &[start, stop],
                    stroke,
                    theme::MARQUEE_DASH,
                    theme::MARQUEE_DASH,
                ));
                painter.circle_filled(start, theme::TRACE_DOT, color);
                head(painter, start, stop, color);
                sheet_icon(painter, stop + vec2(theme::TRACE_ICON / 2.0, 0.0));
            }
            (false, false) => {}
        }
    }
}

impl Spreadsheet {
    pub fn sync_arrows(&mut self) {
        if self.arrows_revision != self.doc.revision() {
            self.arrows.clear();
            self.arrows_revision = self.doc.revision();
        }
    }

    pub fn trace(&mut self, precedents: bool) {
        self.sync_arrows();
        let start = (self.doc.active(), self.sel.active);
        if precedents {
            trace_precedents(&self.doc, &mut self.arrows, start);
        } else {
            trace_dependents(&self.doc, &mut self.arrows, start);
        }
    }

    pub fn remove_arrows(&mut self, kind: Option<bool>) {
        match kind {
            Some(precedent) => self.arrows.retain(|arrow| arrow.precedent != precedent),
            None => self.arrows.clear(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_grid_core::{Sheet, Workbook};

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn doc() -> Document {
        let mut sheet = Sheet::new("S");
        for (at, input) in [
            ("A1", "1"),
            ("A2", "2"),
            ("B1", "=A1*2"),
            ("B2", "=SUM(A1:A2)"),
            ("C1", "=B1+B2"),
            ("D1", "=C1"),
        ] {
            sheet.set_input(addr(at), input);
        }
        Document::new(Workbook::from_sheets(vec![sheet]).unwrap())
    }

    #[test]
    fn precedents_expand_one_level_per_click() {
        let doc = doc();
        let mut arrows = Vec::new();
        assert!(trace_precedents(&doc, &mut arrows, (0, addr("C1"))));
        let sources: Vec<_> = arrows.iter().map(|a| a.from.1.to_string()).collect();
        assert_eq!(sources, ["B1", "B2"]);
        assert!(trace_precedents(&doc, &mut arrows, (0, addr("C1"))));
        let sources: Vec<_> = arrows.iter().map(|a| a.from.1.to_string()).collect();
        assert_eq!(sources, ["B1", "B2", "A1", "A1:A2"]);
        assert!(!trace_precedents(&doc, &mut arrows, (0, addr("C1"))));
    }

    #[test]
    fn dependents_expand_one_level_per_click() {
        let doc = doc();
        let mut arrows = Vec::new();
        assert!(trace_dependents(&doc, &mut arrows, (0, addr("A1"))));
        let targets: Vec<_> = arrows.iter().map(|a| a.to.1.to_string()).collect();
        assert_eq!(targets, ["B1", "B2"]);
        assert!(trace_dependents(&doc, &mut arrows, (0, addr("A1"))));
        assert!(trace_dependents(&doc, &mut arrows, (0, addr("A1"))));
        let targets: Vec<_> = arrows.iter().map(|a| a.to.1.to_string()).collect();
        assert_eq!(targets, ["B1", "B2", "C1", "C1", "D1"]);
        assert!(!trace_dependents(&doc, &mut arrows, (0, addr("A1"))));
        assert!(!trace_dependents(&doc, &mut Vec::new(), (0, addr("D1"))));
    }
}
