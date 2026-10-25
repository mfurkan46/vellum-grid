use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::ops::Range;

use eframe::egui::{self, Key, Ui};
use spellbook::Dictionary;
use vellum_grid_core::{Cell, CellAddr, CellValue};

use crate::app::{Selection, Spreadsheet};
use crate::dialogs::{Dialog, accent_button, list_box, plain_button};
use crate::{settings, text, theme};

static TURKISH: &[u8] = include_bytes!("../dict/tr_TR.zip");
static ENGLISH: &[u8] = include_bytes!("../dict/en_US.zip");

pub struct Speller {
    dictionary: Dictionary,
    english: bool,
}

impl Speller {
    pub fn load(english: bool) -> Option<Self> {
        let (archive, name) = if english {
            (ENGLISH, "en_US")
        } else {
            (TURKISH, "tr_TR")
        };
        let mut archive = zip::ZipArchive::new(Cursor::new(archive)).ok()?;
        let mut part = |extension: &str| {
            let mut out = String::new();
            archive
                .by_name(&format!("{name}.{extension}"))
                .ok()?
                .read_to_string(&mut out)
                .ok()?;
            Some(out)
        };
        let aff = part("aff")?;
        let dic = part("dic")?;
        let mut dictionary = Dictionary::new(&aff, &dic).ok()?;
        for word in settings::user_words() {
            let _ = dictionary.add(&word);
        }
        Some(Self {
            dictionary,
            english,
        })
    }

    pub fn check(&self, word: &str) -> bool {
        self.dictionary.check(&word.replace('’', "'"))
    }

    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.dictionary.suggest(&word.replace('’', "'"), &mut out);
        out
    }

    pub fn learn(&mut self, word: &str) {
        if self.dictionary.add(word).is_ok() {
            settings::add_user_word(word);
        }
    }
}

fn is_address(chunk: &str) -> bool {
    let lower = chunk.to_lowercase();
    let mut chars = chunk.chars();
    let drive = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.next() == Some(':')
        && matches!(chars.next(), Some('\\' | '/'));
    lower.contains("://")
        || lower.starts_with("www.")
        || chunk.contains('@')
        || chunk.starts_with("\\\\")
        || drive
}

fn joiner(c: char) -> bool {
    matches!(c, '\'' | '’')
}

