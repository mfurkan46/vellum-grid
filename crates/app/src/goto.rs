use vellum_grid_core::{CellRange, Kinds, Special, go_to_special};

use crate::app::{Selection, Spreadsheet};
use crate::dialogs::Dialog;
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialChoice {
    Blanks,
    Constants,
    Formulas,
    CurrentRegion,
    LastCell,
    Visible,
    Conditional,
}

impl SpecialChoice {
    pub const ALL: [Self; 7] = [
        Self::Constants,
        Self::Formulas,
        Self::Blanks,
        Self::CurrentRegion,
        Self::LastCell,
        Self::Visible,
        Self::Conditional,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Blanks => text::special_blanks(),
            Self::Constants => text::special_constants(),
            Self::Formulas => text::special_formulas(),
            Self::CurrentRegion => text::special_region(),
            Self::LastCell => text::special_last_cell(),
            Self::Visible => text::special_visible(),
            Self::Conditional => text::special_conditional(),
        }
    }

    pub fn has_kinds(self) -> bool {
        matches!(self, Self::Constants | Self::Formulas)
    }

    fn special(self, kinds: Kinds) -> Special {
        match self {
            Self::Blanks => Special::Blanks,
            Self::Constants => Special::Constants(kinds),
            Self::Formulas => Special::Formulas(kinds),
            Self::CurrentRegion => Special::CurrentRegion,
            Self::LastCell => Special::LastCell,
            Self::Visible => Special::Visible,
            Self::Conditional => Special::Conditional,
        }
    }
}

pub fn parse_reference(input: &str) -> Option<(Option<String>, CellRange)> {
    let input = input.trim();
    let (sheet, address) = match input.rsplit_once('!') {
        Some((sheet, address)) => {
            let sheet = sheet
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .map_or_else(|| sheet.to_owned(), |s| s.replace("''", "'"));
            (Some(sheet), address)
        }
        None => (None, input),
    };
    let address = address.replace('$', "").to_uppercase();
    Some((sheet, address.parse().ok()?))
}

impl Spreadsheet {
    pub fn open_go_to(&mut self) {
        self.dialog = Some(Dialog::GoTo {
            reference: String::new(),
        });
    }

    pub fn open_go_to_special(&mut self) {
        self.dialog = Some(Dialog::GoToSpecial {
            choice: SpecialChoice::Constants,
            kinds: Kinds::ALL,
        });
    }

    pub fn go_to(&mut self, reference: &str) -> bool {
        let Some((sheet, range)) = parse_reference(reference) else {
            return self.go_to_name(reference.trim());
        };
        if let Some(name) = sheet {
            let Some(index) = self.doc.book().index_of(&name) else {
                return false;
            };
            self.switch_sheet(index);
        }
        self.select(Selection {
            active: range.start(),
            anchor: range.start(),
            corner: range.end(),
        });
        true
    }

    pub fn go_to_special(&mut self, choice: SpecialChoice, kinds: Kinds) {
        let found = go_to_special(
            self.sheet(),
            self.selected(),
            self.sel.active,
            choice.special(kinds),
        );
        let Some((first, rest)) = found.split_first() else {
            self.dialog = Some(Dialog::Error {
                title: text::app_name(),
                message: text::no_cells_found().to_owned(),
            });
            return;
        };
        self.select(Selection {
            active: first.start(),
            anchor: first.start(),
            corner: first.end(),
        });
        self.extra = rest.to_vec();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_sheet_references() {
        let range = |s: &str| s.parse::<CellRange>().unwrap();
        assert_eq!(parse_reference("b5"), Some((None, range("B5"))));
        assert_eq!(parse_reference("$A$1:c3"), Some((None, range("A1:C3"))));
        assert_eq!(
            parse_reference("'Yıllık Veri'!D4"),
            Some((Some("Yıllık Veri".into()), range("D4")))
        );
        assert_eq!(
            parse_reference("Sayfa2!A:A"),
            Some((Some("Sayfa2".into()), range("A:A")))
        );
        assert_eq!(parse_reference("merhaba"), None);
    }
}
