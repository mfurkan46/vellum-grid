use eframe::egui::{ComboBox, Grid, Ui};

use crate::settings::{Canvas, Language, Prefs, ThemeChoice};
use crate::text;

fn theme_name(theme: ThemeChoice) -> &'static str {
    match theme {
        ThemeChoice::System => text::theme_system(),
        ThemeChoice::Light => text::theme_light(),
        ThemeChoice::Dark => text::theme_dark(),
    }
}

fn canvas_name(canvas: Canvas) -> &'static str {
    match canvas {
        Canvas::Dark => text::theme_dark(),
        Canvas::White => text::canvas_white(),
    }
}

pub fn form(ui: &mut Ui, prefs: &mut Prefs, width: f32, system_dark: bool) {
    Grid::new("vellum-options").num_columns(2).show(ui, |ui| {
        ui.label(text::theme_label());
        ComboBox::from_id_salt("vellum-theme")
            .width(width)
            .selected_text(theme_name(prefs.theme))
            .show_ui(ui, |ui| {
                for theme in ThemeChoice::ALL {
                    ui.selectable_value(&mut prefs.theme, theme, theme_name(theme));
                }
            });
        ui.end_row();
        if prefs.is_dark(system_dark) {
            ui.label(text::canvas_label());
            ComboBox::from_id_salt("vellum-canvas")
                .width(width)
                .selected_text(canvas_name(prefs.canvas))
                .show_ui(ui, |ui| {
                    for canvas in Canvas::ALL {
                        ui.selectable_value(&mut prefs.canvas, canvas, canvas_name(canvas));
                    }
                });
            ui.end_row();
        }
        ui.label(text::language_label());
        ComboBox::from_id_salt("vellum-language")
            .width(width)
            .selected_text(prefs.language.name())
            .show_ui(ui, |ui| {
                for language in Language::ALL {
                    ui.selectable_value(&mut prefs.language, language, language.name());
                }
            });
        ui.end_row();
    });
    ui.add_space(ui.spacing().item_spacing.y);
    ui.label(text::options_note());
}