pub fn words(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut push = |range: Range<usize>| {
        let word = &text[range.clone()];
        let lead = word.len() - word.trim_start_matches(joiner).len();
        let trail = word.len() - word.trim_end_matches(joiner).len();
        let range = range.start + lead..range.end - trail;
        let word = &text[range.clone()];
        if !word.is_empty()
            && word.chars().any(char::is_lowercase)
            && !word.chars().any(|c| c.is_numeric())
        {
            out.push(range);
        }
    };
    let mut chunk_start = None;
    for (index, c) in text.char_indices().chain([(text.len(), ' ')]) {
        if !c.is_whitespace() {
            chunk_start.get_or_insert(index);
            continue;
        }
        let Some(start) = chunk_start.take() else {
            continue;
        };
        let chunk = &text[start..index];
        if is_address(chunk) {
            continue;
        }
        let mut begin = None;
        for (offset, c) in chunk.char_indices().chain([(chunk.len(), ' ')]) {
            if c.is_alphanumeric() || joiner(c) {
                begin.get_or_insert(offset);
            } else if let Some(first) = begin.take() {
                push(start + first..start + offset);
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellAction {
    Ignore,
    IgnoreAll,
    Add,
    Change,
    ChangeAll,
}

#[derive(Debug)]
pub struct Miss {
    addr: CellAddr,
    range: Range<usize>,
    word: String,
}

#[derive(Debug)]
pub struct SpellState {
    cells: Vec<CellAddr>,
    index: usize,
    offset: usize,
    miss: Option<Miss>,
    input: String,
    suggestions: Vec<String>,
    selected: Option<usize>,
    changes: HashMap<String, String>,
}

impl SpellState {
    fn replacement(&self) -> Option<String> {
        let miss = self.miss.as_ref()?;
        if self.input != miss.word {
            return Some(self.input.clone()).filter(|text| !text.is_empty());
        }
        self.selected
            .and_then(|index| self.suggestions.get(index))
            .cloned()
    }
}

pub fn dialog_ui(ui: &mut Ui, state: &mut SpellState) -> Option<SpellAction> {
    let mut action = None;
    ui.set_width(theme::SPELL_DIALOG_WIDTH);
    let change = state.replacement().is_some();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(theme::SPELL_FIELD_WIDTH);
            ui.label(text::not_in_dictionary());
            ui.add(egui::TextEdit::singleline(&mut state.input).desired_width(f32::INFINITY));
            ui.label(text::suggestions());
            list_box(ui, |ui| {
                if state.suggestions.is_empty() {
                    ui.label(text::no_suggestions());
                }
                for (index, word) in state.suggestions.iter().enumerate() {
                    let item = ui.selectable_label(state.selected == Some(index), word);
                    if item.clicked() {
                        state.selected = Some(index);
                    }
                    if item.double_clicked() {
                        state.selected = Some(index);
                        action = Some(SpellAction::Change);
                    }
                }
            });
            ui.label(format!(
                "{} {}",
                text::dictionary_language(),
                text::dictionary_name()
            ));
        });
        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            let size = egui::vec2(theme::SPELL_BUTTON_WIDTH, theme::DIALOG_BUTTON_HEIGHT);
            ui.add_space(theme::SPELL_BUTTON_OFFSET);
            if ui
                .add(plain_button(text::ignore_once()).min_size(size))
                .clicked()
            {
                action = Some(SpellAction::Ignore);
            }
            if ui
                .add(plain_button(text::ignore_all()).min_size(size))
                .clicked()
            {
                action = Some(SpellAction::IgnoreAll);
            }
            if ui
                .add(plain_button(text::add_to_dictionary()).min_size(size))
                .clicked()
            {
                action = Some(SpellAction::Add);
            }
            ui.add_space(theme::SPELL_BUTTON_OFFSET);
            if ui
                .add_enabled(change, accent_button(text::change()).min_size(size))
                .clicked()
            {
                action = Some(SpellAction::Change);
            }
            if ui
                .add_enabled(change, plain_button(text::change_all()).min_size(size))
                .clicked()
            {
                action = Some(SpellAction::ChangeAll);
            }
        });
    });
    if change && ui.input(|i| i.key_pressed(Key::Enter)) {
        action = Some(SpellAction::Change);
    }
    action
}

impl Spreadsheet {
    fn spell_text(&self, addr: CellAddr) -> Option<String> {
        match self.sheet().get(addr)? {
            Cell {
                value: CellValue::Text(text),
                formula: None,
                ..
            } => Some(text.clone()),
            _ => None,
        }
    }

    fn spell_cells(&self) -> Vec<CellAddr> {
        let range = self.sel.range();
        let protected = self.doc.is_protected();
        let mut cells: Vec<CellAddr> = self
            .sheet()
            .cells()
            .map(|(addr, _)| addr)
            .filter(|addr| range.is_single() || range.contains(*addr))
            .filter(|addr| !(protected && self.doc.is_locked(*addr)))
            .filter(|addr| self.spell_text(*addr).is_some())
            .collect();
        cells.sort_by_key(|addr| (addr.row(), addr.col()));
        if range.is_single() {
            let active = self.sel.active;
            let first = cells
                .iter()
                .position(|addr| (addr.row(), addr.col()) >= (active.row(), active.col()))
                .unwrap_or(0);
            cells.rotate_left(first);
        }
        cells
    }

    pub fn open_spelling(&mut self) {
        let english = vellum_ui::english();
        if self.speller.as_ref().is_none_or(|s| s.english != english) {
            self.speller = Speller::load(english);
        }
        if self.speller.is_none() {
            self.dialog = Some(Dialog::Error {
                title: text::spelling(),
                message: text::spelling_unavailable().to_owned(),
            });
            return;
        }
        let state = SpellState {
            cells: self.spell_cells(),
            index: 0,
            offset: 0,
            miss: None,
            input: String::new(),
            suggestions: Vec::new(),
            selected: None,
            changes: HashMap::new(),
        };
        self.continue_spelling(state);
    }

