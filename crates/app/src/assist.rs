use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    Area, Context, CornerRadius, FontFamily, FontId, Frame, Id, Label, Margin, Order, Rect,
    Response, RichText, ScrollArea, Sense, TextWrapMode, Ui, pos2, vec2,
};
use vellum_grid_core::fold;

use crate::app::{Spreadsheet, editor_cursor, locale, select_chars};
use crate::function_help::{self, Help};
use crate::icons::{self, Icon};
use crate::{fluent, fonts, theme};

#[derive(Debug, Clone)]
pub struct Assist {
    pub editor: Id,
    pub start: usize,
    pub end: usize,
    pub choices: Vec<Choice>,
    pub selected: usize,
    pub reveal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Function(&'static str),
    Name { name: String, detail: String },
}

impl Choice {
    pub fn label(&self) -> &str {
        match self {
            Self::Function(name) => locale().function_name(name),
            Self::Name { name, .. } => name,
        }
    }

    fn detail(&self) -> Option<&str> {
        match self {
            Self::Function(name) => function_help::find(name).map(|help| help.description()),
            Self::Name { detail, .. } => Some(detail.as_str()).filter(|detail| !detail.is_empty()),
        }
    }

    fn insertion(&self, followed_by_paren: bool) -> String {
        match self {
            Self::Function(_) if !followed_by_paren => format!("{}(", self.label()),
            _ => self.label().to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub start: usize,
    pub prefix: String,
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '.' || c == '_'
}

fn outside_quotes(text: &str) -> bool {
    let mut string = false;
    let mut sheet = false;
    for c in text.chars() {
        match c {
            '"' if !sheet => string = !string,
            '\'' if !string => sheet = !sheet,
            _ => {}
        }
    }
    !string && !sheet
}

pub fn word(before: &str) -> Option<Word> {
    if !before.starts_with('=') || !outside_quotes(before) {
        return None;
    }
    let start = before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_name_char(c))
        .last()
        .map(|(index, _)| index)?;
    let prefix = &before[start..];
    if !prefix.starts_with(char::is_alphabetic) {
        return None;
    }
    let previous = before[..start].chars().next_back()?;
    let opens = "=(+-*/^&<>{ ".contains(previous) || previous == locale().list;
    opens.then(|| Word {
        start,
        prefix: prefix.to_owned(),
    })
}

pub fn completions(prefix: &str) -> Vec<&'static Help> {
    let key = fold(prefix);
    let mut found: Vec<_> = function_help::FUNCTIONS
        .iter()
        .filter(|help| fold(locale().function_name(help.name)).starts_with(&key))
        .collect();
    found.sort_by(|a, b| {
        locale().collate(
            locale().function_name(a.name),
            locale().function_name(b.name),
        )
    });
    found
}

pub fn choices(prefix: &str, names: &[(String, String)]) -> Vec<Choice> {
    let key = fold(prefix);
    let mut found: Vec<Choice> = completions(prefix)
        .into_iter()
        .map(|help| Choice::Function(help.name))
        .collect();
    for (name, detail) in names {
        let folded = fold(name);
        let seen = found.iter().any(|choice| fold(choice.label()) == folded);
        if folded.starts_with(&key) && !seen {
            found.push(Choice::Name {
                name: name.clone(),
                detail: detail.clone(),
            });
        }
    }
    found.sort_by(|a, b| locale().collate(a.label(), b.label()));
    found
}

pub fn active_call(before: &str) -> Option<(&'static Help, usize)> {
    if !before.starts_with('=') {
        return None;
    }
    let mut stack: Vec<Option<(&'static Help, usize)>> = Vec::new();
    let mut name_start = None;
    let mut string = false;
    let mut sheet = false;
    for (index, c) in before.char_indices() {
        if string {
            string = c != '"';
            continue;
        }
        if sheet {
            sheet = c != '\'';
            continue;
        }
        match c {
            '"' => string = true,
            '\'' => sheet = true,
            '(' => {
                let call = name_start
                    .and_then(|start| {
                        function_help::find(&locale().canonical_function(&before[start..index]))
                    })
                    .map(|help| (help, 0));
                stack.push(call);
            }
            ')' => {
                stack.pop();
            }
            c if c == locale().list => {
                if let Some(Some((_, argument))) = stack.last_mut() {
                    *argument += 1;
                }
            }
            _ => {}
        }
        name_start = match (is_name_char(c), name_start) {
            (true, None) => Some(index),
            (true, start) => start,
            (false, _) => None,
        };
    }
    stack.into_iter().rev().flatten().next()
}

pub fn highlighted(help: &Help, argument: usize) -> Option<usize> {
    let ellipsis = help.args().iter().position(|arg| *arg == "...");
    match help.args().get(argument) {
        Some(&"...") | None => ellipsis.or(help.args().len().checked_sub(1)),
        Some(_) if ellipsis.is_some_and(|at| argument > at) => ellipsis,
        Some(_) => Some(argument),
    }
}

pub fn signature(help: &Help, argument: Option<usize>) -> Vec<(String, bool)> {
    let bold = argument.and_then(|argument| highlighted(help, argument));
    let separator = format!("{} ", locale().list);
    let mut parts = vec![(format!("{}(", locale().function_name(help.name)), false)];
    for (index, arg) in help.args().iter().enumerate() {
        if index > 0 {
            parts.push((separator.clone(), false));
        }
        parts.push(((*arg).to_owned(), bold == Some(index)));
    }
    parts.push((")".to_owned(), false));
    parts
}

pub fn signature_text(help: &Help) -> String {
    signature(help, None)
        .into_iter()
        .map(|(part, _)| part)
        .collect()
}

impl Spreadsheet {
    pub fn formula_assist(&mut self, ctx: &Context, editor: Id, anchor: Rect) {
        let active = self.doc.active();
        let names: Vec<(String, String)> = self
            .doc
            .book()
            .names()
            .iter()
            .filter(|defined| defined.scope.is_none_or(|scope| scope == active))
            .map(|defined| {
                let detail = if defined.comment.is_empty() {
                    self.doc.name_formula_text(&defined.formula)
                } else {
                    defined.comment.clone()
                };
                (defined.name.clone(), detail)
            })
            .collect();
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let (cursor, _) = editor_cursor(ctx, editor, &edit.text);
        let end = edit
            .text
            .char_indices()
            .nth(cursor)
            .map_or(edit.text.len(), |(index, _)| index);
        let before = &edit.text[..end];
        let closed = edit.assist_closed.as_deref() == Some(edit.text.as_str());
        let typed = word(before).filter(|_| !closed);
        let choices = typed
            .as_ref()
            .map(|word| choices(&word.prefix, &names))
            .unwrap_or_default();
        let call = active_call(before);
        edit.assist = match typed {
            Some(word) if !choices.is_empty() => {
                let kept = edit
                    .assist
                    .take()
                    .filter(|assist| assist.choices == choices);
                Some(Assist {
                    editor,
                    start: word.start,
                    end,
                    selected: kept.as_ref().map_or(0, |assist| assist.selected),
                    reveal: kept.is_none_or(|assist| assist.reveal),
                    choices,
                })
            }
            _ => None,
        };
        let picked = match edit.assist.as_mut() {
            Some(assist) => completion_list(ctx, anchor, assist),
            None => {
                if let Some((help, argument)) = call {
                    argument_tip(ctx, anchor, help, argument);
                }
                None
            }
        };
        if let Some(index) = picked {
            self.accept_completion(ctx, index);
        }
    }

    pub fn accept_completion(&mut self, ctx: &Context, index: usize) {
        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        let Some(assist) = edit.assist.take() else {
            return;
        };
        let Some(choice) = assist.choices.get(index) else {
            return;
        };
        let insert = choice.insertion(edit.text[assist.end..].starts_with('('));
        edit.text.replace_range(assist.start..assist.end, &insert);
        let caret = edit.text[..assist.start + insert.len()].chars().count();
        select_chars(ctx, assist.editor, caret, caret);
        ctx.memory_mut(|memory| memory.request_focus(assist.editor));
        edit.point = None;
        ctx.request_repaint();
    }
}

fn completion_list(ctx: &Context, anchor: Rect, assist: &mut Assist) -> Option<usize> {
    let mut picked = None;
    let list = Area::new(Id::unique("formula-completions"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom() + vec2(0.0, theme::ASSIST_GAP))
        .show(ctx, |ui| {
            Frame::popup(ui.style())
                .inner_margin(Margin::same(theme::MENU_MARGIN))
                .show(ui, |ui| {
                    ui.set_width(theme::ASSIST_WIDTH);
                    ScrollArea::vertical()
                        .max_height(theme::ASSIST_HEIGHT)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for (index, choice) in assist.choices.iter().enumerate() {
                                let selected = index == assist.selected;
                                let row = completion_row(ui, choice, selected);
                                if selected && assist.reveal {
                                    row.scroll_to_me(None);
                                }
                                if row.clicked() {
                                    picked = Some(index);
                                }
                            }
                        });
                })
                .response
                .rect
        });
    assist.reveal = false;
    let detail = assist.choices.get(assist.selected).and_then(Choice::detail);
    if let Some(detail) = detail {
        let corner = list.inner.right_top() + vec2(theme::ASSIST_GAP, 0.0);
        Area::new(Id::unique("formula-completion-help"))
            .order(Order::Foreground)
            .interactable(false)
            .fixed_pos(corner)
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .inner_margin(Margin::same(theme::MENU_MARGIN))
                    .show(ui, |ui| {
                        ui.set_max_width(theme::ASSIST_HELP_WIDTH);
                        ui.label(
                            RichText::new(detail)
                                .font(theme::small_font())
                                .color(theme::chrome_text()),
                        );
                    });
            });
    }
    picked
}

