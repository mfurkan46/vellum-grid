use vellum_grid_core::Style;

pub use vellum_ui::fonts::*;

pub type CellFont = DocumentFont;

pub fn cell_font(style: &Style, zoom: f32) -> CellFont {
    document_font(
        &style.font,
        style.bold,
        style.italic,
        px(style.points()) * zoom,
    )
}

pub struct SystemFonts;

impl vellum_grid_io::print::FontSource for SystemFonts {
    fn load(&self, family: &str, bold: bool, italic: bool) -> Option<Vec<u8>> {
        font_file(family, bold, italic)
    }
}