    fn replace_word(&mut self, addr: CellAddr, range: Range<usize>, with: &str) -> Option<usize> {
        let mut text = self.spell_text(addr)?;
        text.replace_range(range.clone(), with);
        let cell = Cell {
            value: CellValue::Text(text),
            formula: None,
            style: self.sheet().get(addr).and_then(|cell| cell.style.clone()),
        };
        self.doc
            .edit([(addr, cell)])
            .then_some(range.start + with.len())
    }

    fn continue_spelling(&mut self, mut state: SpellState) {
        state.miss = None;
        while let Some(&addr) = state.cells.get(state.index) {
            let text = self.spell_text(addr).unwrap_or_default();
            let found = words(&text)
                .into_iter()
                .filter(|range| range.start >= state.offset)
                .find(|range| {
                    let word = &text[range.clone()];
                    !self.spell_ignored.contains(word)
                        && !self.speller.as_ref().is_some_and(|s| s.check(word))
                });
            let Some(range) = found else {
                state.index += 1;
                state.offset = 0;
                continue;
            };
            let word = text[range.clone()].to_owned();
            if let Some(with) = state.changes.get(&word).cloned() {
                match self.replace_word(addr, range.clone(), &with) {
                    Some(next) => state.offset = next,
                    None => state.offset = range.end,
                }
                continue;
            }
            state.offset = range.start;
            state.suggestions = self
                .speller
                .as_ref()
                .map(|speller| speller.suggest(&word))
                .unwrap_or_default();
            state.selected = (!state.suggestions.is_empty()).then_some(0);
            state.input = word.clone();
            state.miss = Some(Miss { addr, range, word });
            self.select(Selection::at(addr));
            self.dialog = Some(Dialog::Spelling(Box::new(state)));
            return;
        }
        self.dialog = Some(Dialog::Error {
            title: text::spelling(),
            message: text::spelling_done().to_owned(),
        });
    }

    pub fn spell_action(&mut self, mut state: SpellState, action: SpellAction) {
        let replacement = state.replacement();
        let Some(miss) = state.miss.take() else {
            return;
        };
        state.offset = miss.range.end;
        match action {
            SpellAction::Ignore => {}
            SpellAction::IgnoreAll => {
                self.spell_ignored.insert(miss.word.clone());
            }
            SpellAction::Add => {
                if let Some(speller) = self.speller.as_mut() {
                    speller.learn(&miss.word);
                }
            }
            SpellAction::Change | SpellAction::ChangeAll => {
                let Some(with) = replacement else {
                    return self.continue_spelling(state);
                };
                if action == SpellAction::ChangeAll {
                    state.changes.insert(miss.word.clone(), with.clone());
                }
                if let Some(next) = self.replace_word(miss.addr, miss.range.clone(), &with) {
                    state.offset = next;
                }
            }
        }
        self.continue_spelling(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<&str> {
        words(text).into_iter().map(|range| &text[range]).collect()
    }

    #[test]
    fn splits_words_and_skips_ignored_kinds() {
        assert_eq!(
            split("Ocak ayı bütçesi: 'kira', Ali'nin payı."),
            ["Ocak", "ayı", "bütçesi", "kira", "Ali'nin", "payı"]
        );
        assert_eq!(split("KDV dahil 3kat fiyat2"), ["dahil"]);
        assert_eq!(
            split("bkz. https://ornek.com ve ali@ornek.com C:\\Belgeler\\rapor"),
            ["bkz", "ve"]
        );
        assert_eq!(split("çok-yönlü"), ["çok", "yönlü"]);
    }

    #[test]
    fn embedded_dictionary_checks_turkish() {
        let speller = Speller::load(false).unwrap();
        assert!(speller.check("kitaplar"));
        assert!(speller.check("İstanbul"));
        assert!(!speller.check("bütce"));
        assert!(speller.suggest("bütce").iter().any(|word| word == "bütçe"));
    }

    #[test]
    fn embedded_dictionary_checks_english() {
        let speller = Speller::load(true).unwrap();
        assert!(speller.check("spreadsheets"));
        assert!(speller.check("London"));
        assert!(!speller.check("recieve"));
        assert!(
            speller
                .suggest("recieve")
                .iter()
                .any(|word| word == "receive")
        );
    }
}