fn completion_row(ui: &mut Ui, choice: &Choice, selected: bool) -> Response {
    let glyph = match choice {
        Choice::Function(_) => Icon::Function,
        Choice::Name { .. } => Icon::Plain(fluent::TAG),
    };
    list_row(ui, Some(glyph), choice.label(), selected)
}

pub fn list_row(ui: &mut Ui, glyph: Option<Icon>, label: &str, selected: bool) -> Response {
    let size = vec2(ui.available_width(), theme::ASSIST_ROW_HEIGHT);
    let response = ui.allocate_response(size, Sense::click());
    let rect = response.rect;
    let painter = ui.painter();
    if selected || response.hovered() {
        let fill = if selected {
            theme::list_selected()
        } else {
            theme::tool_hover()
        };
        painter.rect_filled(rect, CornerRadius::same(theme::MENU_RADIUS), fill);
    }
    let indent = match glyph {
        Some(glyph) => {
            let icon = Rect::from_center_size(
                pos2(
                    rect.left() + theme::ASSIST_ROW_HEIGHT / 2.0,
                    rect.center().y,
                ),
                vec2(theme::MENU_ICON, theme::MENU_ICON),
            );
            icons::paint(painter, icon, glyph, theme::icon(), None);
            theme::ASSIST_ROW_HEIGHT
        }
        None => theme::MENU_MARGIN as f32,
    };
    let galley = painter.layout_no_wrap(label.to_owned(), theme::menu_font(), theme::chrome_text());
    let y = rect.center().y - galley.size().y / 2.0;
    painter.galley(pos2(rect.left() + indent, y), galley, theme::chrome_text());
    response
}

