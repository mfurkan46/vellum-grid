use eframe::egui::{Context, TextureHandle};

pub fn texture(ctx: &Context) -> Option<TextureHandle> {
    vellum_ui::logo::texture(
        ctx,
        "vellum-grid-logo",
        include_bytes!("../../../assets/vellum-grid-64.png"),
    )
}
