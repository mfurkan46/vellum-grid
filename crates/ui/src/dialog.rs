use eframe::egui::{self, FontFamily, Frame, Margin, RichText, Stroke, Ui};

use crate::{fonts, theme};

pub fn list_box(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::NONE
        .stroke(Stroke::new(1.0, theme::chrome_line()))
        .fill(theme::input_bg())
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .max_height(theme::LIST_HEIGHT)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    add(ui);
                });
        });
}

pub fn title(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(theme::DIALOG_TITLE_SIZE)
            .family(FontFamily::Name(fonts::CHROME_BOLD.into())),
    );
}

pub fn section(ui: &mut Ui, label: &str) {
    ui.label(RichText::new(label).color(theme::chrome_text_muted()));
}

pub fn roomy(ui: &mut Ui) {
    let style = ui.style_mut();
    style.spacing.item_spacing = egui::vec2(theme::DIALOG_SPACING, theme::DIALOG_SPACING);
    style.spacing.button_padding = egui::vec2(theme::DIALOG_BUTTON_PADDING, 4.0);
    style.spacing.interact_size.y = theme::INPUT_HEIGHT;
    for (kind, font) in style.text_styles.iter_mut() {
        if matches!(kind, egui::TextStyle::Body | egui::TextStyle::Button) {
            font.size = theme::DIALOG_FONT_SIZE;
        }
    }
}

pub fn plain_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(label)
        .min_size(egui::vec2(
            theme::DIALOG_BUTTON_MIN_WIDTH,
            theme::DIALOG_BUTTON_HEIGHT,
        ))
        .corner_radius(theme::BUTTON_RADIUS as u8)
}

pub fn accent_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(theme::ON_ACCENT))
        .fill(theme::accent_fill())
        .min_size(egui::vec2(
            theme::DIALOG_BUTTON_MIN_WIDTH,
            theme::DIALOG_BUTTON_HEIGHT,
        ))
        .corner_radius(theme::BUTTON_RADIUS as u8)
}