fn argument_tip(ctx: &Context, anchor: Rect, help: &Help, argument: usize) {
    let regular = theme::small_font();
    let bold = FontId::new(regular.size, FontFamily::Name(fonts::CHROME_BOLD.into()));
    let mut job = LayoutJob::default();
    for (part, strong) in signature(help, Some(argument)) {
        let font = if strong {
            bold.clone()
        } else {
            regular.clone()
        };
        job.append(&part, 0.0, TextFormat::simple(font, theme::chrome_text()));
    }
    Area::new(Id::unique("formula-arguments"))
        .order(Order::Foreground)
        .interactable(false)
        .fixed_pos(anchor.left_bottom() + vec2(0.0, theme::ASSIST_GAP))
        .show(ctx, |ui| {
            Frame::popup(ui.style())
                .inner_margin(Margin::symmetric(
                    theme::MENU_MARGIN,
                    theme::MENU_MARGIN / 2,
                ))
                .show(ui, |ui| {
                    ui.add(Label::new(job).wrap_mode(TextWrapMode::Extend));
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word_of(text: &str) -> Option<String> {
        word(text).map(|word| word.prefix)
    }

    #[test]
    fn finds_the_function_name_being_typed() {
        assert_eq!(word_of("=dü"), Some("dü".into()));
        assert_eq!(word_of("=A1+TOP"), Some("TOP".into()));
        assert_eq!(word_of("=EĞER(B2>0;TOPLA.Ç"), Some("TOPLA.Ç".into()));
        assert_eq!(word("=1+ort").map(|word| word.start), Some(3));
        assert_eq!(word_of("=A1:B"), None);
        assert_eq!(word_of("=\"metin ab"), None);
        assert_eq!(word_of("='Sayfa 1'!A"), None);
        assert_eq!(word_of("=12"), None);
        assert_eq!(word_of("toplam"), None);
        assert_eq!(word_of("=TOPLA("), None);
    }

    #[test]
    fn completions_follow_turkish_names() {
        let names: Vec<_> = completions("eğer")
            .iter()
            .map(|help| locale().function_name(help.name))
            .collect();
        assert_eq!(names.first(), Some(&"EĞER"));
        assert!(names.contains(&"EĞERHATA"));
        assert!(names.contains(&"EĞERORTALAMA"));
        assert!(names.iter().all(|name| name.starts_with("EĞER")));
        assert_eq!(
            completions("indis")
                .iter()
                .map(|help| help.name)
                .collect::<Vec<_>>(),
            ["INDEX"]
        );
        assert!(completions("zzz").is_empty());
    }

    #[test]
    fn defined_names_join_the_completions() {
        let names = [
            ("Toplam_Satış".to_owned(), String::new()),
            ("Oran".to_owned(), "KDV oranı".to_owned()),
        ];
        let found = choices("top", &names);
        let labels: Vec<_> = found.iter().map(Choice::label).collect();
        assert!(labels.contains(&"TOPLA"));
        assert!(labels.contains(&"Toplam_Satış"));
        assert!(!labels.contains(&"Oran"));
        let name = found
            .iter()
            .find(|choice| matches!(choice, Choice::Name { .. }))
            .unwrap();
        assert_eq!(name.insertion(false), "Toplam_Satış");
        assert_eq!(Choice::Function("SUM").insertion(false), "TOPLA(");
        assert_eq!(Choice::Function("SUM").insertion(true), "TOPLA");
        assert_eq!(choices("or", &names)[0].detail(), Some("KDV oranı"));
    }

    #[test]
    fn tracks_the_argument_under_the_cursor() {
        let call = |text: &str| active_call(text).map(|(help, argument)| (help.name, argument));
        assert_eq!(call("=DÜŞEYARA("), Some(("VLOOKUP", 0)));
        assert_eq!(call("=DÜŞEYARA(A1;B:C;"), Some(("VLOOKUP", 2)));
        assert_eq!(call("=EĞER(TOPLA(A1;A2)>3;"), Some(("IF", 1)));
        assert_eq!(call("=EĞER(TOPLA(A1;"), Some(("SUM", 1)));
        assert_eq!(call("=EĞER((A1+2)*3;"), Some(("IF", 1)));
        assert_eq!(call("=EĞER(A1=\"a;b\";"), Some(("IF", 1)));
        assert_eq!(call("=TOPLA(A1)"), None);
        assert_eq!(call("=BİLİNMEYEN("), None);
        assert_eq!(call("TOPLA("), None);
    }

    #[test]
    fn signature_highlights_the_current_argument() {
        let vlookup = function_help::find("VLOOKUP").unwrap();
        let parts = signature(vlookup, Some(1));
        let text: String = parts.iter().map(|(part, _)| part.as_str()).collect();
        assert_eq!(
            text,
            "DÜŞEYARA(aranan_değer; tablo_dizisi; sütun_indis_sayısı; [aralık_bak])"
        );
        let bold: Vec<_> = parts.iter().filter(|(_, bold)| *bold).collect();
        assert_eq!(bold, [&("tablo_dizisi".to_owned(), true)]);

        let sum = function_help::find("SUM").unwrap();
        assert_eq!(highlighted(sum, 0), Some(0));
        assert_eq!(highlighted(sum, 1), Some(1));
        assert_eq!(highlighted(sum, 5), Some(2));
        let switch = function_help::find("SWITCH").unwrap();
        assert_eq!(highlighted(switch, 4), Some(3));
        let today = function_help::find("TODAY").unwrap();
        assert_eq!(highlighted(today, 0), None);
        assert_eq!(signature_text(today), "BUGÜN()");
    }
}
