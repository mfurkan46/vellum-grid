use eframe::egui::{ComboBox, FontFamily, RichText, TextEdit, Ui};
use vellum_grid_core::fold;

use crate::app::locale;
use crate::function_help::{self, Category, Help};
use crate::icons::Icon;
use crate::{assist, fonts, text, theme};

#[derive(Debug, Clone, Default)]
pub struct FunctionPicker {
    pub query: String,
    pub category: Option<Category>,
    pub selected: Option<&'static str>,
    pub focused: bool,
}

pub fn sorted(category: Option<Category>) -> Vec<&'static Help> {
    let mut found: Vec<_> = function_help::FUNCTIONS
        .iter()
        .filter(|help| category.is_none_or(|category| help.category == category))
        .collect();
    found.sort_by(|a, b| {
        locale().collate(
            locale().function_name(a.name),
            locale().function_name(b.name),
        )
    });
    found
}

pub fn search(query: &str, category: Option<Category>) -> Vec<&'static Help> {
    let words: Vec<String> = query.split_whitespace().map(fold).collect();
    sorted(category)
        .into_iter()
        .filter(|help| {
            let name = fold(locale().function_name(help.name));
            let description = fold(help.description());
            words
                .iter()
                .all(|word| name.contains(word.as_str()) || description.contains(word.as_str()))
        })
        .collect()
}

fn category_label(category: Option<Category>) -> &'static str {
    category.map_or(text::all_functions(), Category::label)
}

pub fn picker_ui(ui: &mut Ui, picker: &mut FunctionPicker) -> bool {
    let mut chosen = false;
    ui.label(text::search_function());
    let search_box = ui.add(
        TextEdit::singleline(&mut picker.query)
            .hint_text(text::search_function_hint())
            .background_color(theme::input_bg())
            .desired_width(f32::INFINITY),
    );
    if !picker.focused {
        search_box.request_focus();
        picker.focused = true;
    }
    ui.horizontal(|ui| {
        ui.label(text::select_category());
        ComboBox::from_id_salt("function_category")
            .selected_text(category_label(picker.category))
            .width(theme::FUNCTION_CATEGORY_WIDTH)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut picker.category, None, text::all_functions());
                for category in Category::ALL {
                    ui.selectable_value(&mut picker.category, Some(category), category.label());
                }
            });
    });
    let found = search(&picker.query, picker.category);
    if !picker
        .selected
        .is_some_and(|name| found.iter().any(|help| help.name == name))
    {
        picker.selected = found.first().map(|help| help.name);
    }
    ui.label(text::select_function());
    crate::dialogs::list_box(ui, |ui| {
        if found.is_empty() {
            ui.label(RichText::new(text::no_function_found()).color(theme::chrome_text_muted()));
        }
        for help in &found {
            let selected = picker.selected == Some(help.name);
            let row = assist::list_row(
                ui,
                Some(Icon::Function),
                locale().function_name(help.name),
                selected,
            );
            if row.clicked() {
                picker.selected = Some(help.name);
            }
            if row.double_clicked() {
                chosen = true;
            }
        }
    });
    if let Some(help) = picker.selected.and_then(function_help::find) {
        ui.label(
            RichText::new(assist::signature_text(help))
                .family(FontFamily::Name(fonts::CHROME_BOLD.into())),
        );
        ui.label(help.description());
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(found: Vec<&'static Help>) -> Vec<&'static str> {
        found.into_iter().map(|help| help.name).collect()
    }

    #[test]
    fn lists_functions_in_turkish_order() {
        let logical = names(sorted(Some(Category::Logical)));
        assert_eq!(logical.first(), Some(&"IFS"));
        assert!(logical.contains(&"XOR"));
        assert_eq!(sorted(None).len(), function_help::FUNCTIONS.len());
    }

    #[test]
    fn searches_names_and_descriptions() {
        assert_eq!(names(search("düşey", None)), ["VLOOKUP"]);
        assert!(names(search("iş günü", None)).contains(&"NETWORKDAYS"));
        assert!(names(search("ortalama", Some(Category::Statistical))).contains(&"AVERAGE"));
        assert!(search("ortalama", Some(Category::Text)).is_empty());
        assert_eq!(search("", Some(Category::Information)).len(), 8);
    }
}
