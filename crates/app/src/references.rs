use std::ops::Range;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId};
use vellum_grid_core::formula::{Highlight, highlights};
use vellum_grid_core::{CellRange, Workbook};

use crate::app::locale;
use crate::theme;

#[derive(Debug, Clone, PartialEq)]
pub struct Colored {
    pub span: Range<usize>,
    pub color: Color32,
    pub target: Option<(usize, CellRange)>,
}

pub fn colored(text: &str, book: &Workbook, sheet: usize) -> Vec<Colored> {
    if !text.starts_with('=') {
        return Vec::new();
    }
    let mut keys: Vec<(Option<usize>, CellRange)> = Vec::new();
    let mut out = Vec::new();
    for (span, highlight) in highlights(text, locale()) {
        let key = match highlight {
            Highlight::Reference(name, range) => {
                let index = match name {
                    Some(name) => book.index_of(&name),
                    None => Some(sheet),
                };
                (index, range)
            }
            Highlight::Name(name) => match book.name_range(&name, sheet) {
                Some((index, range)) => (Some(index), range),
                None => continue,
            },
        };
        let slot = keys
            .iter()
            .position(|known| *known == key)
            .unwrap_or_else(|| {
                keys.push(key);
                keys.len() - 1
            });
        out.push(Colored {
            span,
            color: theme::REFERENCE_COLORS[slot % theme::REFERENCE_COLORS.len()],
            target: key.0.map(|index| (index, key.1)),
        });
    }
    out
}

pub fn job(text: &str, colored: &[Colored], font: FontId, color: Color32, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let format = |color| TextFormat::simple(font.clone(), color);
    let mut at = 0;
    for part in colored {
        if part.span.start < at || part.span.end > text.len() {
            continue;
        }
        job.append(&text[at..part.span.start], 0.0, format(color));
        job.append(&text[part.span.clone()], 0.0, format(part.color));
        at = part.span.end;
    }
    job.append(&text[at..], 0.0, format(color));
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_grid_core::{DefinedName, Sheet};

    #[test]
    fn same_references_share_a_color() {
        let mut book =
            Workbook::from_sheets(vec![Sheet::new("Sayfa1"), Sheet::new("Veri")]).unwrap();
        book.add_name(DefinedName::new("Fiyat", "Veri!$A$1:$A$3"))
            .unwrap();
        let text = "=A1+B2:C3*a1+Veri!A1+Fiyat+Bilinmeyen";
        let found = colored(text, &book, 0);
        let parts: Vec<_> = found.iter().map(|part| &text[part.span.clone()]).collect();
        assert_eq!(parts, ["A1", "B2:C3", "a1", "Veri!A1", "Fiyat"]);
        assert_eq!(found[0].color, found[2].color);
        assert_ne!(found[0].color, found[1].color);
        assert_ne!(found[0].color, found[3].color);
        assert_eq!(found[3].target, Some((1, "A1".parse().unwrap())));
        assert_eq!(found[4].target, Some((1, "A1:A3".parse().unwrap())));
        assert!(colored("A1+B2", &book, 0).is_empty());
    }

    #[test]
    fn layout_keeps_all_text() {
        let book = Workbook::new();
        let text = "=TOPLA(A1;B1)";
        let found = colored(text, &book, 0);
        let job = job(
            text,
            &found,
            FontId::default(),
            Color32::BLACK,
            f32::INFINITY,
        );
        assert_eq!(job.text, text);
        assert_eq!(job.sections.len(), 5);
    }
}
