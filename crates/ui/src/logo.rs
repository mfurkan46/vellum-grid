use eframe::egui::{Context, TextureHandle};

#[cfg(not(target_arch = "wasm32"))]
pub fn texture(ctx: &Context, name: &str, png: &[u8]) -> Option<TextureHandle> {
    use eframe::egui::{ColorImage, Id, TextureOptions};

    let id = Id::unique(name);
    if let Some(texture) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(texture);
    }
    let icon = eframe::icon_data::from_png_bytes(png).ok()?;
    let size = [icon.width as usize, icon.height as usize];
    let image = ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
    let texture = ctx.load_texture(name, image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

#[cfg(target_arch = "wasm32")]
pub fn texture(_ctx: &Context, _name: &str, _png: &[u8]) -> Option<TextureHandle> {
    None
}
